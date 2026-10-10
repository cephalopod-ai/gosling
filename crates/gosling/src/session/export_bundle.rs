//! Bounded, resumable native acquisition without weakening ordinary import.
//!
//! A single transactional native snapshot is split at UTF-8 boundaries. Every
//! part is owner-only and digest-addressed by the final manifest; that manifest
//! is the completion point. Interrupted writes resume only for the same snapshot.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_PART_BYTES: usize = 1024 * 1024;
pub const MAX_PARTS: usize = 512;
pub const BUNDLE_SCHEMA: &str = "gosling.session-export-bundle.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportPart {
    pub index: usize,
    pub file: String,
    pub offset: usize,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundle {
    pub schema_version: String,
    pub session_id: String,
    pub snapshot_sha256: String,
    pub total_bytes: usize,
    pub redacted: bool,
    pub complete: bool,
    pub parts: Vec<ExportPart>,
}

pub fn ensure_snapshot_bound(snapshot: &str) -> Result<()> {
    if snapshot.len() > MAX_SNAPSHOT_BYTES {
        bail!("Session acquisition snapshot exceeds the 256 MiB limit");
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    crate::utils::bytes_to_hex(Sha256::digest(bytes))
}

fn persist_or_verify(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = directory.join(name);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
            bail!("Export bundle contains an incompatible existing file");
        }
        #[cfg(unix)]
        if metadata.permissions().mode() & 0o077 != 0 {
            bail!("Export bundle files must be owner-only");
        }
        if fs::read(&path)? != bytes {
            bail!("Export bundle snapshot changed; use a fresh output directory");
        }
        return Ok(());
    }
    let staging = directory.join(".staging");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    match builder.create(&staging) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&staging)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                bail!("Export staging directory is invalid");
            }
            #[cfg(unix)]
            if metadata.permissions().mode() & 0o077 != 0 {
                bail!("Export staging directory must be owner-only");
            }
        }
        Err(error) => return Err(error.into()),
    }
    let mut temporary = tempfile::NamedTempFile::new_in(staging)?;
    #[cfg(unix)]
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(&path)?;
    #[cfg(unix)]
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

/// Write or resume exactly one complete, already-redacted native snapshot.
/// Existing source parts are verified and never overwritten; a different
/// snapshot or any unrelated file requires a fresh caller-selected directory.
pub fn write_bundle(
    directory: &Path,
    session_id: &str,
    snapshot: &str,
    redacted: bool,
) -> Result<ExportBundle> {
    ensure_snapshot_bound(snapshot)?;
    let value: serde_json::Value = serde_json::from_str(snapshot)?;
    if value.get("id").and_then(|id| id.as_str()) != Some(session_id) {
        bail!("Export snapshot session identity does not match");
    }
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < snapshot.len() {
        let mut end = (offset + MAX_PART_BYTES).min(snapshot.len());
        while !snapshot.is_char_boundary(end) {
            end -= 1;
        }
        let bytes = &snapshot.as_bytes()[offset..end];
        parts.push(ExportPart {
            index: parts.len(),
            file: format!("part-{:06}.jsonfrag", parts.len()),
            offset,
            bytes: bytes.len(),
            sha256: digest(bytes),
        });
        offset = end;
    }
    if parts.len() > MAX_PARTS {
        bail!("Session acquisition snapshot exceeds the part limit");
    }
    let bundle = ExportBundle {
        schema_version: BUNDLE_SCHEMA.to_owned(),
        session_id: session_id.to_owned(),
        snapshot_sha256: digest(snapshot.as_bytes()),
        total_bytes: snapshot.len(),
        redacted,
        complete: true,
        parts,
    };
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    match builder.create(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(directory)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                bail!("Export bundle output must be a regular directory");
            }
            #[cfg(unix)]
            if metadata.permissions().mode() & 0o077 != 0 {
                bail!("Export bundle directory must be owner-only");
            }
        }
        Err(error) => return Err(error.into()),
    }
    for item in fs::read_dir(directory)? {
        let name = item?.file_name();
        if name != ".staging"
            && name != "intent.json"
            && name != "manifest.json"
            && !bundle.parts.iter().any(|part| name == part.file.as_str())
        {
            bail!("Export bundle output contains an unrelated file");
        }
    }
    let staging = directory.join(".staging");
    match fs::symlink_metadata(&staging) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                bail!("Export staging directory is invalid");
            }
            #[cfg(unix)]
            if metadata.permissions().mode() & 0o077 != 0 {
                bail!("Export staging directory must be owner-only");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut intent = bundle.clone();
    intent.complete = false;
    // Refuse a different completed snapshot before adding any new intent.
    match fs::symlink_metadata(directory.join("manifest.json")) {
        Ok(_) => {
            persist_or_verify(directory, "manifest.json", &serde_json::to_vec(&bundle)?)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    persist_or_verify(directory, "intent.json", &serde_json::to_vec(&intent)?)?;
    for part in &bundle.parts {
        persist_or_verify(
            directory,
            &part.file,
            &snapshot.as_bytes()[part.offset..part.offset + part.bytes],
        )?;
    }
    persist_or_verify(directory, "manifest.json", &serde_json::to_vec(&bundle)?)?;
    Ok(bundle)
}
