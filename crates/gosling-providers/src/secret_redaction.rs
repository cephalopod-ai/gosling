use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub const REDACTED: &str = "[REDACTED]";

pub static SECRET_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        (
            // JSON payloads quote the header name and value
            // (`"authorization":"Bearer ..."`).
            "authorization",
            Regex::new(
                r#"(?i)\b(authorization["']?\s*[:=]\s*["']?(?:bearer\s+)?)[^\s,;"']+"#,
            )
            .expect("authorization regex"),
        ),
        (
            // Key names carry identifier prefixes/suffixes in real payloads
            // (`OPENAI_API_KEY=`) and are quoted in JSON (`"api_key":`), so the
            // name must be matched inside its surrounding identifier and the
            // separator must tolerate a closing quote.
            "api_key",
            Regex::new(
                r#"(?i)[A-Za-z0-9_.\-]*(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|secret[_-]?access[_-]?key|secret[_-]?key|auth[_-]?token|password|passwd|passphrase)[A-Za-z0-9_.\-]*[\"']?\s*[:=]\s*[\"']?[^\s,;\"']+"#,
            )
            .expect("secret assignment regex"),
        ),
        (
            "token",
            Regex::new(
                r"\b(?:sk|pk|rk|ghp|gho|ghu|ghs|github_pat|glpat|xox[baprs]|npm|AKIA|ASIA)[-_][A-Za-z0-9_\-]{10,}\b",
            )
            .expect("token regex"),
        ),
        (
            // Groq, xAI, Hugging Face and Perplexity keys. Their bodies are
            // alphanumeric, which keeps model ids such as `xai-grok-4` out.
            "provider_key",
            Regex::new(r"\b(?:gsk_|xai-|hf_|pplx-)[A-Za-z0-9]{20,}\b")
                .expect("provider key regex"),
        ),
        (
            "jwt",
            Regex::new(r"\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\b")
                .expect("jwt regex"),
        ),
        (
            "cloud_api_key",
            Regex::new(r"\bAIza[0-9A-Za-z_\-]{20,}\b").expect("cloud api key regex"),
        ),
        (
            "url_credentials",
            Regex::new(r"[a-zA-Z][a-zA-Z0-9+.\-]*://[^/\s:@]+:[^/\s@]+@")
                .expect("url credentials regex"),
        ),
        (
            "webhook_url",
            Regex::new(r"https://hooks\.slack\.com/services/[A-Za-z0-9/+_\-]+")
                .expect("webhook url regex"),
        ),
        (
            "private_key",
            Regex::new(r"(?s)-----BEGIN [^-]*PRIVATE KEY-----.*?-----END [^-]*PRIVATE KEY-----")
                .expect("private key regex"),
        ),
        (
            "url_query_secret",
            Regex::new(r"(?i)([?&](?:token|key|secret|signature|sig|auth)=)[^&#\s]+")
                .expect("URL secret regex"),
        ),
    ]
});

pub fn redact_secrets(value: &str) -> String {
    let mut redacted = value.to_string();
    for (_, pattern) in SECRET_PATTERNS.iter() {
        if pattern.is_match(&redacted) {
            redacted = pattern.replace_all(&redacted, REDACTED).into_owned();
        }
    }
    redacted
}

/// Shorter stored values (ports, user names, flags) would redact ordinary text.
const MIN_KNOWN_SECRET_CHARS: usize = 8;

/// Redacts the values an installation actually holds as secrets verbatim, then
/// applies `SECRET_PATTERNS`. Patterns only recognize credential-shaped text; a
/// configured key that a tool echoes back has no recognizable shape.
#[derive(Debug, Default, Clone)]
pub struct SecretRedactor {
    known: Vec<String>,
}

impl SecretRedactor {
    pub fn new(values: impl IntoIterator<Item = String>) -> Self {
        let mut known: Vec<String> = values
            .into_iter()
            .flat_map(|value| {
                let json_escaped = serde_json::to_string(&value).ok().and_then(|quoted| {
                    quoted
                        .strip_prefix('"')
                        .and_then(|inner| inner.strip_suffix('"'))
                        .map(str::to_string)
                });
                std::iter::once(value).chain(json_escaped)
            })
            .filter(|value| value.chars().count() >= MIN_KNOWN_SECRET_CHARS)
            .collect();
        // Longest first, so a secret that contains another is replaced whole.
        known.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        known.dedup();
        Self { known }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut redacted = text.to_string();
        for value in &self.known {
            if redacted.contains(value.as_str()) {
                redacted = redacted.replace(value.as_str(), REDACTED);
            }
        }
        redact_secrets(&redacted)
    }

    pub fn redact_json(&self, value: Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.redact(&text)),
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(|item| self.redact_json(item))
                    .collect(),
            ),
            Value::Object(fields) => Value::Object(
                fields
                    .into_iter()
                    .map(|(key, value)| (key, self.redact_json(value)))
                    .collect(),
            ),
            other => other,
        }
    }

    /// Logs hold one JSON document per line with escaped strings, where a secret
    /// right after an escaped newline (`…\nsk-…`) follows a word character and
    /// slips past `\b`-anchored patterns. Lines that parse are redacted as
    /// decoded JSON, other lines as text.
    pub fn redact_json_lines(&self, text: &str) -> String {
        text.split('\n')
            .map(|line| match serde_json::from_str::<Value>(line) {
                Ok(value @ (Value::Object(_) | Value::Array(_))) => {
                    serde_json::to_string(&self.redact_json(value))
                        .unwrap_or_else(|_| self.redact(line))
                }
                _ => self.redact(line),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_key_shapes_are_redacted_but_model_ids_are_not() {
        for key in [
            "gsk_FAKEAC04groqKEY1234567890abcdef",
            "xai-FAKEAC04xaiKEY1234567890abcdef",
            "hf_FAKEAC04hfKEY1234567890abcdef",
            "pplx-FAKEAC04pplxKEY1234567890abcdef",
        ] {
            assert_eq!(
                redact_secrets(&format!("key {key} end")),
                "key [REDACTED] end"
            );
        }
        for text in [
            "xai-grok-4-fast-reasoning",
            "hf_hub_download",
            "x-ai/grok-4",
        ] {
            assert_eq!(redact_secrets(text), text);
        }
    }

    #[test]
    fn a_json_quoted_authorization_header_is_redacted() {
        let text = r#"{"authorization":"Bearer opaque-0123456789abcdef","id":"x"}"#;
        assert_eq!(redact_secrets(text), r#"{"[REDACTED]","id":"x"}"#);
    }

    /// GSL-PT-20260927-D05: a configured key a tool echoed had no shape the
    /// patterns know.
    #[test]
    fn known_values_are_redacted_verbatim_and_short_values_are_ignored() {
        let redactor = SecretRedactor::new([
            "FAKESECRET-AC04-KEY-q7w8".to_string(),
            "2222".to_string(),
            "quote\"and\\slash-secret".to_string(),
        ]);

        assert_eq!(
            redactor.redact("tool printed FAKESECRET-AC04-KEY-q7w8 on port 2222"),
            "tool printed [REDACTED] on port 2222"
        );
        assert_eq!(
            redactor.redact(r#"{"out":"quote\"and\\slash-secret"}"#),
            r#"{"out":"[REDACTED]"}"#
        );
    }

    #[test]
    fn json_lines_are_redacted_after_decoding_escapes() {
        let redactor = SecretRedactor::default();
        let line = serde_json::json!({"content": "abcdef\nsk-ant-FAKEAC04anthropic1234567890"})
            .to_string();
        assert!(line.contains(r"\nsk-ant-"), "{line}");
        assert!(redact_secrets(&line).contains("sk-ant-FAKEAC04"));

        let redacted =
            redactor.redact_json_lines(&format!("{line}\nplain sk-proj-0123456789abcdefXYZ\n"));
        assert!(!redacted.contains("FAKEAC04"), "{redacted}");
        assert!(!redacted.contains("sk-proj-0123"), "{redacted}");
        assert_eq!(redacted.lines().count(), 2, "{redacted}");
        let first: Value = serde_json::from_str(redacted.lines().next().unwrap()).unwrap();
        assert_eq!(first["content"], "abcdef\n[REDACTED]");
    }
}
