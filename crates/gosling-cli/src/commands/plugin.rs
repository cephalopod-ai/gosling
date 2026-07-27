use std::path::Path;

use anyhow::Result;
use console::style;

pub fn handle_plugin_install(url: &str, auto_update: bool) -> Result<()> {
    let install = gosling::plugins::install_plugin_with_options(
        url,
        gosling::plugins::PluginInstallOptions { auto_update },
    )?;

    println!(
        "{} Installed {} plugin '{}' ({})",
        style("✓").green(),
        install.format,
        style(&install.name).bold(),
        install.version
    );
    print_plugin_install(&install);

    Ok(())
}

pub fn handle_plugin_update(name: &str) -> Result<()> {
    let install = gosling::plugins::update_plugin(name)?;

    println!(
        "{} Updated {} plugin '{}' ({})",
        style("✓").green(),
        install.format,
        style(&install.name).bold(),
        install.version
    );
    print_plugin_install(&install);

    Ok(())
}

pub fn handle_plugin_trust(path: &Path) -> Result<()> {
    let project_root = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let enabled = gosling::plugins::discovery::trust_project(&project_root)?;

    if enabled.is_empty() {
        println!(
            "{} No plugins under {} request to be enabled; nothing to trust.",
            style("i").blue(),
            project_root.display()
        );
        return Ok(());
    }

    println!(
        "{} Trusted {} ({} plugin{} can now run hooks/MCP servers):",
        style("✓").green(),
        project_root.display(),
        enabled.len(),
        if enabled.len() == 1 { "" } else { "s" }
    );
    for name in &enabled {
        println!("  - {name}");
    }

    Ok(())
}

fn print_plugin_install(install: &gosling::plugins::PluginInstall) {
    println!("  Source: {}", install.source);
    println!("  Location: {}", install.directory.display());

    if install.skills.is_empty() {
        println!("  No skills imported.");
    } else {
        println!("  Imported skills:");
        for skill in &install.skills {
            println!("    - {}", skill.name);
        }
    }
}
