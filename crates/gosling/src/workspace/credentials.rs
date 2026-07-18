use super::store::WorkspaceStoreDocument;
use super::{
    CredentialAuthKind, CredentialProfile, CredentialProfileSource, CredentialProfileStatus,
    WorkspaceService,
};
use crate::config::{Config, ConfigResolutionScope};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProfileResolution {
    pub provider: String,
    pub secret_keys: HashMap<String, String>,
    pub parameters: BTreeMap<String, String>,
}

impl WorkspaceService {
    pub fn credential_profiles(&self) -> Result<Vec<CredentialProfile>> {
        let document = self.store.load()?;
        Ok(effective_profiles(&document))
    }

    pub fn profile_usage(&self, profile_id: &str) -> Result<Vec<(String, String)>> {
        let document = self.store.load()?;
        Ok(document
            .workspaces
            .iter()
            .filter(|workspace| {
                workspace
                    .credential_bindings
                    .iter()
                    .any(|binding| binding.credential_profile_id == profile_id)
            })
            .map(|workspace| (workspace.id.clone(), workspace.name.clone()))
            .collect())
    }

    pub async fn create_profile(
        &self,
        name: String,
        provider: String,
        auth_kind: super::CredentialAuthKind,
        non_secret_fields: BTreeMap<String, String>,
        secret_fields: Vec<super::CredentialFieldUpdate>,
    ) -> Result<CredentialProfile> {
        let _guard = self.operation_lock.lock().await;
        let declared = declared_provider_keys(&provider).await?;
        validate_profile_fields(&declared, &non_secret_fields, &secret_fields, &[])?;
        let id = Uuid::now_v7().to_string();
        let secret_updates = secret_fields
            .iter()
            .map(|field| {
                (
                    profile_secret_key(&id, &field.key),
                    Value::String(field.value.clone()),
                )
            })
            .collect::<Vec<_>>();
        let now = Utc::now().to_rfc3339();
        let mut profile = CredentialProfile {
            id: id.clone(),
            name: super::service::normalized_name(&name)?,
            provider_or_service_id: provider,
            auth_kind,
            configured_secret_fields: secret_fields
                .iter()
                .map(|field| field.key.clone())
                .collect(),
            non_secret_fields,
            status: CredentialProfileStatus::Missing,
            source: CredentialProfileSource::WorkspaceSecureStorage,
            created_at: now.clone(),
            updated_at: now,
        };
        profile.status = profile_status(&profile, &declared);
        let required_secret_fields = required_secret_fields(&declared);
        self.store.mutate(|document| {
            if document
                .credential_profiles
                .iter()
                .any(|item| item.name.eq_ignore_ascii_case(&profile.name))
            {
                bail!("credential profile name is already in use");
            }
            document
                .workspace_profile_required_secret_fields
                .insert(profile.id.clone(), required_secret_fields.clone());
            document.credential_profiles.push(profile.clone());
            document.credential_profiles.sort_by(profile_order);
            Ok(profile.clone())
        })?;
        if let Err(error) = Config::global().set_secret_values(&secret_updates) {
            let rollback = self.store.mutate(|document| {
                document
                    .credential_profiles
                    .retain(|item| item.id != profile.id);
                document
                    .workspace_profile_required_secret_fields
                    .remove(&profile.id);
                Ok(())
            });
            if let Err(rollback_error) = rollback {
                tracing::error!(%rollback_error, "Failed to roll back credential profile metadata");
            }
            return Err(error.into());
        }
        self.credential_profiles()?
            .into_iter()
            .find(|item| item.id == profile.id)
            .ok_or_else(|| anyhow!("credential profile was not persisted"))
    }

    pub async fn update_profile(
        &self,
        profile_id: &str,
        name: String,
        auth_kind: super::CredentialAuthKind,
        non_secret_fields: BTreeMap<String, String>,
        secret_fields: Vec<super::CredentialFieldUpdate>,
        clear_secret_fields: Vec<String>,
    ) -> Result<CredentialProfile> {
        let _guard = self.operation_lock.lock().await;
        let original_document = self.store.load()?;
        let existing = original_document
            .credential_profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| anyhow!("credential profile not found"))?;
        if existing.source != CredentialProfileSource::WorkspaceSecureStorage {
            bail!("global and distribution credential profiles cannot be edited here");
        }
        let declared = declared_provider_keys(&existing.provider_or_service_id).await?;
        validate_profile_fields(
            &declared,
            &non_secret_fields,
            &secret_fields,
            &clear_secret_fields,
        )?;
        let name = super::service::normalized_name(&name)?;
        if original_document
            .credential_profiles
            .iter()
            .any(|item| item.id != profile_id && item.name.eq_ignore_ascii_case(&name))
        {
            bail!("credential profile name is already in use");
        }
        let updates = secret_fields
            .iter()
            .map(|field| {
                (
                    profile_secret_key(profile_id, &field.key),
                    Value::String(field.value.clone()),
                )
            })
            .collect::<Vec<_>>();
        let deletions = clear_secret_fields
            .iter()
            .map(|key| profile_secret_key(profile_id, key))
            .collect::<Vec<_>>();
        let old_requirements = original_document
            .workspace_profile_required_secret_fields
            .get(profile_id)
            .cloned();
        let mut desired = existing.clone();
        desired.name = name;
        desired.auth_kind = auth_kind;
        desired.non_secret_fields = non_secret_fields;
        let mut configured: HashSet<_> = desired.configured_secret_fields.drain(..).collect();
        configured.extend(secret_fields.iter().map(|field| field.key.clone()));
        for key in &clear_secret_fields {
            configured.remove(key);
        }
        desired.configured_secret_fields = configured.into_iter().collect();
        desired.configured_secret_fields.sort();
        desired.status = profile_status(&desired, &declared);
        desired.updated_at = Utc::now().to_rfc3339();
        let required_secret_fields = required_secret_fields(&declared);
        self.store.mutate(|document| {
            let profile = document
                .credential_profiles
                .iter_mut()
                .find(|profile| profile.id == profile_id)
                .ok_or_else(|| anyhow!("credential profile not found"))?;
            *profile = desired.clone();
            document
                .workspace_profile_required_secret_fields
                .insert(profile_id.to_string(), required_secret_fields.clone());
            add_pending_deletions(document, &deletions);
            Ok(())
        })?;
        if let Err(error) = Config::global().update_secret_values(&updates, &deletions) {
            let rollback = self.store.mutate(|document| {
                if let Some(profile) = document
                    .credential_profiles
                    .iter_mut()
                    .find(|profile| profile.id == profile_id)
                {
                    *profile = existing.clone();
                }
                match old_requirements.clone() {
                    Some(fields) => {
                        document
                            .workspace_profile_required_secret_fields
                            .insert(profile_id.to_string(), fields);
                    }
                    None => {
                        document
                            .workspace_profile_required_secret_fields
                            .remove(profile_id);
                    }
                }
                remove_pending_deletions(document, &deletions);
                Ok(())
            });
            if let Err(rollback_error) = rollback {
                tracing::error!(%rollback_error, "Failed to roll back credential profile metadata");
            }
            return Err(error.into());
        }
        self.store.mutate(|document| {
            remove_pending_deletions(document, &deletions);
            Ok(())
        })?;
        self.credential_profiles()?
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| anyhow!("credential profile was not persisted"))
    }

    pub async fn delete_profile(&self, profile_id: &str, confirm_referenced: bool) -> Result<()> {
        let _guard = self.operation_lock.lock().await;
        let document = self.store.load()?;
        let profile = document
            .credential_profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| anyhow!("credential profile not found"))?;
        let referenced = document.workspaces.iter().any(|workspace| {
            workspace
                .credential_bindings
                .iter()
                .any(|binding| binding.credential_profile_id == profile_id)
        });
        if referenced && !confirm_referenced {
            bail!("credential profile is referenced by one or more workspaces");
        }
        if profile.source != CredentialProfileSource::WorkspaceSecureStorage {
            bail!("global and distribution credential profiles cannot be deleted here");
        }
        let keys = profile
            .configured_secret_fields
            .iter()
            .map(|key| profile_secret_key(profile_id, key))
            .collect::<Vec<_>>();
        self.store.mutate(|document| {
            document
                .credential_profiles
                .retain(|profile| profile.id != profile_id);
            document
                .distribution_profile_secret_fields
                .remove(profile_id);
            document
                .workspace_profile_required_secret_fields
                .remove(profile_id);
            add_pending_deletions(document, &keys);
            Ok(())
        })?;
        Config::global().delete_secret_values(&keys)?;
        self.store.mutate(|document| {
            remove_pending_deletions(document, &keys);
            Ok(())
        })
    }

    pub fn profile_resolution(&self, profile_id: &str) -> Result<ProfileResolution> {
        let document = self.store.load()?;
        let profile = effective_profiles(&document)
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| anyhow!("credential profile must be relinked"))?;
        if profile.status != CredentialProfileStatus::Configured {
            bail!("credential profile requires setup or relinking");
        }
        let mut secret_keys = HashMap::new();
        for key in &profile.configured_secret_fields {
            let stored = match profile.source {
                CredentialProfileSource::WorkspaceSecureStorage => {
                    profile_secret_key(&profile.id, key)
                }
                CredentialProfileSource::GlobalConfigurationAlias => key.clone(),
                CredentialProfileSource::DistributionTemplate => {
                    profile_secret_key(&profile.id, key)
                }
            };
            secret_keys.insert(key.clone(), stored);
        }
        Ok(ProfileResolution {
            provider: profile.provider_or_service_id,
            secret_keys,
            parameters: profile.non_secret_fields,
        })
    }

    pub async fn config_scope(&self, profile_id: &str) -> Result<ConfigResolutionScope> {
        let resolution = self.profile_resolution(profile_id)?;
        let entry = crate::providers::get_from_registry(&resolution.provider).await?;
        let scoped_keys = entry
            .metadata()
            .config_keys
            .iter()
            .map(|key| key.name.clone())
            .collect::<Vec<_>>();
        let parameter_values = resolution
            .parameters
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect();
        Ok(ConfigResolutionScope::new(
            scoped_keys,
            resolution.secret_keys,
            parameter_values,
        ))
    }

    pub(super) async fn migrate_global_provider_profile(&self) -> Result<()> {
        let _guard = self.operation_lock.lock().await;
        let document = self.store.load()?;
        if document.migration_completed {
            return Ok(());
        }
        let provider = Config::global().get_gosling_provider().ok();
        let profile = if let Some(provider) = provider {
            global_alias_profile(&provider).await?
        } else {
            None
        };
        self.store.mutate(|document| {
            if let Some((profile, required_secret_fields)) = profile {
                if !document
                    .credential_profiles
                    .iter()
                    .any(|item| item.id == profile.id)
                {
                    if let Some(default) = document
                        .workspaces
                        .iter_mut()
                        .find(|item| item.id == document.default_workspace_id)
                    {
                        let binding_id = Uuid::now_v7().to_string();
                        default.default_provider = Some(profile.provider_or_service_id.clone());
                        default.credential_bindings.push(super::CredentialBinding {
                            id: binding_id.clone(),
                            label: profile.name.clone(),
                            credential_profile_id: profile.id.clone(),
                            target_kind: super::CredentialTargetKind::Provider,
                            target_id: profile.provider_or_service_id.clone(),
                            is_default: true,
                        });
                        default.default_credential_binding_id = Some(binding_id);
                    }
                    document
                        .workspace_profile_required_secret_fields
                        .insert(profile.id.clone(), required_secret_fields);
                    document.credential_profiles.push(profile);
                }
            }
            document.migration_completed = true;
            Ok(())
        })
    }

    pub(super) async fn cleanup_pending_secret_deletions(&self) -> Result<()> {
        let _guard = self.operation_lock.lock().await;
        let pending = self.store.load()?.pending_secret_deletions;
        if pending.is_empty() {
            return Ok(());
        }
        if let Err(error) = Config::global().delete_secret_values(&pending) {
            tracing::warn!(%error, "Deferred credential cleanup remains pending");
            return Ok(());
        }
        self.store.mutate(|document| {
            remove_pending_deletions(document, &pending);
            Ok(())
        })
    }
}

async fn global_alias_profile(provider: &str) -> Result<Option<(CredentialProfile, Vec<String>)>> {
    let entry = match crate::providers::get_from_registry(provider).await {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };
    let declared = &entry.metadata().config_keys;
    let configured = declared
        .iter()
        .filter(|key| key.secret && Config::global().get_secret::<Value>(&key.name).is_ok())
        .map(|key| key.name.clone())
        .collect::<Vec<_>>();
    let non_secret_fields = declared
        .iter()
        .filter(|key| !key.secret)
        .filter_map(|key| {
            Config::global()
                .get_param::<Value>(&key.name)
                .ok()
                .map(|value| (key.name.clone(), config_value_string(value)))
        })
        .collect();
    let mut profile = CredentialProfile {
        id: format!("global-provider::{provider}"),
        name: format!("Current {provider} configuration"),
        provider_or_service_id: provider.to_string(),
        auth_kind: super::CredentialAuthKind::ConfigFields,
        configured_secret_fields: configured,
        non_secret_fields,
        status: CredentialProfileStatus::Missing,
        source: CredentialProfileSource::GlobalConfigurationAlias,
        created_at: String::new(),
        updated_at: String::new(),
    };
    profile.status = profile_status(&profile, declared);
    if profile.status != CredentialProfileStatus::Configured {
        return Ok(None);
    }
    let now = Utc::now().to_rfc3339();
    profile.created_at = now.clone();
    profile.updated_at = now;
    Ok(Some((profile, required_secret_fields(declared))))
}

pub(super) fn effective_profiles(document: &WorkspaceStoreDocument) -> Vec<CredentialProfile> {
    document
        .credential_profiles
        .iter()
        .cloned()
        .map(|profile| {
            effective_profile_with(document, profile, |key| {
                Config::global().get_secret::<Value>(key).is_ok()
            })
        })
        .collect()
}

fn effective_profile_with(
    document: &WorkspaceStoreDocument,
    mut profile: CredentialProfile,
    has_secret: impl Fn(&str) -> bool,
) -> CredentialProfile {
    let intended = if profile.source == CredentialProfileSource::DistributionTemplate {
        document
            .distribution_profile_secret_fields
            .get(&profile.id)
            .cloned()
            .unwrap_or_default()
    } else {
        profile.configured_secret_fields.clone()
    };
    profile.configured_secret_fields = intended
        .iter()
        .filter(|field| has_secret(&secret_storage_key(&profile, field)))
        .cloned()
        .collect();

    let required = if profile.source == CredentialProfileSource::DistributionTemplate {
        document
            .distribution_profile_secret_fields
            .get(&profile.id)
            .cloned()
            .unwrap_or_default()
    } else {
        document
            .workspace_profile_required_secret_fields
            .get(&profile.id)
            .cloned()
            .unwrap_or_default()
    };
    let required_available = required
        .iter()
        .all(|field| profile.configured_secret_fields.contains(field));
    profile.status = match profile.source {
        CredentialProfileSource::DistributionTemplate if required.is_empty() => {
            if profile.auth_kind == CredentialAuthKind::Local {
                CredentialProfileStatus::Configured
            } else {
                CredentialProfileStatus::NeedsAuthentication
            }
        }
        CredentialProfileSource::DistributionTemplate => {
            if required_available {
                CredentialProfileStatus::Configured
            } else {
                CredentialProfileStatus::Missing
            }
        }
        _ if !required_available || profile.status == CredentialProfileStatus::Missing => {
            CredentialProfileStatus::Missing
        }
        _ => profile.status,
    };
    profile
}

fn secret_storage_key(profile: &CredentialProfile, field: &str) -> String {
    match profile.source {
        CredentialProfileSource::GlobalConfigurationAlias => field.to_string(),
        CredentialProfileSource::WorkspaceSecureStorage
        | CredentialProfileSource::DistributionTemplate => profile_secret_key(&profile.id, field),
    }
}

fn required_secret_fields(declared: &[crate::providers::base::ConfigKey]) -> Vec<String> {
    declared
        .iter()
        .filter(|key| key.secret && key.required)
        .map(|key| key.name.clone())
        .collect()
}

fn add_pending_deletions(document: &mut WorkspaceStoreDocument, keys: &[String]) {
    for key in keys {
        if !document.pending_secret_deletions.contains(key) {
            document.pending_secret_deletions.push(key.clone());
        }
    }
    document.pending_secret_deletions.sort();
}

fn remove_pending_deletions(document: &mut WorkspaceStoreDocument, keys: &[String]) {
    document
        .pending_secret_deletions
        .retain(|pending| !keys.contains(pending));
}

fn config_value_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        value => value.to_string(),
    }
}

fn profile_order(left: &CredentialProfile, right: &CredentialProfile) -> std::cmp::Ordering {
    left.name
        .to_lowercase()
        .cmp(&right.name.to_lowercase())
        .then_with(|| left.id.cmp(&right.id))
}

fn profile_secret_key(profile_id: &str, field: &str) -> String {
    format!("workspace-credential::{profile_id}::{field}")
}

async fn declared_provider_keys(provider: &str) -> Result<Vec<crate::providers::base::ConfigKey>> {
    Ok(crate::providers::get_from_registry(provider)
        .await
        .with_context(|| format!("unknown provider {provider}"))?
        .metadata()
        .config_keys
        .clone())
}

fn validate_profile_fields(
    declared: &[crate::providers::base::ConfigKey],
    parameters: &BTreeMap<String, String>,
    secrets: &[super::CredentialFieldUpdate],
    cleared: &[String],
) -> Result<()> {
    let secret_names: HashSet<_> = declared
        .iter()
        .filter(|key| key.secret)
        .map(|key| key.name.as_str())
        .collect();
    let parameter_names: HashSet<_> = declared
        .iter()
        .filter(|key| !key.secret)
        .map(|key| key.name.as_str())
        .collect();
    if secrets
        .iter()
        .any(|field| field.value.is_empty() || !secret_names.contains(field.key.as_str()))
        || cleared
            .iter()
            .any(|field| !secret_names.contains(field.as_str()))
    {
        bail!("credential request contains an undeclared or empty secret field");
    }
    if parameters
        .keys()
        .any(|key| !parameter_names.contains(key.as_str()))
    {
        bail!("credential request contains an undeclared non-secret field");
    }
    Ok(())
}

fn profile_status(
    profile: &CredentialProfile,
    declared: &[crate::providers::base::ConfigKey],
) -> CredentialProfileStatus {
    let configured: HashSet<_> = profile.configured_secret_fields.iter().collect();
    let missing = declared.iter().any(|key| {
        key.required
            && if key.secret {
                !configured.contains(&key.name)
            } else {
                !profile.non_secret_fields.contains_key(&key.name) && key.default.is_none()
            }
    });
    if missing {
        CredentialProfileStatus::Missing
    } else {
        CredentialProfileStatus::Configured
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_secret_status_is_derived_from_secure_storage_presence() {
        let mut document = WorkspaceStoreDocument::create_default_for_test();
        let profile = CredentialProfile {
            id: "profile".into(),
            name: "Profile".into(),
            provider_or_service_id: "provider".into(),
            configured_secret_fields: vec!["API_KEY".into()],
            status: CredentialProfileStatus::Configured,
            source: CredentialProfileSource::WorkspaceSecureStorage,
            ..CredentialProfile::default()
        };
        document
            .workspace_profile_required_secret_fields
            .insert(profile.id.clone(), vec!["API_KEY".into()]);

        let missing = effective_profile_with(&document, profile.clone(), |_| false);
        let configured = effective_profile_with(&document, profile, |_| true);

        assert_eq!(missing.status, CredentialProfileStatus::Missing);
        assert!(missing.configured_secret_fields.is_empty());
        assert_eq!(configured.status, CredentialProfileStatus::Configured);
        assert_eq!(configured.configured_secret_fields, vec!["API_KEY"]);
    }
}
