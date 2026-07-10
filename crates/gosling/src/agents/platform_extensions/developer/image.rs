use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use image::GenericImageView;
use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::edit::resolve_path;

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImageReadParams {
    /// Local file path or http(s) URL of the image to load.
    pub source: String,
    /// Optional crop rectangle in pixels. Coordinates are measured from the top-left corner.
    /// use to zoom in and get more details.
    #[serde(default)]
    pub crop: Option<CropParams>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CropParams {
    /// Left edge of the crop rectangle in pixels.
    pub x: u32,
    /// Top edge of the crop rectangle in pixels.
    pub y: u32,
    /// Width of the crop rectangle in pixels.
    pub width: u32,
    /// Height of the crop rectangle in pixels.
    pub height: u32,
}

pub struct ImageTool;

impl ImageTool {
    pub fn new() -> Self {
        Self
    }

    pub async fn image_read_with_cwd(
        &self,
        params: ImageReadParams,
        working_dir: Option<&Path>,
    ) -> CallToolResult {
        match load_image(&params, working_dir).await {
            Ok(loaded) => {
                let mut result = CallToolResult::success(vec![
                    Content::text(loaded.summary(&params.source)).with_priority(0.0),
                    Content::image(loaded.data, loaded.mime_type.clone()).with_priority(0.0),
                ]);
                result.structured_content = Some(json!({
                    "source": params.source,
                    "mimeType": loaded.mime_type,
                    "width": loaded.width,
                    "height": loaded.height,
                    "bytes": loaded.bytes_len,
                    "originalWidth": loaded.original_width,
                    "originalHeight": loaded.original_height,
                    "crop": params.crop,
                }));
                result
            }
            Err(error) => CallToolResult::error(vec![
                Content::text(format!("Error: {error}")).with_priority(0.0)
            ]),
        }
    }
}

impl Default for ImageTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct LoadedImage {
    data: String,
    mime_type: String,
    bytes_len: usize,
    width: u32,
    height: u32,
    original_width: u32,
    original_height: u32,
    cropped: bool,
}

impl LoadedImage {
    fn summary(&self, source: &str) -> String {
        let crop_note = if self.cropped {
            format!(
                " Cropped from {}x{} to {}x{}.",
                self.original_width, self.original_height, self.width, self.height
            )
        } else {
            String::new()
        };

        format!(
            "Loaded image from {source} ({} bytes, {}, {}x{}).{crop_note}",
            self.bytes_len, self.mime_type, self.width, self.height
        )
    }
}

async fn load_image(
    params: &ImageReadParams,
    working_dir: Option<&Path>,
) -> Result<LoadedImage, String> {
    if params.source.trim().is_empty() {
        return Err("source cannot be empty".to_string());
    }

    let bytes = load_image_bytes(&params.source, working_dir).await?;
    ensure_image_size(bytes.len() as u64)?;

    let format = image::guess_format(&bytes).map_err(|_| {
        "unsupported image format; supported formats are png, jpeg, gif, and webp".to_string()
    })?;
    let mime_type = mime_type(format)?;
    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|error| format!("failed to decode image: {error}"))?;
    let (original_width, original_height) = image.dimensions();

    let Some(crop) = &params.crop else {
        return Ok(LoadedImage {
            data: base64::prelude::BASE64_STANDARD.encode(&bytes),
            mime_type: mime_type.to_string(),
            bytes_len: bytes.len(),
            width: original_width,
            height: original_height,
            original_width,
            original_height,
            cropped: false,
        });
    };

    validate_crop(crop, original_width, original_height)?;
    let cropped = image.crop_imm(crop.x, crop.y, crop.width, crop.height);
    let mut cropped_bytes = Cursor::new(Vec::new());
    cropped
        .write_to(&mut cropped_bytes, image::ImageFormat::Png)
        .map_err(|error| format!("failed to encode cropped image: {error}"))?;
    let cropped_bytes = cropped_bytes.into_inner();
    ensure_image_size(cropped_bytes.len() as u64)?;

    Ok(LoadedImage {
        data: base64::prelude::BASE64_STANDARD.encode(&cropped_bytes),
        mime_type: "image/png".to_string(),
        bytes_len: cropped_bytes.len(),
        width: crop.width,
        height: crop.height,
        original_width,
        original_height,
        cropped: true,
    })
}

async fn load_image_bytes(source: &str, working_dir: Option<&Path>) -> Result<Vec<u8>, String> {
    if let Ok(url) = url::Url::parse(source) {
        match url.scheme() {
            "http" | "https" => load_url_bytes(url).await,
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| "invalid file URL".to_string())?;
                load_file_bytes(path)
            }
            _ => load_file_bytes(resolve_path(source, working_dir)),
        }
    } else {
        load_file_bytes(resolve_path(source, working_dir))
    }
}

async fn load_url_bytes(url: url::Url) -> Result<Vec<u8>, String> {
    // The URL is model/tool-supplied, so guard against SSRF: resolve the host and
    // refuse private/loopback/link-local targets (e.g. cloud metadata at
    // 169.254.169.254) before connecting. The redirect policy below also rejects
    // redirects to private IP literals. A residual DNS-rebinding / hostname-redirect
    // gap remains and would need connection-time IP pinning to fully close.
    ensure_url_target_is_public(&url).await?;

    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "gosling/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/repo-makeover/gosling)"
        ))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("too many redirects")
            } else if redirect_target_is_private(attempt.url()) {
                attempt.error("refusing to follow redirect to a non-public address")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|error| format!("failed to create HTTP client: {error}"))?;

    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("failed to download image: {error}"))?
        .error_for_status()
        .map_err(|error| format!("failed to download image: {error}"))?;

    // Enforce the size cap even when Content-Length is absent (chunked/streamed
    // responses) by counting bytes as they arrive rather than buffering first.
    if let Some(len) = response.content_length() {
        ensure_image_size(len)?;
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("failed to read image response: {error}"))?
    {
        ensure_image_size(bytes.len() as u64 + chunk.len() as u64)?;
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}

/// Resolve `url`'s host and reject it if any resolved address is non-public
/// (loopback/private/link-local/etc.), to prevent SSRF to internal services.
async fn ensure_url_target_is_public(url: &url::Url) -> Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return if is_disallowed_ip(ip) {
            Err(format!("refusing to fetch from non-public address {ip}"))
        } else {
            Ok(())
        };
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let mut resolved = false;
    for addr in tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("failed to resolve host '{host}': {e}"))?
    {
        resolved = true;
        if is_disallowed_ip(addr.ip()) {
            return Err(format!(
                "refusing to fetch from non-public address {} (host '{host}')",
                addr.ip()
            ));
        }
    }
    if !resolved {
        return Err(format!("host '{host}' did not resolve"));
    }
    Ok(())
}

/// Synchronous best-effort check used inside the redirect policy: true if the
/// redirect target's host is a private/loopback/link-local IP literal. Hostname
/// redirects are not re-resolved here (no async in the policy).
fn redirect_target_is_private(url: &url::Url) -> bool {
    url.host_str()
        .and_then(|h| h.parse::<std::net::IpAddr>().ok())
        .map(is_disallowed_ip)
        .unwrap_or(false)
}

fn is_disallowed_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => is_disallowed_v4(v4),
        std::net::IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_disallowed_v4(v4);
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // unique-local fc00::/7
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        }
    }
}

fn is_disallowed_v4(v4: std::net::Ipv4Addr) -> bool {
    v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.octets()[0] == 0
        || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64) // CGNAT 100.64.0.0/10
}

fn load_file_bytes(path: PathBuf) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("failed to read image file: {error}"))
}

fn validate_crop(crop: &CropParams, image_width: u32, image_height: u32) -> Result<(), String> {
    if crop.width == 0 || crop.height == 0 {
        return Err("crop width and height must be greater than zero".to_string());
    }

    let right = crop
        .x
        .checked_add(crop.width)
        .ok_or_else(|| "crop rectangle is out of bounds".to_string())?;
    let bottom = crop
        .y
        .checked_add(crop.height)
        .ok_or_else(|| "crop rectangle is out of bounds".to_string())?;

    if right > image_width || bottom > image_height {
        return Err(format!(
            "crop rectangle {}x{} at {},{} exceeds image bounds {}x{}",
            crop.width, crop.height, crop.x, crop.y, image_width, image_height
        ));
    }

    Ok(())
}

fn ensure_image_size(len: u64) -> Result<(), String> {
    if len > MAX_IMAGE_BYTES {
        Err(format!(
            "image is too large: {len} bytes exceeds {MAX_IMAGE_BYTES} byte limit"
        ))
    } else {
        Ok(())
    }
}

fn mime_type(format: image::ImageFormat) -> Result<&'static str, String> {
    match format {
        image::ImageFormat::Png => Ok("image/png"),
        image::ImageFormat::Jpeg => Ok("image/jpeg"),
        image::ImageFormat::Gif => Ok("image/gif"),
        image::ImageFormat::WebP => Ok("image/webp"),
        _ => Err(
            "unsupported image format; supported formats are png, jpeg, gif, and webp".to_string(),
        ),
    }
}
