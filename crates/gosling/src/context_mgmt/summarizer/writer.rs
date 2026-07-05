//! Appends extracted facts to `memories.jsonl`, giving the existing
//! [`super::super::memory::FileMemorySource`] read seam its first producer.
//!
//! Each record carries the extracted `content`/`type`/`confidence` plus
//! `source`, `session_id`, and `created_at`. `FileMemorySource` only reads
//! `content` (required) and `source` (optional, defaulted) — the extra
//! fields ride along as context for a future retrieval-side consumer without
//! breaking the existing reader, since serde ignores unknown fields on a
//! struct that doesn't opt into `deny_unknown_fields`.

use std::io::Write;
use std::path::Path;

use serde::Serialize;

use super::schema::ExtractedFact;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryRecord {
    pub content: String,
    pub source: String,
    #[serde(rename = "type")]
    pub fact_type: String,
    pub confidence: f32,
    pub session_id: String,
    pub created_at: String,
}

/// Builds one [`MemoryRecord`] per extracted fact. `source` encodes the
/// fact's type (e.g. `summarizer:decision`) so it reads as useful provenance
/// wherever `FileMemorySource` renders it alongside recalled content.
pub fn records_for_facts(
    facts: &[ExtractedFact],
    session_id: &str,
    created_at: &str,
) -> Vec<MemoryRecord> {
    facts
        .iter()
        .map(|fact| MemoryRecord {
            content: fact.content.clone(),
            source: format!("summarizer:{}", fact.fact_type.as_str()),
            fact_type: fact.fact_type.as_str().to_string(),
            confidence: fact.confidence,
            session_id: session_id.to_string(),
            created_at: created_at.to_string(),
        })
        .collect()
}

/// Appends `records` as JSON lines to `path`, creating the file (and any
/// missing parent directory) if needed. A no-op for an empty slice — callers
/// don't need to special-case "no facts extracted".
pub fn append_memories(path: &Path, records: &[MemoryRecord]) -> std::io::Result<()> {
    if records.is_empty() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    for record in records {
        let line = serde_json::to_string(record)
            .expect("MemoryRecord only contains plain strings and a float");
        writeln!(file, "{line}")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_mgmt::summarizer::schema::FactType;
    use crate::context_mgmt::MemorySource;

    fn fact(content: &str, fact_type: FactType) -> ExtractedFact {
        ExtractedFact {
            content: content.to_string(),
            fact_type,
            confidence: 0.75,
        }
    }

    #[test]
    fn append_memories_is_noop_for_empty_slice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memories.jsonl");
        append_memories(&path, &[]).unwrap();
        assert!(!path.exists(), "no file should be created for zero facts");
    }

    #[test]
    fn round_trips_through_file_memory_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memories.jsonl");

        let facts = vec![
            fact("The project was renamed to gosling", FactType::Fact),
            fact(
                "Use anyhow::Result for error handling",
                FactType::Preference,
            ),
        ];
        let records = records_for_facts(&facts, "session-123", "2026-07-05T00:00:00Z");
        append_memories(&path, &records).unwrap();

        let source = crate::context_mgmt::FileMemorySource::new(path);
        let messages = vec![crate::conversation::message::Message::user()
            .with_text("what did we decide about the project name and error handling?")];
        let recalled = source.retrieve(&crate::context_mgmt::MemoryQuery {
            session_id: "session-123",
            messages: &messages,
            reserved_tokens: 1_000,
        });

        assert_eq!(recalled.len(), 2);
        assert!(recalled
            .iter()
            .any(|item| item.content.contains("gosling") && item.source == "summarizer:fact"));
        assert!(recalled
            .iter()
            .any(|item| item.content.contains("anyhow::Result")
                && item.source == "summarizer:preference"));
    }

    #[test]
    fn appends_to_existing_file_without_truncating() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memories.jsonl");

        let first = records_for_facts(
            &[fact("first fact", FactType::Fact)],
            "session-a",
            "2026-07-05T00:00:00Z",
        );
        append_memories(&path, &first).unwrap();

        let second = records_for_facts(
            &[fact("second fact", FactType::Decision)],
            "session-b",
            "2026-07-05T00:01:00Z",
        );
        append_memories(&path, &second).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.lines().count(), 2);
        assert!(contents.contains("first fact"));
        assert!(contents.contains("second fact"));
    }
}
