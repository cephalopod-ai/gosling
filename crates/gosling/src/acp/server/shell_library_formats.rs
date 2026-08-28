use anyhow::Result;
use image::{DynamicImage, ImageFormat, ImageOutputFormat};
use office_oxide::{Document, DocumentFormat};
use std::fs;
use std::io::Cursor;
use std::path::Path;

const MAX_OFFICE_ARCHIVE_ENTRIES: usize = 4_096;
const MAX_OFFICE_ARCHIVE_UNCOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 20_000;
const MAX_IMAGE_ALLOC_BYTES: u64 = 128 * 1024 * 1024;

pub(super) enum LinkedFileContent {
    Text(String),
    Image {
        bytes: Vec<u8>,
        mime_type: &'static str,
    },
}

pub(super) fn linked_file_mime_type(path: &Path) -> Option<&'static str> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if let Some(mime_type) = extension.as_deref().and_then(extension_mime_type) {
        validate_linked_file(path, mime_type).ok()?;
        return Some(mime_type);
    }
    let bytes = fs::read(path).ok()?;
    decode_text(&bytes).ok()?;
    Some("text/plain")
}

pub(super) fn bytes_match_mime(bytes: &[u8], mime_type: &str) -> bool {
    match mime_type {
        "application/pdf" => bytes.starts_with(b"%PDF-"),
        "application/json" => serde_json::from_slice::<serde_json::Value>(bytes).is_ok(),
        "application/x-ndjson" => validate_ndjson(bytes),
        "application/postscript" => bytes.starts_with(b"%!PS") && decode_text(bytes).is_ok(),
        mime if raster_image_format(mime).is_some() => validate_raster_image(bytes, mime).is_ok(),
        mime if mime.starts_with("text/") || mime == "image/svg+xml" => decode_text(bytes).is_ok(),
        _ => false,
    }
}

pub(super) fn is_raster_image_mime(mime_type: &str) -> bool {
    raster_image_format(mime_type).is_some()
}

pub(super) fn resolve_linked_file(
    path: &Path,
    mime_type: &str,
    text_limit: usize,
    pdf_page_limit: usize,
    pdf_object_limit: usize,
) -> Result<LinkedFileContent> {
    if mime_type == "application/pdf" {
        return Ok(LinkedFileContent::Text(extract_bounded_pdf_text(
            path,
            text_limit,
            pdf_page_limit,
            pdf_object_limit,
        )?));
    }
    if office_format_for_mime(mime_type).is_some() {
        return Ok(LinkedFileContent::Text(extract_office_text(
            path, mime_type,
        )?));
    }

    let bytes = fs::read(path)?;
    if raster_image_format(mime_type).is_some() {
        let image = decode_raster_image(&bytes, mime_type)?;
        return if matches!(
            mime_type,
            "image/bmp" | "image/tiff" | "image/x-icon" | "image/x-tga" | "image/x-portable-anymap"
        ) {
            let mut normalized = Cursor::new(Vec::new());
            image.write_to(&mut normalized, ImageOutputFormat::Png)?;
            Ok(LinkedFileContent::Image {
                bytes: normalized.into_inner(),
                mime_type: "image/png",
            })
        } else {
            Ok(LinkedFileContent::Image {
                bytes,
                mime_type: canonical_raster_mime(mime_type),
            })
        };
    }
    if mime_type == "application/json" {
        let value = serde_json::from_slice::<serde_json::Value>(&bytes)?;
        return Ok(LinkedFileContent::Text(serde_json::to_string_pretty(
            &value,
        )?));
    }
    if mime_type == "application/x-ndjson" {
        anyhow::ensure!(validate_ndjson(&bytes), "invalid newline-delimited JSON");
    }
    if mime_type == "application/postscript" {
        anyhow::ensure!(bytes.starts_with(b"%!PS"), "invalid PostScript header");
    }
    Ok(LinkedFileContent::Text(decode_text(&bytes)?))
}

fn extension_mime_type(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" | "dib" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "ico" => "image/x-icon",
        "tga" => "image/x-tga",
        "pbm" | "pgm" | "ppm" | "pnm" => "image/x-portable-anymap",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "jsonl" | "ndjson" => "application/x-ndjson",
        "ps" | "eps" => "application/postscript",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "xml" => "text/xml",
        "yaml" | "yml" => "text/yaml",
        "rtf" => "text/rtf",
        "css" => "text/css",
        "txt" | "text" | "rst" | "log" | "toml" | "ini" | "cfg" | "conf" | "properties" | "env"
        | "tex" | "bib" | "rs" | "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "py" | "go"
        | "java" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "swift" | "kt" | "kts" | "rb"
        | "php" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" | "sql" => "text/plain",
        _ => return None,
    })
}

fn validate_linked_file(path: &Path, mime_type: &str) -> Result<()> {
    if let Some(expected) = office_format_for_mime(mime_type) {
        validate_office_archive(path, expected)?;
        let document = Document::open(path)?;
        anyhow::ensure!(
            document.format() == expected,
            "office document type changed"
        );
        return Ok(());
    }
    let bytes = fs::read(path)?;
    anyhow::ensure!(
        bytes_match_mime(&bytes, mime_type),
        "file content does not match its type"
    );
    Ok(())
}

fn extract_office_text(path: &Path, mime_type: &str) -> Result<String> {
    let expected = office_format_for_mime(mime_type)
        .ok_or_else(|| anyhow::anyhow!("unsupported office document type"))?;
    validate_office_archive(path, expected)?;
    let document = Document::open(path)?;
    anyhow::ensure!(
        document.format() == expected,
        "office document type changed"
    );
    Ok(document.plain_text())
}

fn validate_office_archive(path: &Path, format: DocumentFormat) -> Result<()> {
    if format.is_legacy() {
        return Ok(());
    }
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)?;
    anyhow::ensure!(
        archive.len() <= MAX_OFFICE_ARCHIVE_ENTRIES,
        "office document has too many archive entries"
    );
    let mut total = 0u64;
    for index in 0..archive.len() {
        total = total.saturating_add(archive.by_index(index)?.size());
        anyhow::ensure!(
            total <= MAX_OFFICE_ARCHIVE_UNCOMPRESSED_BYTES,
            "office document expands beyond the extraction limit"
        );
    }
    Ok(())
}

fn office_format_for_mime(mime_type: &str) -> Option<DocumentFormat> {
    Some(match mime_type {
        "application/msword" => DocumentFormat::Doc,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            DocumentFormat::Docx
        }
        "application/vnd.ms-powerpoint" => DocumentFormat::Ppt,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            DocumentFormat::Pptx
        }
        "application/vnd.ms-excel" => DocumentFormat::Xls,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => DocumentFormat::Xlsx,
        _ => return None,
    })
}

fn raster_image_format(mime_type: &str) -> Option<ImageFormat> {
    Some(match mime_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        "image/bmp" => ImageFormat::Bmp,
        "image/tiff" => ImageFormat::Tiff,
        "image/x-icon" => ImageFormat::Ico,
        "image/x-tga" => ImageFormat::Tga,
        "image/x-portable-anymap" => ImageFormat::Pnm,
        _ => return None,
    })
}

fn canonical_raster_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "image/png",
        "image/jpeg" => "image/jpeg",
        "image/gif" => "image/gif",
        "image/webp" => "image/webp",
        "image/bmp" => "image/bmp",
        "image/tiff" => "image/tiff",
        "image/x-icon" => "image/x-icon",
        "image/x-tga" => "image/x-tga",
        "image/x-portable-anymap" => "image/x-portable-anymap",
        _ => unreachable!("validated raster MIME"),
    }
}

fn validate_raster_image(bytes: &[u8], mime_type: &str) -> Result<()> {
    decode_raster_image(bytes, mime_type).map(|_| ())
}

fn decode_raster_image(bytes: &[u8], mime_type: &str) -> Result<DynamicImage> {
    let expected =
        raster_image_format(mime_type).ok_or_else(|| anyhow::anyhow!("unsupported image type"))?;
    let mut reader = image::io::Reader::with_format(Cursor::new(bytes), expected);
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_ALLOC_BYTES);
    reader.limits(limits);
    Ok(reader.decode()?)
}

fn validate_ndjson(bytes: &[u8]) -> bool {
    let Ok(text) = decode_text(bytes) else {
        return false;
    };
    let mut found = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        found = true;
        if serde_json::from_str::<serde_json::Value>(line).is_err() {
            return false;
        }
    }
    found
}

fn decode_text(bytes: &[u8]) -> Result<String> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if let Some(content) = bytes.strip_prefix(b"\xff\xfe") {
        anyhow::ensure!(content.len() % 2 == 0, "invalid UTF-16 text");
        let values = content
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        return Ok(String::from_utf16(&values)?);
    }
    if let Some(content) = bytes.strip_prefix(b"\xfe\xff") {
        anyhow::ensure!(content.len() % 2 == 0, "invalid UTF-16 text");
        let values = content
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        return Ok(String::from_utf16(&values)?);
    }
    anyhow::ensure!(!bytes.contains(&0), "binary content is not text");
    Ok(std::str::from_utf8(bytes)?.to_string())
}

fn extract_bounded_pdf_text(
    path: &Path,
    text_limit: usize,
    page_limit: usize,
    object_limit: usize,
) -> Result<String> {
    let document = lopdf::Document::load(path)?;
    let pages = document.get_pages();
    ensure_pdf_complexity(
        document.objects.len(),
        pages.len(),
        object_limit,
        page_limit,
    )?;

    let mut text = String::new();
    for page in pages.keys().copied() {
        let page_text = document.extract_text(&[page])?;
        if text.len().saturating_add(page_text.len()) > text_limit {
            let remaining = text_limit.saturating_sub(text.len());
            let mut boundary = remaining.min(page_text.len());
            while !page_text.is_char_boundary(boundary) {
                boundary -= 1;
            }
            text.push_str(page_text.get(..boundary).unwrap_or_default());
            text.push_str("\n[Content truncated]");
            break;
        }
        text.push_str(&page_text);
    }
    Ok(text)
}

fn ensure_pdf_complexity(
    object_count: usize,
    page_count: usize,
    object_limit: usize,
    page_limit: usize,
) -> Result<()> {
    anyhow::ensure!(object_count <= object_limit, "PDF has too many objects");
    anyhow::ensure!(page_count <= page_limit, "PDF has too many pages");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_image(path: &Path, format: ImageOutputFormat) {
        let image = DynamicImage::new_rgba8(2, 2);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        fs::write(path, bytes.into_inner()).unwrap();
    }

    #[test]
    fn recognizes_traditional_text_data_and_postscript_files() {
        let root = tempfile::tempdir().unwrap();
        for (name, content, expected) in [
            ("notes.rtf", "{\\rtf1 Research notes}", "text/rtf"),
            (
                "figure.svg",
                "<svg><text>Evidence</text></svg>",
                "image/svg+xml",
            ),
            (
                "plot.ps",
                "%!PS-Adobe-3.0\n(Research) show",
                "application/postscript",
            ),
            (
                "records.jsonl",
                "{\"id\":1}\n{\"id\":2}\n",
                "application/x-ndjson",
            ),
            ("analysis.tex", "\\section{Evidence}", "text/plain"),
        ] {
            let path = root.path().join(name);
            fs::write(&path, content).unwrap();
            assert_eq!(linked_file_mime_type(&path), Some(expected));
        }
    }

    #[test]
    fn normalizes_non_provider_raster_formats_to_png() {
        let root = tempfile::tempdir().unwrap();
        for (name, format) in [
            ("figure.bmp", ImageOutputFormat::Bmp),
            ("figure.tiff", ImageOutputFormat::Tiff),
            ("figure.ico", ImageOutputFormat::Ico),
            ("figure.tga", ImageOutputFormat::Tga),
        ] {
            let path = root.path().join(name);
            write_image(&path, format);
            let mime_type = linked_file_mime_type(&path).unwrap_or_else(|| panic!("{name}"));
            let LinkedFileContent::Image { bytes, mime_type } =
                resolve_linked_file(&path, mime_type, 1024, 8, 100).unwrap()
            else {
                panic!("expected an image");
            };
            assert_eq!(mime_type, "image/png");
            assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Png);
        }
    }

    #[test]
    fn extracts_docx_xlsx_and_pptx_as_research_text() {
        let root = tempfile::tempdir().unwrap();
        for (name, format, expected_mime) in [
            (
                "report.docx",
                DocumentFormat::Docx,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
            (
                "evidence.xlsx",
                DocumentFormat::Xlsx,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            (
                "briefing.pptx",
                DocumentFormat::Pptx,
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ),
        ] {
            let path = root.path().join(name);
            office_oxide::create::create_from_markdown(
                "# Research evidence\n\nConfirmed finding",
                format,
                &path,
            )
            .unwrap();
            assert_eq!(linked_file_mime_type(&path), Some(expected_mime));
            let LinkedFileContent::Text(text) =
                resolve_linked_file(&path, expected_mime, 64 * 1024, 8, 10_000).unwrap()
            else {
                panic!("expected office text");
            };
            assert!(text.contains("Confirmed finding"), "{name}: {text}");
        }
    }

    #[test]
    fn maps_legacy_word_excel_and_powerpoint_formats() {
        assert_eq!(extension_mime_type("doc"), Some("application/msword"));
        assert_eq!(extension_mime_type("xls"), Some("application/vnd.ms-excel"));
        assert_eq!(
            extension_mime_type("ppt"),
            Some("application/vnd.ms-powerpoint")
        );
    }

    #[test]
    fn rejects_suffix_content_mismatches_and_invalid_structured_text() {
        let root = tempfile::tempdir().unwrap();
        for (name, content) in [
            ("report.docx", b"not a DOCX".as_slice()),
            ("figure.tiff", b"not a TIFF".as_slice()),
            ("plot.ps", b"not PostScript".as_slice()),
            ("records.jsonl", b"{not json}\n".as_slice()),
        ] {
            let path = root.path().join(name);
            fs::write(&path, content).unwrap();
            assert_eq!(linked_file_mime_type(&path), None, "{name}");
        }
    }

    #[test]
    fn accepts_utf8_and_utf16_text() {
        let root = tempfile::tempdir().unwrap();
        let utf8 = root.path().join("utf8.txt");
        fs::write(&utf8, "evidence").unwrap();
        assert_eq!(linked_file_mime_type(&utf8), Some("text/plain"));

        let utf16 = root.path().join("utf16.txt");
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend("evidence".encode_utf16().flat_map(u16::to_le_bytes));
        fs::write(&utf16, bytes).unwrap();
        assert_eq!(linked_file_mime_type(&utf16), Some("text/plain"));
        let LinkedFileContent::Text(text) =
            resolve_linked_file(&utf16, "text/plain", 1024, 8, 100).unwrap()
        else {
            panic!("expected text");
        };
        assert_eq!(text, "evidence");

        let uncommon_suffix = root.path().join("notes.research");
        fs::write(&uncommon_suffix, "plain text with an uncommon suffix").unwrap();
        assert_eq!(linked_file_mime_type(&uncommon_suffix), Some("text/plain"));

        let binary = root.path().join("unknown.data");
        fs::write(&binary, [0xff, 0x00, 0xfe, 0x01]).unwrap();
        assert_eq!(linked_file_mime_type(&binary), None);
    }

    #[test]
    fn rejects_pdf_complexity_before_extraction() {
        ensure_pdf_complexity(50_000, 256, 50_000, 256).unwrap();
        assert!(ensure_pdf_complexity(50_001, 1, 50_000, 256)
            .unwrap_err()
            .to_string()
            .contains("too many objects"));
        assert!(ensure_pdf_complexity(1, 257, 50_000, 256)
            .unwrap_err()
            .to_string()
            .contains("too many pages"));
    }
}
