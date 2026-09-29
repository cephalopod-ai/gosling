use anyhow::{anyhow, Result};
use chrono;
use gosling::config::Config;
use gosling::conversation::message::{Message, MessageContent, MessageMetadata};
use gosling::session::{SessionManager, SessionType};
use rmcp::model::Role;

use crate::session::{build_session, SessionBuilderConfig};

use clap::ValueEnum;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    #[value(alias = "nushell")]
    Nu,
    #[value(alias = "pwsh")]
    Powershell,
}

struct ShellConfig {
    script_template: &'static str,
    command_not_found: Option<&'static str>,
}

impl Shell {
    fn config(&self) -> &'static ShellConfig {
        match self {
            Shell::Bash => &BASH_CONFIG,
            Shell::Zsh => &ZSH_CONFIG,
            Shell::Fish => &FISH_CONFIG,
            Shell::Nu => &NU_CONFIG,
            Shell::Powershell => &POWERSHELL_CONFIG,
        }
    }

    fn quote(&self, word: &str) -> String {
        match self {
            Shell::Bash | Shell::Zsh => format!("'{}'", word.replace('\'', r"'\''")),
            Shell::Fish => format!("'{}'", word.replace('\\', r"\\").replace('\'', r"\'")),
            Shell::Nu => format!("\"{}\"", word.replace('\\', r"\\").replace('"', "\\\"")),
            Shell::Powershell => format!("'{}'", word.replace('\'', "''")),
        }
    }
}

static BASH_CONFIG: ShellConfig = ShellConfig {
    script_template: r#"export AGENT_SESSION_ID="{session_id}"
alias @gosling={gosling_run_alias}
alias @g={gosling_run_alias}

gosling_preexec() {
    [[ "$1" =~ ^gosling\ term ]] && return
    [[ "$1" =~ ^(@gosling|@g)($|[[:space:]]) ]] && return
    ({gosling_bin} term log "$1" &) 2>/dev/null
}

if [[ -z "$gosling_preexec_installed" ]]; then
    gosling_preexec_installed=1
    trap 'gosling_preexec "$BASH_COMMAND"' DEBUG
fi{command_not_found_handler}"#,
    command_not_found: Some(
        r#"

command_not_found_handle() {
    echo "🪿 Command '$1' not found. Asking gosling..."
    {gosling_bin} term run "$@"
    return 0
}"#,
    ),
};

static ZSH_CONFIG: ShellConfig = ShellConfig {
    script_template: r#"export AGENT_SESSION_ID="{session_id}"
alias @gosling={gosling_run_alias}
alias @g={gosling_run_alias}

gosling_preexec() {
    [[ "$1" =~ ^gosling\ term ]] && return
    [[ "$1" =~ ^(@gosling|@g)($|[[:space:]]) ]] && return
    ({gosling_bin} term log "$1" &) 2>/dev/null
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec gosling_preexec{command_not_found_handler}"#,
    command_not_found: Some(
        r#"

command_not_found_handler() {
    echo "🪿 Command '$1' not found. Asking gosling..."
    {gosling_bin} term run "$@"
    return 0
}"#,
    ),
};

static FISH_CONFIG: ShellConfig = ShellConfig {
    script_template: r#"set -gx AGENT_SESSION_ID "{session_id}"
function @gosling; {gosling_bin} term run $argv; end
function @g; {gosling_bin} term run $argv; end

function gosling_preexec --on-event fish_preexec
    string match -q -r '^gosling term' -- $argv[1]; and return
    string match -q -r '^(@gosling|@g)($|\s)' -- $argv[1]; and return
    {gosling_bin} term log "$argv[1]" 2>/dev/null &
end"#,
    command_not_found: None,
};

static NU_CONFIG: ShellConfig = ShellConfig {
    script_template: r#"$env.AGENT_SESSION_ID = "{session_id}"
def --wrapped @gosling [...args] { run-external {gosling_bin} "term" "run" ...$args }
def --wrapped @g [...args] { run-external {gosling_bin} "term" "run" ...$args }

if (($env | get -o GOSLING_NU_PREEXEC_INSTALLED | default false) != true) {
    $env.GOSLING_NU_PREEXEC_INSTALLED = true
    $env.config.hooks.pre_execution = (
        $env.config.hooks.pre_execution
        | append {||
            let line = (commandline | str trim)
            if ($line | is-empty) {
                return
            }
            if ($line =~ '^gosling term(\s|$)') {
                return
            }
            if ($line =~ '^(@gosling|@g)(\s|$)') {
                return
            }
            job spawn { run-external {gosling_bin} "term" "log" $line | complete | ignore } | ignore
        }
    )
}
{command_not_found_handler}"#,
    command_not_found: Some(
        r#"
$env.config.hooks.command_not_found = {|command_name|
    let prompt = (try { commandline | str trim } catch { $command_name })
    print $"🪿 Command '($command_name)' not found. Asking gosling..."
    run-external {gosling_bin} "term" "run" $prompt | complete | ignore
    null
}"#,
    ),
};

static POWERSHELL_CONFIG: ShellConfig = ShellConfig {
    script_template: r#"$env:AGENT_SESSION_ID = "{session_id}"
function @gosling { & {gosling_bin} term run @args }
function @g { & {gosling_bin} term run @args }

Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {
    $line = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$null)
    if ($line -notmatch '^gosling term' -and $line -notmatch '^(@gosling|@g)($|\s)') {
        Start-Job -ScriptBlock { & {gosling_bin} term log $using:line } | Out-Null
    }
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
}"#,
    command_not_found: None,
};

fn render_term_init_script(
    shell: Shell,
    session_id: &str,
    gosling_bin: &str,
    with_command_not_found: bool,
) -> String {
    let config = shell.config();
    let gosling_bin = shell.quote(gosling_bin);
    let gosling_run_alias = shell.quote(&format!("{gosling_bin} term run"));
    let command_not_found_handler = if with_command_not_found {
        config
            .command_not_found
            .map(|handler| handler.replace("{gosling_bin}", &gosling_bin))
            .unwrap_or_default()
    } else {
        String::new()
    };

    config
        .script_template
        .replace("{session_id}", session_id)
        .replace("{gosling_run_alias}", &gosling_run_alias)
        .replace("{gosling_bin}", &gosling_bin)
        .replace("{command_not_found_handler}", &command_not_found_handler)
}

pub async fn handle_term_init(
    shell: Shell,
    name: Option<String>,
    with_command_not_found: bool,
) -> Result<()> {
    let session_manager = SessionManager::instance();

    let working_dir = std::env::current_dir()?;
    let named_session = if let Some(ref name) = name {
        let sessions = session_manager
            .list_sessions_by_types(&[SessionType::Terminal])
            .await?;
        sessions.into_iter().find(|s| s.name == *name)
    } else {
        None
    };

    let session = match named_session {
        Some(s) => s,
        None => {
            let session = session_manager
                .create_session(
                    working_dir,
                    "Gosling Term Session".to_string(),
                    SessionType::Terminal,
                    Config::global().effective_gosling_mode(),
                )
                .await?;

            if let Some(name) = name {
                session_manager
                    .update(&session.id)
                    .user_provided_name(name)
                    .apply()
                    .await?;
            }

            session
        }
    };

    let gosling_bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "gosling".to_string());

    println!(
        "{}",
        render_term_init_script(shell, &session.id, &gosling_bin, with_command_not_found)
    );
    Ok(())
}

pub async fn handle_term_log(command: String) -> Result<()> {
    let session_id = std::env::var("AGENT_SESSION_ID").map_err(|_| {
        anyhow!(
            "AGENT_SESSION_ID not set. Initialize terminal integration with `gosling term init <shell>` and reload your shell first."
        )
    })?;

    let message = Message::new(
        Role::User,
        chrono::Utc::now().timestamp_millis(),
        vec![MessageContent::text(command)],
    )
    .with_metadata(MessageMetadata::user_only())
    .with_generated_id();

    let session_manager = SessionManager::instance();
    session_manager.add_message(&session_id, &message).await?;

    Ok(())
}

pub async fn handle_term_run(prompt: Vec<String>) -> Result<()> {
    let prompt = prompt.join(" ");
    let session_id = std::env::var("AGENT_SESSION_ID").map_err(|_| {
        anyhow!(
            "AGENT_SESSION_ID not set.\n\n\
             Initialize terminal integration with `gosling term init <shell>` in your shell profile, \
             then restart or reload that shell."
        )
    })?;

    let working_dir = std::env::current_dir()?;
    let session_manager = SessionManager::instance();

    session_manager
        .update(&session_id)
        .working_dir(working_dir)
        .apply()
        .await?;

    let session = session_manager.get_session(&session_id, true).await?;
    let user_messages_after_last_assistant: Vec<&Message> =
        if let Some(conv) = &session.conversation {
            conv.messages()
                .iter()
                .rev()
                .take_while(|m| m.role != Role::Assistant)
                .collect()
        } else {
            Vec::new()
        };

    if let Some(oldest_user) = user_messages_after_last_assistant.last() {
        if let Some(message_id) = oldest_user.id.as_deref() {
            session_manager
                .truncate_conversation_from_message(&session_id, message_id)
                .await?;
        } else {
            session_manager
                .truncate_conversation(&session_id, oldest_user.created)
                .await?;
        }
    }

    let prompt_with_context = if user_messages_after_last_assistant.is_empty() {
        prompt
    } else {
        let history = user_messages_after_last_assistant
            .iter()
            .rev() // back to chronological order
            .map(|m| m.as_concat_text())
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "<shell_history>\n{}\n</shell_history>\n\n{}",
            history, prompt
        )
    };

    let config = SessionBuilderConfig {
        session_id: Some(session_id),
        resume: true,
        interactive: false,
        quiet: true,
        ..Default::default()
    };

    let mut session = build_session(config).await;
    session.headless(prompt_with_context).await?;

    Ok(())
}

/// Handle `gosling term info` - print compact session info for prompt integration
pub async fn handle_term_info() -> Result<()> {
    let session_id = match std::env::var("AGENT_SESSION_ID") {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };

    let session_manager = SessionManager::instance();
    let session = session_manager.get_session(&session_id, false).await.ok();
    let total_tokens = session
        .as_ref()
        .and_then(|s| s.usage.total_tokens)
        .unwrap_or(0) as usize;

    let config = gosling::config::Config::global();
    let model_name = config
        .get_gosling_model()
        .ok()
        .map(|name| {
            let short = name.rsplit('/').next().unwrap_or(&name);
            if let Some(stripped) = short.strip_prefix("gosling-") {
                stripped.to_string()
            } else {
                short.to_string()
            }
        })
        .unwrap_or_else(|| "?".to_string());

    let context_limit = config
        .get_gosling_model()
        .ok()
        .and_then(|model_name| {
            config
                .get_gosling_provider()
                .ok()
                .and_then(|provider_name| {
                    gosling::model_config::model_config_from_user_config(
                        &provider_name,
                        &model_name,
                    )
                    .ok()
                })
        })
        .map(|mc| mc.context_limit())
        .unwrap_or(128_000);

    let percentage = if context_limit > 0 {
        ((total_tokens as f64 / context_limit as f64) * 100.0).round() as usize
    } else {
        0
    };

    let filled = (percentage / 20).min(5);
    let empty = 5 - filled;
    let dots = format!("{}{}", "●".repeat(filled), "○".repeat(empty));

    println!("{} {}", dots, model_name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_term_init_script_includes_nushell_hooks() {
        let script = render_term_init_script(Shell::Nu, "session-123", "/tmp/gosling", false);

        assert!(script.contains("$env.AGENT_SESSION_ID = \"session-123\""));
        assert!(script.contains("def --wrapped @gosling [...args]"));
        assert!(script.contains("def --wrapped @g [...args]"));
        assert!(script.contains("GOSLING_NU_PREEXEC_INSTALLED"));
        assert!(script.contains("$env.config.hooks.pre_execution"));
        assert!(script.contains("job spawn { run-external \"/tmp/gosling\" \"term\" \"log\" $line | complete | ignore } | ignore"));
        assert!(!script.contains("command_not_found = {|command_name|"));
    }

    #[test]
    fn render_term_init_script_includes_nushell_default_handler() {
        let script = render_term_init_script(Shell::Nu, "session-123", "/tmp/gosling", true);

        assert!(script.contains("$env.config.hooks.command_not_found = {|command_name|"));
        assert!(script.contains(
            "run-external \"/tmp/gosling\" \"term\" \"run\" $prompt | complete | ignore"
        ));
    }

    #[test]
    fn render_term_init_script_skips_unsupported_default_handler() {
        let script = render_term_init_script(Shell::Fish, "session-123", "/tmp/gosling", true);

        assert!(!script.contains("command_not_found"));
    }

    // GSL-PT-20260927-D15: a gosling binary path with spaces or quotes must reach the shell as
    // one word.
    #[cfg(unix)]
    fn run_alias_in(shell: &str, args: &[&str]) -> Option<String> {
        use std::os::unix::fs::PermissionsExt;

        if !std::path::Path::new(shell).exists() {
            return None;
        }
        let dir = tempfile::TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin dir's");
        std::fs::create_dir(&bin_dir).unwrap();
        let bin = bin_dir.join("gosling");
        std::fs::write(
            &bin,
            "#!/bin/sh\n[ \"$2\" = run ] && echo \"ran: $*\"\nexit 0\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();

        let kind = if shell.ends_with("zsh") {
            Shell::Zsh
        } else {
            Shell::Bash
        };
        let script = render_term_init_script(kind, "session-123", bin.to_str().unwrap(), false);
        let script_path = dir.path().join("init.sh");
        std::fs::write(&script_path, script).unwrap();

        let output = std::process::Command::new(shell)
            .args(args)
            .arg("-c")
            .arg("shopt -s expand_aliases 2>/dev/null; eval \"$(cat \"$1\")\"; eval '@g hello world'")
            .arg("sh")
            .arg(&script_path)
            .env("HOME", dir.path())
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        Some(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }

    #[cfg(unix)]
    #[test]
    fn bash_and_zsh_aliases_survive_spaces_and_quotes_in_the_binary_path() {
        for (shell, args) in [
            ("/bin/bash", &["--noprofile", "--norc"][..]),
            ("/bin/zsh", &["-f"][..]),
        ] {
            let Some(output) = run_alias_in(shell, args) else {
                continue;
            };
            assert!(
                output.contains("ran: term run hello world"),
                "{shell}: {output}"
            );
        }
    }

    #[test]
    fn shell_scripts_quote_the_binary_path_as_one_word() {
        let bin = "/opt/my tools/it's/gosling";

        let bash = render_term_init_script(Shell::Bash, "s", bin, true);
        assert!(
            bash.contains(r#"alias @g=''\''/opt/my tools/it'\''\'\'''\''s/gosling'\'' term run'"#)
        );
        assert!(bash.contains(r#"('/opt/my tools/it'\''s/gosling' term log "$1" &)"#));
        assert!(bash.contains(r#"    '/opt/my tools/it'\''s/gosling' term run "$@""#));

        let nu =
            render_term_init_script(Shell::Nu, "s", r#"C:\Program Files\"g"\gosling.exe"#, true);
        assert!(nu.contains(
            r#"run-external "C:\\Program Files\\\"g\"\\gosling.exe" "term" "run" ...$args"#
        ));

        let fish = render_term_init_script(Shell::Fish, "s", bin, false);
        assert!(fish.contains(r#"function @g; '/opt/my tools/it\'s/gosling' term run $argv; end"#));

        let powershell = render_term_init_script(Shell::Powershell, "s", bin, false);
        assert!(powershell.contains("& '/opt/my tools/it''s/gosling' term run @args"));
    }

    // PowerShell treats `{ ... }` as a scriptblock literal, so a body written as
    // `{{ & gosling ... }}` makes the function return an inner scriptblock instead of running
    // gosling, and the Enter key handler would never call AcceptLine.
    #[test]
    fn powershell_script_uses_single_braces_for_blocks() {
        let script = render_term_init_script(Shell::Powershell, "s-1", "/opt/gosling", false);

        assert!(!script.contains("{{") && !script.contains("}}"), "{script}");
        assert_eq!(
            script.matches('{').count(),
            script.matches('}').count(),
            "{script}"
        );
        for line in [
            "$env:AGENT_SESSION_ID = \"s-1\"",
            "function @gosling { & '/opt/gosling' term run @args }",
            "function @g { & '/opt/gosling' term run @args }",
            "Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {",
            "    if ($line -notmatch '^gosling term' -and $line -notmatch '^(@gosling|@g)($|\\s)') {",
            "        Start-Job -ScriptBlock { & '/opt/gosling' term log $using:line } | Out-Null",
            "    }",
            "    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()",
            "}",
        ] {
            assert!(
                script.lines().any(|l| l == line),
                "missing line {line:?} in:\n{script}"
            );
        }
    }
}
