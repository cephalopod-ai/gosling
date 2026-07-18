//! Appends extracted facts to `memories.jsonl`, giving the existing
//! [`super::super::memory::FileMemorySource`] read seam its first producer.
//!
//! Each record carries the extracted `content`/`type`/`confidence` plus
//! `source`, `session_id`, and `created_at`. `FileMemorySource` only reads
//! `content` (required) and `source` (optional, defaulted) — the extra
//! fields ride along as context for a future retrieval-side consumer without
//! breaking the existing reader, since serde ignores unknown fields on a
//! struct that doesn't opt into `deny_unknown_fields`.

use std::io::{Read, Write};
use std::path::Path;

use fs2::FileExt;
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

/// Heading under which extracted facts are grouped when appended to a
/// self-managing backend's durable file (`CLAUDE.md` / `AGENTS.md`).
const DURABLE_SECTION_HEADING: &str = "## Gosling extracted memory";

/// Appends `facts` as Markdown bullets to a self-managing backend's durable
/// file (e.g. `CLAUDE.md` / `AGENTS.md`), creating it (and any missing parent
/// directory) if needed. The section heading is written once, the first time
/// this producer touches the file, so repeated appends accumulate under a
/// single heading rather than repeating it. A no-op for an empty slice.
///
/// Unlike [`append_memories`], the target is a human-facing Markdown file the
/// backend reads as instructions, so facts are rendered as readable bullets
/// with light provenance rather than JSON lines.
pub fn append_facts_to_durable_file(
    path: &Path,
    label: &str,
    facts: &[ExtractedFact],
    session_id: &str,
    created_at: &str,
) -> std::io::Result<()> {
    if facts.is_empty() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Hold an exclusive lock across the read-check and the append so two
    // concurrent summarizer runs on the same project can't both observe
    // "heading missing" and each append their own copy of it.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    file.lock_exclusive()?;

    let mut existing = String::new();
    file.read_to_string(&mut existing)?;
    let needs_heading = !existing.contains(DURABLE_SECTION_HEADING);

    if needs_heading {
        writeln!(file, "\n{DURABLE_SECTION_HEADING}")?;
        writeln!(
            file,
            "<!-- appended by gosling from {label}; safe to edit or curate -->\n"
        )?;
    }

    for fact in facts {
        writeln!(
            file,
            "- {} _({}, session {}, {})_",
            fact.content,
            fact.fact_type.as_str(),
            session_id,
            created_at
        )?;
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
    fn durable_file_writes_heading_once_and_accumulates_bullets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");

        append_facts_to_durable_file(
            &path,
            "AGENTS.md",
            &[fact("Project renamed to gosling", FactType::Fact)],
            "session-a",
            "2026-07-05T00:00:00Z",
        )
        .unwrap();
        append_facts_to_durable_file(
            &path,
            "AGENTS.md",
            &[fact("Use anyhow::Result", FactType::Preference)],
            "session-b",
            "2026-07-05T00:01:00Z",
        )
        .unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents.matches(DURABLE_SECTION_HEADING).count(),
            1,
            "the managed heading should be written exactly once across appends"
        );
        assert!(contents.contains("- Project renamed to gosling _(fact, session session-a,"));
        assert!(contents.contains("- Use anyhow::Result _(preference, session session-b,"));
    }

    #[test]
    fn concurrent_durable_file_appends_write_heading_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    append_facts_to_durable_file(
                        &path,
                        "AGENTS.md",
                        &[fact(&format!("fact {i}"), FactType::Fact)],
                        &format!("session-{i}"),
                        "2026-07-05T00:00:00Z",
                    )
                    .unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents.matches(DURABLE_SECTION_HEADING).count(),
            1,
            "concurrent summarizer runs on the same project must not duplicate the heading"
        );
        for i in 0..8 {
            assert!(contents.contains(&format!("fact {i}")));
        }
    }

    #[test]
    fn durable_file_is_noop_for_empty_slice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("CLAUDE.md");
        append_facts_to_durable_file(&path, "CLAUDE.md", &[], "s", "t").unwrap();
        assert!(!path.exists());
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
