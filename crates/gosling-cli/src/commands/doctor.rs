use anyhow::Result;
use std::path::Path;

use gosling::config::Config;
use gosling::session::{config_path, SystemInfo};

pub async fn handle_doctor() -> Result<()> {
    let config = Config::global();
    let system_info = SystemInfo::collect().to_text();
    let report = render_report(
        &system_info,
        &config_path(),
        config.get_gosling_provider().ok().as_deref(),
        config.get_gosling_model().ok().as_deref(),
    );
    println!("{report}");
    Ok(())
}

fn render_report(
    system_info: &str,
    config_file: &Path,
    provider: Option<&str>,
    model: Option<&str>,
) -> String {
    // "local diagnostics complete" read as a verdict on the whole setup even
    // though nothing here contacts the provider, so a broken key still exited
    // 0 with a reassuring line. Report only what was actually inspected, and
    // say plainly what was not. (REL-GSL-011)
    let status = match (provider, model) {
        (Some(_), Some(_)) => {
            "Status: configuration present (not verified — no provider request was made)"
        }
        (None, _) => "Status: no provider configured",
        (Some(_), None) => "Status: provider configured but no model selected",
    };

    format!(
        "Gosling Doctor\n\n{system_info}\nConfig file: {}\nProvider: {}\nModel: {}\n{status}",
        config_file.display(),
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
            Some("ollama"),
            Some("qwen2.5:latest"),
        );

        assert!(report.contains("Gosling Doctor"));
        assert!(report.contains("Provider: ollama"));
        assert!(report.contains("Model: qwen2.5:latest"));
        assert!(!report.contains("/doctor"));
    }

    #[test]
    fn configured_setup_is_not_reported_as_verified() {
        let report = render_report("info", Path::new("/tmp/config.yaml"), Some("p"), Some("m"));
        assert!(report.contains("not verified"));
        assert!(!report.contains("diagnostics complete"));
    }

    #[test]
    fn missing_provider_and_model_are_named() {
        assert!(render_report("i", Path::new("/c"), None, None).contains("no provider configured"));
        assert!(render_report("i", Path::new("/c"), Some("p"), None).contains("no model selected"));
    }
}
