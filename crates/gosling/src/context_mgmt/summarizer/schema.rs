//! Request/response shapes for the local-LLM summarizer worker.
//!
//! The worker prompt instructs the model to reply with only JSON matching
//! [`WorkerResponse`] — no prose, no markdown fences. Anything else (garbled
//! text, a wrapped code fence, a missing field) fails to deserialize and the
//! caller falls back to the deterministic truncation stub.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub summary: String,
    #[serde(default)]
    pub facts: Vec<ExtractedFact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedFact {
    pub content: String,
    #[serde(rename = "type")]
    pub fact_type: FactType,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactType {
    Fact,
    Decision,
    Preference,
    Entity,
}

impl FactType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FactType::Fact => "fact",
            FactType::Decision => "decision",
            FactType::Preference => "preference",
            FactType::Entity => "entity",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_response() {
        let raw = r#"{
            "summary": "User is building a Rust CLI called gosling.",
            "facts": [
                {"content": "Project is named gosling", "type": "entity", "confidence": 0.9},
                {"content": "User prefers anyhow::Result for errors", "type": "preference", "confidence": 0.8}
            ]
        }"#;
        let parsed: WorkerResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(
            parsed.summary,
            "User is building a Rust CLI called gosling."
        );
        assert_eq!(parsed.facts.len(), 2);
        assert_eq!(parsed.facts[0].fact_type, FactType::Entity);
    }

    #[test]
    fn empty_facts_array_is_valid() {
        let raw = r#"{"summary": "Nothing durable happened.", "facts": []}"#;
        let parsed: WorkerResponse = serde_json::from_str(raw).unwrap();
        assert!(parsed.facts.is_empty());
    }

    #[test]
    fn missing_facts_field_defaults_to_empty() {
        let raw = r#"{"summary": "Only a summary, no facts key at all."}"#;
        let parsed: WorkerResponse = serde_json::from_str(raw).unwrap();
        assert!(parsed.facts.is_empty());
    }

    #[test]
    fn malformed_json_fails_to_parse() {
        let raw = "not json at all";
        assert!(serde_json::from_str::<WorkerResponse>(raw).is_err());
    }

    #[test]
    fn missing_summary_field_fails_to_parse() {
        let raw = r#"{"facts": []}"#;
        assert!(serde_json::from_str::<WorkerResponse>(raw).is_err());
    }
}
