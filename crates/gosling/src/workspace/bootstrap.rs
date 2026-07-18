use super::{
    CredentialAuthKind, CredentialProfile, CredentialProfileSource, CredentialProfileStatus,
    WorkspaceMutation, WorkspaceService,
};
use crate::config::{paths::Paths, Config, ConfigError};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const TEMPLATE_CONFIG_KEY: &str = "GOSLING_WORKSPACE_TEMPLATES";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceTemplateManifest {
    schema_version: u32,
    #[serde(default)]
    credential_profiles: Vec<CredentialProfileTemplate>,
    #[serde(default)]
    workspaces: Vec<WorkspaceTemplate>,
    #[serde(default)]
    active_workspace_template_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CredentialProfileTemplate {
    id: String,
    name: String,
    provider_or_service_id: String,
    #[serde(default)]
    auth_kind: CredentialAuthKind,
    #[serde(default)]
    secret_field_names: Vec<String>,
    #[serde(default)]
    non_secret_fields: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceTemplate {
    id: String,
    workspace: WorkspaceMutation,
}

struct TemplatePathRoots {
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
    cwd: PathBuf,
}

impl WorkspaceService {
    pub(super) async fn materialize_distribution_templates(
        &self,
        data_dir: &Path,
        default_working_folder: &Path,
    ) -> Result<()> {
        let _guard = self.operation_lock.lock().await;
        let _credential_transaction = self.store.lock_credential_transaction()?;
        if self.store.load()?.templates_materialized {
            return Ok(());
        }

        let value = match Config::global().get_param::<Value>(TEMPLATE_CONFIG_KEY) {
            Ok(value) => Some(value),
            Err(ConfigError::NotFound(_)) => None,
            Err(error) => return Err(error.into()),
        };
        let Some(value) = value else {
            return self.store.mutate(|document| {
                document.templates_materialized = true;
                Ok(())
            });
        };

        super::service::reject_secret_shaped_value(&value)?;
        let manifest: WorkspaceTemplateManifest =
            serde_json::from_value(value).context("invalid workspace template manifest")?;
        if manifest.schema_version != 1 {
            bail!("unsupported workspace template schema version");
        }

        let roots = TemplatePathRoots {
            home: dirs::home_dir().ok_or_else(|| anyhow!("home directory is unavailable"))?,
            config: Paths::config_dir(),
            data: data_dir.to_path_buf(),
            cwd: default_working_folder.to_path_buf(),
        };
        let profiles = prepare_profiles(&manifest.credential_profiles).await?;
        let mut workspaces = Vec::with_capacity(manifest.workspaces.len());
        for template in manifest.workspaces {
            require_uuid(&template.id, "workspace template")?;
            let mut mutation = template.workspace;
            resolve_workspace_paths(&mut mutation, &roots)?;
            ensure_nested_ids(&mut mutation);
            super::service::validate_workspace_boundary(&mutation)?;
            workspaces.push((template.id, mutation));
        }

        let now = Utc::now().to_rfc3339();
        self.store.mutate(|document| {
            for (profile, expected_fields) in &profiles {
                if document
                    .credential_profiles
                    .iter()
                    .any(|existing| existing.id == profile.id)
                {
                    bail!("workspace template credential profile ID is already in use");
                }
                document
                    .distribution_profile_secret_fields
                    .insert(profile.id.clone(), expected_fields.clone());
                document.credential_profiles.push(profile.clone());
            }

            for (id, mutation) in workspaces {
                if mutation.credential_bindings.iter().any(|binding| {
                    !document
                        .credential_profiles
                        .iter()
                        .any(|profile| profile.id == binding.credential_profile_id)
                }) {
                    bail!("workspace template references an unknown credential profile");
                }
                if document.workspaces.iter().any(|workspace| {
                    workspace.id == id || workspace.name.eq_ignore_ascii_case(&mutation.name)
                }) {
                    bail!("workspace template ID or name is already in use");
                }
                document
                    .workspaces
                    .push(super::service::workspace_from_mutation(
                        id,
                        mutation,
                        now.clone(),
                        now.clone(),
                        now.clone(),
                    ));
            }

            if let Some(active_id) = manifest.active_workspace_template_id {
                if !document
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.id == active_id)
                {
                    bail!("active workspace template does not exist");
                }
                document.active_workspace_id = active_id;
            }
            document.workspaces.sort_by(super::service::workspace_order);
            document.templates_materialized = true;
            Ok(())
        })
    }
}

async fn prepare_profiles(
    templates: &[CredentialProfileTemplate],
) -> Result<Vec<(CredentialProfile, Vec<String>)>> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut profiles = Vec::with_capacity(templates.len());
    for template in templates {
        require_uuid(&template.id, "credential profile template")?;
        if !ids.insert(template.id.clone())
            || !names.insert(template.name.trim().to_ascii_lowercase())
        {
            bail!("duplicate credential profile template");
        }
        let entry = crate::providers::get_from_registry(&template.provider_or_service_id)
            .await
            .with_context(|| {
                format!(
                    "unknown workspace template provider {}",
                    template.provider_or_service_id
                )
            })?;
        let declared_secret_fields = entry
            .metadata()
            .config_keys
            .iter()
            .filter(|key| key.secret)
            .map(|key| key.name.as_str())
            .collect::<HashSet<_>>();
        if template
            .secret_field_names
            .iter()
            .any(|field| !declared_secret_fields.contains(field.as_str()))
        {
            bail!("workspace template contains an undeclared credential field name");
        }
        let configured = configured_distribution_fields(&template.id, &template.secret_field_names);
        let status = if (template.secret_field_names.is_empty()
            && template.auth_kind == CredentialAuthKind::Local)
            || (!template.secret_field_names.is_empty()
                && configured.len() == template.secret_field_names.len())
        {
            CredentialProfileStatus::Configured
        } else if matches!(
            template.auth_kind,
            CredentialAuthKind::Oauth | CredentialAuthKind::Cli
        ) {
            CredentialProfileStatus::NeedsAuthentication
        } else {
            CredentialProfileStatus::Missing
        };
        let now = Utc::now().to_rfc3339();
        profiles.push((
            CredentialProfile {
                id: template.id.clone(),
                name: super::service::normalized_name(&template.name)?,
                provider_or_service_id: template.provider_or_service_id.clone(),
                auth_kind: template.auth_kind,
                configured_secret_fields: template.secret_field_names.clone(),
                non_secret_fields: template.non_secret_fields.clone(),
                status,
                source: CredentialProfileSource::DistributionTemplate,
                created_at: now.clone(),
                updated_at: now,
            },
            template.secret_field_names.clone(),
        ));
    }
    Ok(profiles)
}

pub(super) fn configured_distribution_fields(id: &str, fields: &[String]) -> Vec<String> {
    fields
        .iter()
        .filter(|field| {
            Config::global()
                .get_secret::<Value>(&format!("workspace-credential::{id}::{field}"))
                .is_ok()
        })
        .cloned()
        .collect()
}

fn resolve_workspace_paths(
    workspace: &mut WorkspaceMutation,
    roots: &TemplatePathRoots,
) -> Result<()> {
    workspace.working_folder = resolve_template_path(&workspace.working_folder, roots)?;
    for folder in &mut workspace.folders {
        folder.path = resolve_template_path(&folder.path, roots)?;
    }
    for output in &mut workspace.product_output_folders {
        output.path = resolve_template_path(&output.path, roots)?;
    }
    Ok(())
}

fn resolve_template_path(value: &str, roots: &TemplatePathRoots) -> Result<String> {
    let candidates = [
        ("${HOME}", &roots.home),
        ("${CONFIG_DIR}", &roots.config),
        ("${DATA_DIR}", &roots.data),
        ("${CWD}", &roots.cwd),
        ("~", &roots.home),
    ];
    for (token, root) in candidates {
        if value == token {
            return super::normalize_workspace_path(&root.to_string_lossy())
                .map_err(anyhow::Error::msg);
        }
        if let Some(suffix) = value
            .strip_prefix(token)
            .and_then(|value| value.strip_prefix('/').or_else(|| value.strip_prefix('\\')))
        {
            return super::normalize_workspace_path(&root.join(suffix).to_string_lossy())
                .map_err(anyhow::Error::msg);
        }
    }
    if value.contains("${") || value.starts_with('~') {
        bail!("workspace template contains an unsupported path placeholder");
    }
    super::normalize_workspace_path(value).map_err(anyhow::Error::msg)
}

fn ensure_nested_ids(workspace: &mut WorkspaceMutation) {
    for folder in &mut workspace.folders {
        if folder.id.is_empty() {
            folder.id = Uuid::now_v7().to_string();
        }
    }
    for output in &mut workspace.product_output_folders {
        if output.id.is_empty() {
            output.id = Uuid::now_v7().to_string();
        }
    }
    for binding in &mut workspace.credential_bindings {
        if binding.id.is_empty() {
            binding.id = Uuid::now_v7().to_string();
        }
    }
}

fn require_uuid(value: &str, label: &str) -> Result<()> {
    Uuid::parse_str(value)
        .map(|_| ())
        .with_context(|| format!("{label} ID must be a UUID"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_only_known_path_placeholders() {
        let roots = TemplatePathRoots {
            home: PathBuf::from("/home/test"),
            config: PathBuf::from("/config"),
            data: PathBuf::from("/data"),
            cwd: PathBuf::from("/work"),
        };
        assert_eq!(
            resolve_template_path("${HOME}/Projects", &roots).unwrap(),
            "/home/test/Projects"
        );
        assert!(resolve_template_path("${API_KEY}/Projects", &roots).is_err());
        assert!(resolve_template_path("../escape", &roots).is_err());
    }
}
