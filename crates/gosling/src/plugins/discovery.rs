use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::plugins::plugin_install_dir;

const PLUGINS_CONFIG_KEY: &str = "plugins";

/// Per-plugin entry stored under the `plugins` map in `config.yaml`, keyed by
/// the plugin's filesystem path.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginConfigEntry {
    enabled: bool,
    #[serde(default)]
    trusted: bool,
}

/// A plugin found on disk and not disabled by any settings file.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub name: String,
    pub root: PathBuf,
    pub scope: PluginScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginScope {
    User,
    Project,
}

/// Settings file format from <https://open-plugins.com/plugin-builders/installation>.
#[derive(Debug, Default, Deserialize)]
struct PluginSettings {
    #[serde(default, rename = "enabledPlugins")]
    enabled: Vec<String>,
    #[serde(default, rename = "disabledPlugins")]
    disabled: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SettingsScope {
    Local,
    Project,
    User,
}

/// Discover all plugins that should be considered active.
///
/// `project_root`, when supplied, enables project + local scope settings and
/// project-scope `.agents/plugins/` lookups.
pub fn discover_enabled_plugins(project_root: Option<&Path>) -> Vec<DiscoveredPlugin> {
    discover_enabled_plugins_with_config(project_root, Config::global())
}

fn discover_enabled_plugins_with_config(
    project_root: Option<&Path>,
    config: &Config,
) -> Vec<DiscoveredPlugin> {
    let scoped_settings = load_all_settings(project_root);
    let mut found: HashMap<String, DiscoveredPlugin> = HashMap::new();

    if let Some(root) = project_root {
        for (name, root) in list_dir_children(&project_plugin_dir(root)) {
            found.entry(name.clone()).or_insert(DiscoveredPlugin {
                name,
                root,
                scope: PluginScope::Project,
            });
        }
    }
    for (name, root) in list_dir_children(&plugin_install_dir()) {
        found.entry(name.clone()).or_insert(DiscoveredPlugin {
            name,
            root,
            scope: PluginScope::User,
        });
    }

    let enabled_by_settings: Vec<DiscoveredPlugin> = found
        .into_values()
        .filter(|plugin| settings_state(&plugin.name, &scoped_settings) != Some(false))
        .collect();

    filter_by_config(enabled_by_settings, config, &scoped_settings, project_root)
}

/// Apply the `plugins` map in `config.yaml`. Newly discovered user plugins stay
/// enabled for compatibility (installing a user plugin is already an explicit
/// action via `gosling plugin install`). Project plugins are never enabled or
/// marked `trusted` from repo-shipped content alone (SEC-GSL-101): a
/// project-scope plugin's own `settings.json`/`settings.local.json` can only
/// *disable* it, never establish trust. Trust for a project's plugins is set
/// exclusively by [`trust_project`], called from an explicit, out-of-repo user
/// action (e.g. the `gosling plugin trust` CLI command).
fn filter_by_config(
    plugins: Vec<DiscoveredPlugin>,
    config: &Config,
    scoped_settings: &[(SettingsScope, PluginSettings)],
    project_root: Option<&Path>,
) -> Vec<DiscoveredPlugin> {
    let mut entries: HashMap<String, PluginConfigEntry> =
        config.get_param(PLUGINS_CONFIG_KEY).unwrap_or_default();

    let mut dirty = false;
    let mut enabled = Vec::new();
    let mut untrusted_pending = Vec::new();
    for plugin in plugins {
        let key = plugin.root.to_string_lossy().to_string();
        match entries.get(&key) {
            Some(entry) => {
                // `trusted` is only ever flipped true by `trust_project`; it
                // must never be re-derived here from the current
                // settings.json content, or a repo could re-win trust simply
                // by re-shipping `enabledPlugins`.
                let trusted = plugin.scope == PluginScope::User || entry.trusted;
                if entry.enabled && trusted {
                    enabled.push(plugin);
                } else if !trusted && settings_state(&plugin.name, scoped_settings) == Some(true) {
                    untrusted_pending.push(plugin.name);
                }
            }
            None => {
                let repo_requests_enable = match plugin.scope {
                    PluginScope::User => true,
                    PluginScope::Project => {
                        settings_state(&plugin.name, scoped_settings) == Some(true)
                    }
                };
                let trusted = plugin.scope == PluginScope::User;
                let is_enabled = trusted && repo_requests_enable;
                entries.insert(
                    key,
                    PluginConfigEntry {
                        enabled: is_enabled,
                        trusted,
                    },
                );
                dirty = true;
                if is_enabled {
                    enabled.push(plugin);
                } else if plugin.scope == PluginScope::Project && repo_requests_enable {
                    untrusted_pending.push(plugin.name);
                }
            }
        }
    }

    if dirty {
        if let Err(e) = config.set_param(PLUGINS_CONFIG_KEY, entries) {
            tracing::warn!(error = %e, "Failed to persist plugin config entries");
        }
    }

    if !untrusted_pending.is_empty() {
        tracing::warn!(
            plugins = ?untrusted_pending,
            project = ?project_root,
            "project plugin(s) request to run hooks/MCP servers but this project is not \
             trusted; run `gosling plugin trust` after reviewing them to allow it",
        );
    }

    enabled
}

/// Marks every plugin currently discovered under `project_root`'s
/// `.agents/plugins/` directory as trusted, applying the project's own
/// `settings.json`/`settings.local.json` to decide which of them become
/// enabled. This is the only place `PluginConfigEntry::trusted` is set true
/// for a project-scope plugin, and it must only be called from an explicit
/// user action (SEC-GSL-101) — never automatically while discovering or
/// loading a session.
pub fn trust_project(project_root: &Path) -> anyhow::Result<Vec<String>> {
    trust_project_with_config(project_root, Config::global())
}

fn trust_project_with_config(project_root: &Path, config: &Config) -> anyhow::Result<Vec<String>> {
    let scoped_settings = load_all_settings(Some(project_root));
    let discovered = list_dir_children(&project_plugin_dir(project_root));
    let mut entries: HashMap<String, PluginConfigEntry> =
        config.get_param(PLUGINS_CONFIG_KEY).unwrap_or_default();

    let mut newly_enabled = Vec::new();
    for (name, root) in discovered {
        let key = root.to_string_lossy().to_string();
        let repo_requests_enable = settings_state(&name, &scoped_settings) == Some(true);
        let entry = entries.entry(key).or_insert(PluginConfigEntry {
            enabled: false,
            trusted: false,
        });
        entry.trusted = true;
        if repo_requests_enable {
            entry.enabled = true;
        }
        if entry.enabled {
            newly_enabled.push(name);
        }
    }

    config.set_param(PLUGINS_CONFIG_KEY, entries)?;
    Ok(newly_enabled)
}

fn settings_state(
    plugin_name: &str,
    scoped_settings: &[(SettingsScope, PluginSettings)],
) -> Option<bool> {
    for scope in [
        SettingsScope::Local,
        SettingsScope::Project,
        SettingsScope::User,
    ] {
        let Some(settings) = scoped_settings
            .iter()
            .find_map(|(s, settings)| (*s == scope).then_some(settings))
        else {
            continue;
        };

        let listed_disabled = settings.disabled.iter().any(|n| n == plugin_name);
        let listed_enabled = settings.enabled.iter().any(|n| n == plugin_name);

        if listed_disabled {
            return Some(false);
        }
        if listed_enabled {
            return Some(true);
        }
    }

    None
}

fn project_plugin_dir(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("plugins")
}

fn list_dir_children(dir: &Path) -> Vec<(String, PathBuf)> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            Some((name, path))
        })
        .collect()
}

fn load_all_settings(project_root: Option<&Path>) -> Vec<(SettingsScope, PluginSettings)> {
    let mut paths: Vec<(SettingsScope, PathBuf)> = Vec::new();
    if let Some(path) = user_settings_path() {
        paths.push((SettingsScope::User, path));
    }
    if let Some(root) = project_root {
        paths.push((SettingsScope::Project, project_settings_path(root, false)));
        paths.push((SettingsScope::Local, project_settings_path(root, true)));
    }

    paths
        .into_iter()
        .filter_map(|(scope, path)| match read_settings(&path) {
            Ok(Some(s)) => Some((scope, s)),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to read plugin settings");
                None
            }
        })
        .collect()
}

fn user_settings_path() -> Option<PathBuf> {
    if let Ok(test_root) = std::env::var("GOSLING_PATH_ROOT") {
        return Some(
            PathBuf::from(test_root)
                .join(".config")
                .join("gosling")
                .join("settings.json"),
        );
    }
    Some(
        dirs::home_dir()?
            .join(".config")
            .join("gosling")
            .join("settings.json"),
    )
}

fn project_settings_path(project_root: &Path, local: bool) -> PathBuf {
    let file = if local {
        "settings.local.json"
    } else {
        "settings.json"
    };
    project_root.join(".config").join("gosling").join(file)
}

fn read_settings(path: &Path) -> anyhow::Result<Option<PluginSettings>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    let parsed: PluginSettings = serde_json::from_str(&text)?;
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin_dir(root: &Path, name: &str) {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("hooks")).unwrap();
        std::fs::write(
            dir.join("hooks").join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"hooks":[]}]}}"#,
        )
        .unwrap();
    }

    fn write_settings(dir: &Path, contents: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("settings.json"), contents).unwrap();
    }

    fn write_local_settings(dir: &Path, contents: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("settings.local.json"), contents).unwrap();
    }

    fn test_config(dir: &Path) -> Config {
        Config::new(dir.join("config.yaml"), "gosling-discovery-test").unwrap()
    }

    fn discover(project: &Path) -> Vec<DiscoveredPlugin> {
        let cfg_dir = tempfile::tempdir().unwrap();
        discover_enabled_plugins_with_config(Some(project), &test_config(cfg_dir.path()))
    }

    #[test]
    fn project_scope_plugin_requires_explicit_enable() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        let found = discover(project);
        assert!(found.iter().all(|p| p.name != "demo"), "got: {found:?}");
    }

    // SEC-GSL-101 regression: a repo-shipped settings.json alone must never be
    // sufficient to run a project's plugin hooks/MCP servers, even though the
    // plugin is otherwise "explicitly enabled" per the settings file.
    #[test]
    fn explicit_enabled_project_plugin_does_not_load_without_trust() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");
        write_settings(
            &project.join(".config").join("gosling"),
            r#"{"enabledPlugins":["demo"]}"#,
        );

        let found = discover(project);
        assert!(
            found.iter().all(|p| p.name != "demo"),
            "an untrusted project's settings.json must not auto-enable a plugin; got: {found:?}"
        );
    }

    #[test]
    fn trusting_project_enables_settings_json_requested_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");
        write_settings(
            &project.join(".config").join("gosling"),
            r#"{"enabledPlugins":["demo"]}"#,
        );

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());

        // Before trust: still refused, exactly like the untrusted case above.
        let found = discover_enabled_plugins_with_config(Some(project), &config);
        assert!(found.iter().all(|p| p.name != "demo"));

        let newly_enabled = trust_project_with_config(project, &config).unwrap();
        assert_eq!(newly_enabled, vec!["demo".to_string()]);

        // After the explicit trust action, the settings.json request now applies.
        let found = discover_enabled_plugins_with_config(Some(project), &config);
        let demo = found.iter().find(|p| p.name == "demo").unwrap();
        assert_eq!(demo.scope, PluginScope::Project);
    }

    #[test]
    fn trusting_project_does_not_enable_plugin_settings_do_not_request() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());

        let newly_enabled = trust_project_with_config(project, &config).unwrap();
        assert!(
            newly_enabled.is_empty(),
            "trust alone should not enable a plugin settings.json never asked for"
        );

        let found = discover_enabled_plugins_with_config(Some(project), &config);
        assert!(found.iter().all(|p| p.name != "demo"));
    }

    #[test]
    fn disabled_in_project_settings_drops_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        write_settings(
            &project.join(".config").join("gosling"),
            r#"{"disabledPlugins":["demo"]}"#,
        );

        let found = discover(project);
        assert!(found.iter().all(|p| p.name != "demo"));
    }

    #[test]
    fn explicit_enabled_filters_out_unlisted_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");
        write_plugin_dir(&project.join(".agents").join("plugins"), "other");

        write_settings(
            &project.join(".config").join("gosling"),
            r#"{"enabledPlugins":["demo"]}"#,
        );

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());
        trust_project_with_config(project, &config).unwrap();

        let found = discover_enabled_plugins_with_config(Some(project), &config);
        let names: Vec<_> = found.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"demo"), "got: {names:?}");
        assert!(!names.contains(&"other"), "got: {names:?}");
    }

    #[test]
    fn local_scope_overrides_project_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        write_settings(
            &project.join(".config").join("gosling"),
            r#"{"disabledPlugins":["demo"]}"#,
        );
        write_local_settings(
            &project.join(".config").join("gosling"),
            r#"{"enabledPlugins":["demo"]}"#,
        );

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());
        trust_project_with_config(project, &config).unwrap();

        let found = discover_enabled_plugins_with_config(Some(project), &config);
        assert!(
            found.iter().any(|p| p.name == "demo"),
            "local scope should win; got: {:?}",
            found.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn project_scope_overrides_user_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        let fake_home = tempfile::tempdir().unwrap();
        write_settings(
            &fake_home.path().join(".config").join("gosling"),
            r#"{"disabledPlugins":["demo"]}"#,
        );

        write_settings(
            &project.join(".config").join("gosling"),
            r#"{"enabledPlugins":["demo"]}"#,
        );

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());

        let prev = std::env::var("GOSLING_PATH_ROOT").ok();
        unsafe { std::env::set_var("GOSLING_PATH_ROOT", fake_home.path()) };
        trust_project_with_config(project, &config).unwrap();
        let found = discover_enabled_plugins_with_config(Some(project), &config);
        match prev {
            Some(v) => unsafe { std::env::set_var("GOSLING_PATH_ROOT", v) },
            None => unsafe { std::env::remove_var("GOSLING_PATH_ROOT") },
        }

        assert!(
            found.iter().any(|p| p.name == "demo"),
            "project scope should win over user; got: {:?}",
            found.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn newly_discovered_project_plugin_is_added_to_config_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());

        let found = discover_enabled_plugins_with_config(Some(project), &config);
        assert!(found.iter().all(|p| p.name != "demo"));

        let entries: HashMap<String, PluginConfigEntry> =
            config.get_param(PLUGINS_CONFIG_KEY).unwrap();
        let key = project
            .join(".agents")
            .join("plugins")
            .join("demo")
            .to_string_lossy()
            .to_string();
        let entry = entries.get(&key).expect("project plugin entry persisted");
        assert!(!entry.enabled, "got: {entries:?}");
        assert!(!entry.trusted, "got: {entries:?}");
    }

    #[test]
    fn disabled_in_config_drops_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());
        let key = project
            .join(".agents")
            .join("plugins")
            .join("demo")
            .to_string_lossy()
            .to_string();
        let entries = HashMap::from([(
            key,
            PluginConfigEntry {
                enabled: false,
                trusted: false,
            },
        )]);
        config.set_param(PLUGINS_CONFIG_KEY, entries).unwrap();

        let found = discover_enabled_plugins_with_config(Some(project), &config);
        assert!(found.iter().all(|p| p.name != "demo"));
    }

    #[test]
    fn enabled_in_config_keeps_plugin_without_modifying_config() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        write_plugin_dir(&project.join(".agents").join("plugins"), "demo");

        let cfg_dir = tempfile::tempdir().unwrap();
        let config = test_config(cfg_dir.path());
        let key = project
            .join(".agents")
            .join("plugins")
            .join("demo")
            .to_string_lossy()
            .to_string();
        config
            .set_param(
                PLUGINS_CONFIG_KEY,
                HashMap::from([(
                    key.clone(),
                    PluginConfigEntry {
                        enabled: true,
                        trusted: true,
                    },
                )]),
            )
            .unwrap();

        let found = discover_enabled_plugins_with_config(Some(project), &config);
        assert!(found.iter().any(|p| p.name == "demo"));

        let entries: HashMap<String, PluginConfigEntry> =
            config.get_param(PLUGINS_CONFIG_KEY).unwrap();
        assert!(entries.get(&key).is_some_and(|e| e.enabled));
    }
}
