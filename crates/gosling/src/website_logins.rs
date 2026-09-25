//! Saved website logins the agent can use without ever seeing the password.
//!
//! Name, URL and username are ordinary config so the agent can see which
//! account belongs to which site; the password lives in the secret store. The
//! agent refers to a password only through a `{{login:NAME}}` placeholder. The
//! host swaps in the real value only for a call the user approved, and scrubs it
//! from that call's output, so it never reaches the model or the transcript.

use crate::config::{Config, ConfigError};
use anyhow::{anyhow, bail, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use rmcp::model::{CallToolResult, ErrorData, RawContent, ResourceContents, ServerNotification};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::Mutex;
use uuid::Uuid;

const LOGINS_PARAM: &str = "GOSLING_WEBSITE_LOGINS";
const PASSWORD_KEY_PREFIX: &str = "GOSLING_WEBSITE_LOGIN_PASSWORD_";
const MAX_NAME_CHARS: usize = 64;
const MAX_FIELD_CHARS: usize = 2048;

static PLACEHOLDER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\{login:([^{}]+)\}\}").expect("placeholder pattern is valid"));
static MUTATION_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebsiteLogin {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
}

impl WebsiteLogin {
    pub fn placeholder(&self) -> String {
        placeholder_for(&self.name)
    }
}

#[derive(Debug, Clone)]
pub struct WebsiteLoginInput {
    pub id: Option<String>,
    pub name: String,
    pub url: String,
    pub username: String,
    /// Required for a new login; `None` keeps an existing login's password.
    pub password: Option<String>,
}

pub fn placeholder_for(name: &str) -> String {
    format!("{{{{login:{name}}}}}")
}

fn password_key(id: &str) -> String {
    format!("{PASSWORD_KEY_PREFIX}{id}")
}

pub fn list() -> Result<Vec<WebsiteLogin>> {
    match Config::global().get_param::<Vec<WebsiteLogin>>(LOGINS_PARAM) {
        Ok(logins) => Ok(logins),
        Err(ConfigError::NotFound(_)) => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

/// Logins whose password is present in the secret store.
pub fn ids_with_password() -> Result<Vec<String>> {
    let secrets = Config::global().all_secrets()?;
    Ok(list()?
        .into_iter()
        .filter(|login| secrets.contains_key(&password_key(&login.id)))
        .map(|login| login.id)
        .collect())
}

pub fn save(input: WebsiteLoginInput) -> Result<WebsiteLogin> {
    let name = validated_name(&input.name)?;
    let url = validated_url(&input.url)?;
    let username = validated_field("Username", &input.username)?;
    let password = input.password.filter(|password| !password.is_empty());
    if let Some(password) = &password {
        if password.chars().count() > MAX_FIELD_CHARS {
            bail!("Password is too long");
        }
    }

    let _guard = MUTATION_LOCK
        .lock()
        .map_err(|_| anyhow!("login store lock poisoned"))?;
    let config = Config::global();
    let mut logins = list()?;
    let id = match &input.id {
        Some(id) => {
            if !logins.iter().any(|login| &login.id == id) {
                bail!("website login not found");
            }
            id.clone()
        }
        None => {
            if password.is_none() {
                bail!("A password is required for a new website login");
            }
            Uuid::now_v7().simple().to_string()
        }
    };
    if logins
        .iter()
        .any(|login| login.id != id && login.name.eq_ignore_ascii_case(&name))
    {
        bail!("A website login named \"{name}\" already exists");
    }

    let saved = WebsiteLogin {
        id: id.clone(),
        name,
        url,
        username,
    };
    logins.retain(|login| login.id != id);
    logins.push(saved.clone());
    logins.sort_by_key(|login| login.name.to_lowercase());

    if let Some(password) = &password {
        config.set_secret(&password_key(&id), &Value::String(password.clone()))?;
    }
    if let Err(error) = config.set_param(LOGINS_PARAM, &logins) {
        if input.id.is_none() {
            let _ = config.delete_secret(&password_key(&id));
        }
        return Err(error.into());
    }
    Ok(saved)
}

pub fn delete(id: &str) -> Result<()> {
    let _guard = MUTATION_LOCK
        .lock()
        .map_err(|_| anyhow!("login store lock poisoned"))?;
    let config = Config::global();
    let mut logins = list()?;
    let before = logins.len();
    logins.retain(|login| login.id != id);
    if logins.len() == before {
        bail!("website login not found");
    }
    config.set_param(LOGINS_PARAM, &logins)?;
    match config.delete_secret(&password_key(id)) {
        Ok(()) | Err(ConfigError::NotFound(_)) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validated_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Name is required");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        bail!("Name must be at most {MAX_NAME_CHARS} characters");
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.' | '@'))
    {
        bail!("Name may contain only letters, numbers, spaces, \"_\", \"-\", \".\" or \"@\"");
    }
    Ok(name.to_string())
}

fn validated_url(url: &str) -> Result<String> {
    let url = url.trim();
    let parsed = url::Url::parse(url)
        .map_err(|_| anyhow!("Website must be a full URL, like https://example.com"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        bail!("Website must be an http or https URL");
    }
    if url.chars().count() > MAX_FIELD_CHARS {
        bail!("Website URL is too long");
    }
    Ok(url.to_string())
}

fn validated_field(label: &str, value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    if value.chars().count() > MAX_FIELD_CHARS || value.chars().any(char::is_control) {
        bail!("{label} is invalid");
    }
    Ok(value.to_string())
}

/// Names of every login placeholder in the call's string arguments.
pub fn referenced_names(arguments: Option<&Map<String, Value>>) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(arguments) = arguments {
        for value in arguments.values() {
            collect_names(value, &mut names);
        }
    }
    names
}

fn collect_names(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for captures in PLACEHOLDER.captures_iter(text) {
                let name = captures[1].trim().to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_names(item, names)),
        Value::Object(map) => map.values().for_each(|item| collect_names(item, names)),
        _ => {}
    }
}

pub fn find_by_name<'a>(logins: &'a [WebsiteLogin], name: &str) -> Option<&'a WebsiteLogin> {
    logins
        .iter()
        .find(|login| login.name.eq_ignore_ascii_case(name.trim()))
}

/// Passwords inserted into one call, kept only to scrub them from its output.
#[derive(Default)]
pub struct InsertedPasswords {
    replacements: Vec<(String, String)>,
}

impl InsertedPasswords {
    pub fn is_empty(&self) -> bool {
        self.replacements.is_empty()
    }

    pub fn redact(&self, text: &mut String) {
        for (secret, placeholder) in &self.replacements {
            if text.contains(secret.as_str()) {
                *text = text.replace(secret.as_str(), placeholder);
            }
        }
    }

    pub fn redact_value(&self, value: &mut Value) {
        match value {
            Value::String(text) => self.redact(text),
            Value::Array(items) => items.iter_mut().for_each(|item| self.redact_value(item)),
            Value::Object(map) => map.values_mut().for_each(|item| self.redact_value(item)),
            _ => {}
        }
    }

    pub fn redact_result(&self, result: &mut CallToolResult) {
        for content in &mut result.content {
            match &mut content.raw {
                RawContent::Text(text) => self.redact(&mut text.text),
                RawContent::Resource(resource) => {
                    if let ResourceContents::TextResourceContents { text, .. } =
                        &mut resource.resource
                    {
                        self.redact(text);
                    }
                }
                _ => {}
            }
        }
        if let Some(structured) = result.structured_content.as_mut() {
            self.redact_value(structured);
        }
    }

    pub fn redact_error(&self, error: &mut ErrorData) {
        self.redact(error.message.to_mut());
        if let Some(data) = error.data.as_mut() {
            self.redact_value(data);
        }
    }

    pub fn redact_notification(&self, notification: &mut ServerNotification) {
        match notification {
            ServerNotification::LoggingMessageNotification(logging) => {
                self.redact_value(&mut logging.params.data)
            }
            ServerNotification::ProgressNotification(progress) => {
                if let Some(message) = progress.params.message.as_mut() {
                    self.redact(message);
                }
            }
            _ => {}
        }
    }
}

/// Replaces every `{{login:NAME}}` in the arguments with that login's password.
pub fn insert_passwords(arguments: &mut Map<String, Value>) -> Result<InsertedPasswords> {
    let names = referenced_names(Some(arguments));
    if names.is_empty() {
        return Ok(InsertedPasswords::default());
    }
    let logins = list()?;
    let secrets = Config::global().all_secrets()?;
    let mut passwords = Vec::with_capacity(names.len());
    for name in names {
        let login = find_by_name(&logins, &name)
            .ok_or_else(|| anyhow!("No saved website login is named \"{name}\""))?;
        let password = secrets
            .get(&password_key(&login.id))
            .and_then(Value::as_str)
            .filter(|password| !password.is_empty())
            .ok_or_else(|| anyhow!("The website login \"{name}\" has no saved password"))?;
        passwords.push((name, password.to_string()));
    }
    for value in arguments.values_mut() {
        replace_placeholders(value, &passwords);
    }

    let mut inserted = InsertedPasswords::default();
    for (name, password) in passwords {
        let placeholder = placeholder_for(&name);
        let encoded = urlencoding::encode(&password).into_owned();
        if encoded != password {
            inserted.replacements.push((encoded, placeholder.clone()));
        }
        inserted.replacements.push((password, placeholder));
    }
    // Longest first so a password that contains another is scrubbed whole.
    inserted
        .replacements
        .sort_by_key(|(secret, _)| std::cmp::Reverse(secret.len()));
    Ok(inserted)
}

fn replace_placeholders(value: &mut Value, passwords: &[(String, String)]) {
    match value {
        Value::String(text) => {
            if PLACEHOLDER.is_match(text) {
                *text = PLACEHOLDER
                    .replace_all(text, |captures: &regex::Captures| {
                        let name = captures[1].trim();
                        passwords
                            .iter()
                            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                            .map(|(_, password)| password.clone())
                            .unwrap_or_else(|| captures[0].to_string())
                    })
                    .into_owned();
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| replace_placeholders(item, passwords)),
        Value::Object(map) => map
            .values_mut()
            .for_each(|item| replace_placeholders(item, passwords)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn inserted(pairs: &[(&str, &str)]) -> InsertedPasswords {
        InsertedPasswords {
            replacements: pairs
                .iter()
                .map(|(secret, name)| (secret.to_string(), placeholder_for(name)))
                .collect(),
        }
    }

    #[test]
    fn referenced_names_walks_nested_arguments_once_per_name() {
        let arguments = json!({
            "value": "{{login:GitHub}}",
            "steps": [{"fill": "user {{login: GitHub }} and {{login:Bank}}"}],
            "count": 3
        });
        let names = referenced_names(arguments.as_object());
        assert_eq!(names, vec!["GitHub".to_string(), "Bank".to_string()]);
    }

    #[test]
    fn replace_placeholders_matches_names_case_insensitively() {
        let mut value = json!({"text": "{{login:github}}", "other": ["x{{login:Unknown}}"]});
        replace_placeholders(&mut value, &[("GitHub".to_string(), "s3cret".to_string())]);
        assert_eq!(
            value,
            json!({"text": "s3cret", "other": ["x{{login:Unknown}}"]})
        );
    }

    #[test]
    fn redaction_scrubs_text_resources_and_structured_output() {
        let passwords = inserted(&[("p%40ss", "Bank"), ("p@ss", "Bank")]);
        let mut result = CallToolResult::success(vec![
            rmcp::model::Content::text("typed p@ss into /login?pw=p%40ss"),
            rmcp::model::Content::resource(ResourceContents::TextResourceContents {
                uri: "file:///log".to_string(),
                mime_type: None,
                text: "p@ss".to_string(),
                meta: None,
            }),
        ]);
        result.structured_content = Some(json!({"echo": ["p@ss"]}));

        passwords.redact_result(&mut result);

        let text = match &result.content[0].raw {
            RawContent::Text(text) => text.text.clone(),
            _ => unreachable!(),
        };
        assert_eq!(text, "typed {{login:Bank}} into /login?pw={{login:Bank}}");
        match &result.content[1].raw {
            RawContent::Resource(resource) => match &resource.resource {
                ResourceContents::TextResourceContents { text, .. } => {
                    assert_eq!(text, "{{login:Bank}}")
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
        assert_eq!(
            result.structured_content,
            Some(json!({"echo": ["{{login:Bank}}"]}))
        );
    }

    #[test]
    fn validation_rejects_unsafe_names_and_non_web_urls() {
        assert!(validated_name("Work GitHub").is_ok());
        assert!(validated_name("bad}}name").is_err());
        assert!(validated_name("   ").is_err());
        assert!(validated_url("https://github.com/login").is_ok());
        assert!(validated_url("github.com").is_err());
        assert!(validated_url("file:///etc/passwd").is_err());
        assert!(validated_field("Username", "a\nb").is_err());
    }
}
