use anyhow::Result;
use std::path::Path;

use crate::commands::configure::unparsable_config_problem;
use gosling::config::{Config, PermissionManager};
use gosling::providers::get_from_registry;
use gosling::providers::provider_test::test_provider_configuration;
use gosling::session::{config_path, SystemInfo};

pub async fn handle_doctor() -> Result<()> {
    let config = Config::global();
    let system_info = SystemInfo::collect().to_text();
    let provider = config.get_gosling_provider().ok();
    let model = config.get_gosling_model().ok();
    let config_problem = unparsable_config_problem(config);
    let problem = setup_problem(
        provider.as_deref(),
        model.as_deref(),
        config_problem.as_deref(),
    )
    .await;
    let policy_problem = PermissionManager::instance().policy_problem();
    let mut report = render_report(
        &system_info,
        &config_path(),
        config_problem.is_some(),
        provider.as_deref(),
        model.as_deref(),
        problem.is_none(),
    );
    if let Some(policy_problem) = &policy_problem {
        report.push_str(&format!("\n{policy_problem}"));
    }
    println!("{report}");
    problem
        .or(policy_problem)
        .map_or(Ok(()), |problem| Err(anyhow::anyhow!(problem)))
}

async fn setup_problem(
    provider: Option<&str>,
    model: Option<&str>,
    config_problem: Option<&str>,
) -> Option<String> {
    let Some(provider) = provider else {
        return Some(match config_problem {
            Some(problem) => format!("{problem}, then run 'gosling doctor' again."),
            None => "no provider configured. Run 'gosling configure' first.".to_string(),
        });
    };
    if let Err(e) = get_from_registry(provider).await {
        return Some(e.to_string());
    }
    let Some(model) = model else {
        return Some("no model configured. Run 'gosling configure' first.".to_string());
    };
    test_provider_configuration(provider, model, false, None)
        .await
        .err()
        .map(|error| format!("provider check failed for {provider}/{model}: {error}"))
}

fn render_report(
    system_info: &str,
    config_file: &Path,
    config_unparsable: bool,
    provider: Option<&str>,
    model: Option<&str>,
    provider_verified: bool,
) -> String {
    let status = match (provider, model) {
        (Some(_), Some(_)) if provider_verified => "Status: provider request verified",
        (Some(_), Some(_)) => "Status: provider check failed",
        (None, _) if config_unparsable => "Status: config file could not be parsed",
        (None, _) => "Status: no provider configured",
        (Some(_), None) => "Status: provider configured but no model selected",
    };

    format!(
        "Gosling Doctor\n\n{system_info}\nConfig file: {}{}\nProvider: {}\nModel: {}\n{status}",
        config_file.display(),
        if config_unparsable {
            " (could not be parsed)"
        } else {
            ""
        },
        provider.unwrap_or("not configured"),
        model.unwrap_or("not configured")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_report_is_bounded_and_non_interactive() {
        let report = render_report(
            "OS: test",
            Path::new("/tmp/config.yaml"),
            false,
            Some("ollama"),
            Some("qwen2.5:latest"),
            true,
        );

        assert!(report.contains("Gosling Doctor"));
        assert!(report.contains("Provider: ollama"));
        assert!(report.contains("Model: qwen2.5:latest"));
        assert!(!report.contains("/doctor"));
    }

    #[test]
    fn configured_setup_reports_the_probe_result() {
        let verified = render_report(
            "info",
            Path::new("/tmp/config.yaml"),
            false,
            Some("p"),
            Some("m"),
            true,
        );
        let failed = render_report(
            "info",
            Path::new("/tmp/config.yaml"),
            false,
            Some("p"),
            Some("m"),
            false,
        );
        assert!(verified.contains("provider request verified"));
        assert!(failed.contains("provider check failed"));
    }

    #[test]
    fn missing_provider_and_model_are_named() {
        assert!(
            render_report("i", Path::new("/c"), false, None, None, false)
                .contains("no provider configured")
        );
        assert!(
            render_report("i", Path::new("/c"), false, Some("p"), None, false)
                .contains("no model selected")
        );
    }

    #[test]
    fn an_unparsable_config_file_is_named_instead_of_a_missing_provider() {
        let report = render_report("i", Path::new("/c/config.yaml"), true, None, None, false);
        assert!(report.contains("Config file: /c/config.yaml (could not be parsed)"));
        assert!(report.contains("Status: config file could not be parsed"));
        assert!(!report.contains("no provider configured"));
    }
}
