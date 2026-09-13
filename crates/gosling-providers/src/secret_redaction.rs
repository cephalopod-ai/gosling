use regex::Regex;
use std::sync::LazyLock;

pub static SECRET_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        (
            "authorization",
            Regex::new(r"(?i)\b(authorization\s*[:=]\s*(?:bearer\s+)?)[^\s,;]+")
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
            redacted = pattern.replace_all(&redacted, "[REDACTED]").into_owned();
        }
    }
    redacted
}
