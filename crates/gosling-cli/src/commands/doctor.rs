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
    format!(
        "Gosling Doctor\n\n{system_info}\nConfig file: {}\nProvider: {}\nModel: {}\nStatus: local diagnostics complete",
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
        assert!(report.ends_with("Status: local diagnostics complete"));
        assert!(!report.contains("/doctor"));
    }
}
