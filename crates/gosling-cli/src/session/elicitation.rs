use console::style;
use rmcp::model::ElicitationAction;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, BufRead, IsTerminal, Write};

pub struct ElicitationInput {
    pub action: ElicitationAction,
    pub user_data: HashMap<String, Value>,
}

pub fn collect_elicitation_input(message: &str, schema: &Value) -> io::Result<ElicitationInput> {
    if !message.is_empty() {
        println!("\n{}", style(message).cyan());
    }

    let properties = schema.get("properties").and_then(|p| p.as_object());

    // Schema-less (or empty-schema) elicitations are pure approval prompts —
    // offer an explicit Y/N confirmation instead of silently auto-accepting.
    let properties = match properties {
        Some(props) if !props.is_empty() => props,
        _ => {
            let prompt = if message.is_empty() {
                "Approve this action?"
            } else {
                "Approve?"
            };
            return match cliclack::confirm(prompt).initial_value(true).interact() {
                Ok(true) => Ok(ElicitationInput {
                    action: ElicitationAction::Accept,
                    user_data: HashMap::new(),
                }),
                Ok(false) => Ok(ElicitationInput {
                    action: ElicitationAction::Decline,
                    user_data: HashMap::new(),
                }),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(ElicitationInput {
                    action: ElicitationAction::Cancel,
                    user_data: HashMap::new(),
                }),
                Err(e) => Err(e),
            };
        }
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut data: HashMap<String, Value> = HashMap::new();

    for (name, field_schema) in properties {
        let is_required = required.contains(&name.as_str());
        let field_type = field_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        let label = field_schema
            .get("title")
            .and_then(|t| t.as_str())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(name);
        let description = field_schema.get("description").and_then(|d| d.as_str());
        let default = field_schema.get("default");
        let enum_values = field_schema
            .get("enum")
            .or_else(|| {
                field_schema
                    .get("items")
                    .and_then(|items| items.get("enum"))
            })
            .and_then(|e| e.as_array());

        // makes a little true/false toggle
        if field_type == "boolean" {
            let label = match description {
                Some(desc) => format!("{} ({})", label, desc),
                None => label.to_string(),
            };
            let default_bool = default.and_then(|v| v.as_bool()).unwrap_or(false);

            match cliclack::confirm(&label)
                .initial_value(default_bool)
                .interact()
            {
                Ok(v) => {
                    data.insert(name.clone(), Value::Bool(v));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    return Ok(ElicitationInput {
                        action: ElicitationAction::Cancel,
                        user_data: HashMap::new(),
                    });
                }
                Err(e) => return Err(e),
            }
            continue;
        }

        if let Some(desc) = description {
            println!("{}", style(desc).dim());
        }
        if let Some(options) = enum_values {
            let opts: Vec<String> = options
                .iter()
                .filter_map(|v| v.as_str())
                .enumerate()
                .map(|(index, option)| format!("{}. {}", index + 1, option))
                .collect();
            let hint = if field_type == "array" {
                "Options (comma-separated numbers or names)"
            } else {
                "Options (number or name)"
            };
            println!("  {}: {}", style(hint).dim(), opts.join(", "));
        }

        print!("{}", style(label).yellow());
        if is_required {
            print!("{}", style("*").red());
        }
        if let Some(def) = default {
            print!(" {}", style(format!("[{}]", format_default(def))).dim());
        }
        print!(": ");
        io::stdout().flush()?;

        let input = read_line()?;

        // Handle Ctrl+C / EOF for cancellation
        if input.is_none() {
            return Ok(ElicitationInput {
                action: ElicitationAction::Cancel,
                user_data: HashMap::new(),
            });
        }
        let input = input.unwrap();

        let value = if input.is_empty() {
            default.cloned()
        } else {
            Some(parse_value(&input, field_type, enum_values))
        };

        if let Some(v) = value {
            if !v.is_null() {
                data.insert(name.clone(), v);
            }
        }

        if is_required && !data.contains_key(name) {
            println!(
                "{}",
                style(format!("Required field '{}' is missing", name)).red()
            );
            return Ok(ElicitationInput {
                action: ElicitationAction::Decline,
                user_data: HashMap::new(),
            });
        }
    }

    println!();
    Ok(ElicitationInput {
        action: ElicitationAction::Accept,
        user_data: data,
    })
}

fn read_line() -> io::Result<Option<String>> {
    if !std::io::stdin().is_terminal() {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        return Ok(Some(line.trim().to_string()));
    }

    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) => Ok(None), // EOF
        Ok(_) => Ok(Some(line.trim().to_string())),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(e) => Err(e),
    }
}

fn format_default(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        _ => value.to_string(),
    }
}

fn parse_option(input: &str, valid: &[&str]) -> Option<String> {
    if valid.contains(&input) {
        return Some(input.to_string());
    }
    let idx = input.parse::<usize>().ok()?;
    (idx > 0 && idx <= valid.len()).then(|| valid[idx - 1].to_string())
}

fn parse_value(input: &str, field_type: &str, enum_values: Option<&Vec<Value>>) -> Value {
    if let Some(options) = enum_values {
        let valid: Vec<&str> = options.iter().filter_map(|v| v.as_str()).collect();
        if field_type == "array" {
            let chosen: Vec<Value> = input
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .filter_map(|part| parse_option(part, &valid))
                .map(Value::String)
                .collect();
            return if chosen.is_empty() {
                Value::Null
            } else {
                Value::Array(chosen)
            };
        }
        if let Some(choice) = parse_option(input, &valid) {
            return Value::String(choice);
        }
    }

    match field_type {
        "boolean" => {
            let lower = input.to_lowercase();
            Value::Bool(matches!(lower.as_str(), "true" | "yes" | "y" | "1"))
        }
        "integer" => input
            .parse::<i64>()
            .map(|n| Value::Number(n.into()))
            .unwrap_or(Value::Null),
        "number" => input
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        _ => Value::String(input.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_value_accepts_option_names_and_numbers() {
        let options = vec![json!("Summary"), json!("Detailed")];
        assert_eq!(
            parse_value("2", "string", Some(&options)),
            json!("Detailed")
        );
        assert_eq!(
            parse_value("Summary", "string", Some(&options)),
            json!("Summary")
        );
        assert_eq!(
            parse_value("other", "string", Some(&options)),
            json!("other")
        );
    }

    #[test]
    fn parse_value_collects_multi_select_answers_into_an_array() {
        let options = vec![json!("Introduction"), json!("Conclusion")];
        assert_eq!(
            parse_value("1, Conclusion", "array", Some(&options)),
            json!(["Introduction", "Conclusion"])
        );
        assert_eq!(parse_value("nope", "array", Some(&options)), Value::Null);
    }
}
