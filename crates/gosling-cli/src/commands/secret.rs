use anyhow::{bail, Context, Result};
use gosling::config::Config;
use serde_json::Value;

const FIELDS: &[&str] = &["LOGIN", "PASSWORD", "HOST", "PORT"];

fn normalize_name(name: &str) -> String {
    name.trim().to_uppercase()
}

fn field_key(name: &str, field: &str) -> String {
    format!("{name}_{field}")
}

pub struct SetArgs {
    pub name: String,
    pub login: Option<String>,
    pub password: Option<String>,
    pub host: Option<String>,
    pub port: Option<String>,
}

pub fn handle_set(args: SetArgs) -> Result<()> {
    let name = normalize_name(&args.name);
    if name.is_empty() {
        bail!("server name must not be empty");
    }

    let mut updates: Vec<(String, Value)> = Vec::new();
    if let Some(login) = args.login {
        updates.push((field_key(&name, "LOGIN"), Value::String(login)));
    }
    if let Some(password) = args.password {
        updates.push((field_key(&name, "PASSWORD"), Value::String(password)));
    }
    if let Some(host) = args.host {
        updates.push((field_key(&name, "HOST"), Value::String(host)));
    }
    if let Some(port) = args.port {
        updates.push((field_key(&name, "PORT"), Value::String(port)));
    }

    if updates.is_empty() {
        bail!("provide at least one of --login, --password, --host, --port");
    }

    let config = Config::global();
    config
        .set_secret_values(&updates)
        .context("failed to store server credentials")?;

    println!(
        "Stored {} field(s) for server '{}' in {}",
        updates.len(),
        name,
        config.path()
    );
    Ok(())
}

pub fn handle_get(name: &str, field: Option<&str>) -> Result<()> {
    let name = normalize_name(name);
    let config = Config::global();

    let fields: Vec<&str> = match field {
        Some(f) => vec![f],
        None => FIELDS.to_vec(),
    };

    let mut found_any = false;
    for field in fields {
        let field = field.to_uppercase();
        let key = field_key(&name, &field);
        if let Ok(value) = config.get_secret::<String>(&key) {
            println!("{key}={value}");
            found_any = true;
        }
    }

    if !found_any {
        bail!("no stored credentials found for server '{name}'");
    }
    Ok(())
}

pub fn handle_remove(name: &str) -> Result<()> {
    let name = normalize_name(name);
    let config = Config::global();

    let keys: Vec<String> = FIELDS.iter().map(|field| field_key(&name, field)).collect();
    config
        .delete_secret_values(&keys)
        .context("failed to remove server credentials")?;

    println!("Removed credentials for server '{name}'");
    Ok(())
}

pub fn handle_list() -> Result<()> {
    let config = Config::global();
    let secrets = config
        .all_secrets()
        .context("failed to read stored secrets")?;

    let mut names: Vec<&str> = secrets
        .keys()
        .filter_map(|key| {
            FIELDS.iter().find_map(|field| {
                let suffix = format!("_{field}");
                key.strip_suffix(&suffix).filter(|name| !name.is_empty())
            })
        })
        .collect();
    names.sort_unstable();
    names.dedup();

    if names.is_empty() {
        println!("No stored server credentials.");
        return Ok(());
    }

    for name in names {
        let stored_fields: Vec<&str> = FIELDS
            .iter()
            .filter(|field| secrets.contains_key(&field_key(name, field)))
            .copied()
            .collect();
        println!("{name}: {}", stored_fields.join(", "));
    }
    Ok(())
}
