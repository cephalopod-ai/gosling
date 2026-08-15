use crate::acp::custom_requests::{
    ShellCredentialPolicy, ShellCredentialStatus, ShellCredentialSummary, ShellProvisioning,
    ShellProvisioningIssue, ShellProvisioningIssueCode, ShellProvisioningIssueSeverity,
    ShellProvisioningResolution, ShellProvisioningValidationReport,
    SHELL_PROVISIONING_SCHEMA_VERSION, SHELL_SETTINGS_SCHEMA_VERSION,
};
use crate::agents::ExtensionConfig;
use crate::config::extensions::get_enabled_extensions_with_config_for_cwd;
use crate::config::Config;
use crate::skills::discover_skills;
use crate::workspace::{CredentialProfile, CredentialProfileStatus, WorkspaceService};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_SHELL_SYSTEM_PROMPT_BYTES: usize = 64 * 1024;

fn issue(
    code: ShellProvisioningIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ShellProvisioningIssue {
    ShellProvisioningIssue {
        code,
        severity: ShellProvisioningIssueSeverity::Error,
        path: path.into(),
        message: message.into(),
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        && value != "."
        && value != ".."
}

fn extension_name(extension: &ExtensionConfig) -> String {
    extension.name().to_string()
}

pub async fn validate_shell_provisioning(
    provisioning: &ShellProvisioning,
    config: &Config,
    workspace_service: &WorkspaceService,
    builtins: &[String],
    default_working_dir: &Path,
) -> ShellProvisioningValidationReport {
    validate_shell_provisioning_for_working_dir(
        provisioning,
        config,
        workspace_service,
        builtins,
        default_working_dir,
        None,
    )
    .await
}

pub(crate) async fn validate_shell_provisioning_for_working_dir(
    provisioning: &ShellProvisioning,
    config: &Config,
    workspace_service: &WorkspaceService,
    builtins: &[String],
    default_working_dir: &Path,
    effective_working_dir: Option<&Path>,
) -> ShellProvisioningValidationReport {
    validate_shell_provisioning_for_working_dir_with_profiles(
        provisioning,
        config,
        workspace_service,
        builtins,
        default_working_dir,
        effective_working_dir,
        None,
    )
    .await
}

pub(crate) async fn validate_shell_provisioning_for_working_dir_with_profiles(
    provisioning: &ShellProvisioning,
    config: &Config,
    workspace_service: &WorkspaceService,
    builtins: &[String],
    default_working_dir: &Path,
    effective_working_dir: Option<&Path>,
    credential_profiles: Option<Result<Vec<CredentialProfile>, String>>,
) -> ShellProvisioningValidationReport {
    let mut issues = Vec::new();
    let mut resolution = ShellProvisioningResolution::default();

    if provisioning.schema_version != SHELL_PROVISIONING_SCHEMA_VERSION {
        issues.push(issue(
            ShellProvisioningIssueCode::UnsupportedSchemaVersion,
            "schemaVersion",
            format!(
                "unsupported shell provisioning schema version {}; expected {}",
                provisioning.schema_version, SHELL_PROVISIONING_SCHEMA_VERSION
            ),
        ));
    }
    if !valid_identifier(&provisioning.identity.id) {
        issues.push(issue(
            ShellProvisioningIssueCode::InvalidIdentity,
            "identity.id",
            "identity ID must use 1-64 lowercase letters, digits, '-' or '_'",
        ));
    }
    if provisioning.identity.display_name.trim().is_empty() {
        issues.push(issue(
            ShellProvisioningIssueCode::InvalidIdentity,
            "identity.displayName",
            "identity display name cannot be empty",
        ));
    }
    if provisioning.identity.version.trim().is_empty() {
        issues.push(issue(
            ShellProvisioningIssueCode::InvalidIdentity,
            "identity.version",
            "identity version cannot be empty",
        ));
    }
    if provisioning
        .settings_schema_version
        .is_some_and(|version| version != SHELL_SETTINGS_SCHEMA_VERSION)
    {
        issues.push(issue(
            ShellProvisioningIssueCode::UnsupportedSettingsSchemaVersion,
            "settingsSchemaVersion",
            format!(
                "unsupported shell settings schema version; expected {SHELL_SETTINGS_SCHEMA_VERSION}"
            ),
        ));
    }
    if let Some(instructions) = &provisioning.instructions {
        if instructions.system_prompt.trim().is_empty()
            || instructions.system_prompt.len() > MAX_SHELL_SYSTEM_PROMPT_BYTES
            || instructions.system_prompt.contains('\0')
        {
            issues.push(issue(
                ShellProvisioningIssueCode::InvalidInstructions,
                "instructions.systemPrompt",
                "shell system prompt must be non-empty, contain no NUL bytes, and be at most 64 KiB",
            ));
        }
    }

    let session = &provisioning.session;
    resolution.credential_policy = session.credential_policy;
    if session.credential_policy == ShellCredentialPolicy::SelectableCatalog
        && session.credential_profile_id.is_some()
    {
        issues.push(issue(
            ShellProvisioningIssueCode::InvalidCredentialPolicy,
            "session.credentialPolicy",
            "selectable_catalog cannot be combined with a fixed credentialProfileId",
        ));
    }
    let mut working_dir = effective_working_dir
        .unwrap_or(default_working_dir)
        .to_path_buf();
    let workspace =
        session.workspace_id.as_deref().and_then(|workspace_id| {
            match workspace_service.get(workspace_id) {
                Ok(workspace) => {
                    resolution.workspace_id = Some(workspace_id.to_string());
                    if effective_working_dir.is_none() {
                        working_dir = workspace.working_folder.clone().into();
                    }
                    match workspace_service.list() {
                        Ok((workspaces, _, _)) => {
                            if let Some(entry) = workspaces
                                .into_iter()
                                .find(|entry| entry.workspace.id == workspace_id)
                            {
                                if !entry.validation.valid_for_session {
                                    issues.push(issue(
                                        ShellProvisioningIssueCode::InvalidWorkspace,
                                        "session.workspaceId",
                                        format!(
                                            "workspace '{workspace_id}' is not valid for a session"
                                        ),
                                    ));
                                }
                            }
                        }
                        Err(error) => issues.push(issue(
                            ShellProvisioningIssueCode::InvalidWorkspace,
                            "session.workspaceId",
                            format!("workspace validation failed: {error}"),
                        )),
                    }
                    Some(workspace)
                }
                Err(_) => {
                    issues.push(issue(
                        ShellProvisioningIssueCode::MissingWorkspace,
                        "session.workspaceId",
                        format!(
                            "workspace '{workspace_id}' does not exist in main Gosling settings"
                        ),
                    ));
                    None
                }
            }
        });

    let profiles = if session.credential_profile_id.is_some() {
        match credential_profiles.unwrap_or_else(|| {
            workspace_service
                .credential_profiles()
                .map_err(|error| error.to_string())
        }) {
            Ok(profiles) => profiles,
            Err(error) => {
                issues.push(issue(
                    ShellProvisioningIssueCode::CredentialProfileUnavailable,
                    "session.credentialProfileId",
                    format!("credential profile catalog is unavailable: {error}"),
                ));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let profile = session
        .credential_profile_id
        .as_deref()
        .and_then(|profile_id| {
            let Some(profile) = profiles.iter().find(|profile| profile.id == profile_id) else {
                issues.push(issue(
                    ShellProvisioningIssueCode::MissingCredentialProfile,
                    "session.credentialProfileId",
                    format!(
                        "credential profile '{profile_id}' does not exist in main Gosling settings"
                    ),
                ));
                return None;
            };
            resolution.credential_profile_id = Some(profile_id.to_string());
            if profile.status != CredentialProfileStatus::Configured {
                issues.push(issue(
                    ShellProvisioningIssueCode::CredentialProfileUnavailable,
                    "session.credentialProfileId",
                    format!("credential profile '{profile_id}' requires setup or relinking"),
                ));
            }
            Some(profile)
        });

    let workspace_provider = workspace.as_ref().and_then(|workspace| {
        session
            .credential_profile_id
            .is_none()
            .then(|| workspace.default_provider.clone())
            .flatten()
    });
    let provider = session
        .provider
        .clone()
        .or_else(|| profile.map(|profile| profile.provider_or_service_id.clone()))
        .or(workspace_provider);
    if let (Some(profile), Some(provider)) = (profile, provider.as_deref()) {
        if profile.provider_or_service_id != provider {
            issues.push(issue(
                ShellProvisioningIssueCode::CredentialProviderMismatch,
                "session.provider",
                format!(
                    "provider '{provider}' does not match credential profile provider '{}'",
                    profile.provider_or_service_id
                ),
            ));
        }
    }
    if let Some(provider) = provider.as_deref() {
        match crate::providers::get_from_registry(provider).await {
            Ok(_) => {
                resolution.provider = Some(provider.to_string());
                let model = session.model.clone().or_else(|| {
                    workspace
                        .as_ref()
                        .filter(|_| session.provider.is_none())
                        .and_then(|workspace| workspace.default_model.clone())
                });
                if let Some(model) = model {
                    if model.trim().is_empty()
                        || crate::model_config::model_config_from_user_config(provider, &model)
                            .is_err()
                    {
                        issues.push(issue(
                            ShellProvisioningIssueCode::InvalidModel,
                            "session.model",
                            format!("model '{model}' is invalid for provider '{provider}'"),
                        ));
                    } else {
                        resolution.model = Some(model);
                    }
                }
            }
            Err(_) => issues.push(issue(
                ShellProvisioningIssueCode::MissingProvider,
                "session.provider",
                format!("provider '{provider}' is not registered"),
            )),
        }
    } else if session.model.is_some() {
        issues.push(issue(
            ShellProvisioningIssueCode::MissingProvider,
            "session.provider",
            "a model selection requires a provider, workspace, or credential profile",
        ));
    }

    if let Some(selections) = &session.extensions {
        let mut configured = crate::acp::server::selected_builtin_extensions(config, builtins);
        for extension in get_enabled_extensions_with_config_for_cwd(config, &working_dir) {
            crate::acp::server::push_or_replace_extension(&mut configured, extension);
        }
        for extension in crate::plugins::mcp_servers::enabled_plugin_mcp_servers(Some(&working_dir))
        {
            crate::acp::server::push_or_replace_extension(&mut configured, extension);
        }
        let available = configured
            .iter()
            .map(extension_name)
            .collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        for (index, selection) in selections.iter().enumerate() {
            let path = format!("session.extensions[{index}]");
            if !seen.insert(selection.name.clone()) {
                issues.push(issue(
                    ShellProvisioningIssueCode::DuplicateExtension,
                    format!("{path}.name"),
                    format!("extension '{}' is selected more than once", selection.name),
                ));
            } else if !available.contains(&selection.name) {
                issues.push(issue(
                    ShellProvisioningIssueCode::MissingExtension,
                    format!("{path}.name"),
                    format!(
                        "extension '{}' is not enabled for the resolved working directory",
                        selection.name
                    ),
                ));
            }
            if let Some(tools) = &selection.available_tools {
                let mut tool_seen = HashSet::new();
                for tool in tools {
                    if tool.trim().is_empty() || !tool_seen.insert(tool) {
                        issues.push(issue(
                            ShellProvisioningIssueCode::InvalidToolSelection,
                            format!("{path}.availableTools"),
                            "tool selections must be non-empty and unique",
                        ));
                        break;
                    }
                }
            }
        }
        crate::acp::server::apply_shell_extension_selection(&mut configured, Some(selections));
        resolution.extensions = configured
            .into_iter()
            .filter_map(|extension| {
                let name = extension.name().to_string();
                selections
                    .iter()
                    .find(|selection| selection.name == name)
                    .cloned()
            })
            .collect();
    }

    if session.extensions.as_ref().is_some_and(|extensions| {
        !extensions
            .iter()
            .any(|extension| extension.name == "skills")
    }) && session
        .skill_ids
        .as_ref()
        .is_some_and(|skills| !skills.is_empty())
    {
        issues.push(issue(
            ShellProvisioningIssueCode::MissingExtension,
            "session.skillIds",
            "skill selections require the 'skills' extension",
        ));
    }

    if let Some(skill_ids) = &session.skill_ids {
        let skills = discover_skills(Some(&working_dir))
            .into_iter()
            .map(|skill| skill.name)
            .collect::<HashSet<_>>();
        let mut seen = HashMap::<&str, usize>::new();
        for (index, skill_id) in skill_ids.iter().enumerate() {
            if let Some(previous) = seen.insert(skill_id, index) {
                issues.push(issue(
                    ShellProvisioningIssueCode::DuplicateSkill,
                    format!("session.skillIds[{index}]"),
                    format!("skill '{skill_id}' duplicates session.skillIds[{previous}]"),
                ));
            } else if !skills.contains(skill_id) {
                issues.push(issue(
                    ShellProvisioningIssueCode::MissingSkill,
                    format!("session.skillIds[{index}]"),
                    format!("skill '{skill_id}' is not available"),
                ));
            }
        }
        resolution.skill_ids = skill_ids.clone();
    }

    if !provisioning.protocol_policy.denied_methods.is_empty() {
        let known_methods = crate::acp::server::GoslingAcpAgent::custom_method_schemas(
            &mut schemars::SchemaGenerator::default(),
        )
        .into_iter()
        .map(|schema| schema.method)
        .collect::<HashSet<_>>();
        let mut denied = HashSet::new();
        for (index, method) in provisioning
            .protocol_policy
            .denied_methods
            .iter()
            .enumerate()
        {
            if !method.starts_with("_gosling/")
                || !known_methods.contains(method)
                || !denied.insert(method)
            {
                issues.push(issue(
                    ShellProvisioningIssueCode::InvalidDeniedMethod,
                    format!("protocolPolicy.deniedMethods[{index}]"),
                    format!("denied method '{method}' is not a unique Gosling custom method"),
                ));
            }
        }
    }

    if let Some(adapter) = &provisioning.domain_adapter {
        if !valid_identifier(&adapter.domain_id)
            || adapter.display_name.trim().is_empty()
            || adapter.version.trim().is_empty()
            || adapter.protocol_version.trim().is_empty()
        {
            issues.push(issue(
                ShellProvisioningIssueCode::InvalidDomainAdapter,
                "domainAdapter",
                "domain adapter requires a valid domain ID, display name, version, and protocol version",
            ));
        }
        let mut actions = HashSet::new();
        if adapter.actions.iter().any(|action| {
            !valid_identifier(&action.name)
                || action.schema_ref.trim().is_empty()
                || !actions.insert(&action.name)
        }) {
            issues.push(issue(
                ShellProvisioningIssueCode::InvalidDomainAdapter,
                "domainAdapter.actions",
                "domain adapter actions require unique valid names and schema references",
            ));
        }
    }

    ShellProvisioningValidationReport {
        valid: issues.is_empty(),
        issues,
        resolution,
    }
}

pub const MAX_SHELL_CREDENTIAL_PROFILES: usize = 128;
const MAX_SHELL_CREDENTIAL_FIELD_BYTES: usize = 256;

fn shell_credential_status(status: CredentialProfileStatus) -> ShellCredentialStatus {
    match status {
        CredentialProfileStatus::Configured => ShellCredentialStatus::Configured,
        CredentialProfileStatus::Missing | CredentialProfileStatus::NeedsAuthentication => {
            ShellCredentialStatus::RelinkRequired
        }
    }
}

/// Narrows Gosling's credential catalog to the four facts a shell may observe.
///
/// The broad `CredentialProfile` never crosses the shell boundary: this projection is built field
/// by field so a future catalog field cannot reach a shell by simply existing.
pub fn shell_credential_summaries(
    profiles: &[crate::workspace::CredentialProfile],
    provider_constraint: Option<&str>,
) -> Vec<ShellCredentialSummary> {
    let mut summaries = profiles
        .iter()
        .filter(|profile| {
            provider_constraint.is_none_or(|provider| profile.provider_or_service_id == provider)
        })
        .filter(|profile| {
            [
                profile.id.as_str(),
                profile.name.as_str(),
                profile.provider_or_service_id.as_str(),
            ]
            .iter()
            .all(|value| {
                !value.is_empty()
                    && value.len() <= MAX_SHELL_CREDENTIAL_FIELD_BYTES
                    && !value.contains('\0')
            })
        })
        .scan(HashSet::new(), |seen, profile| {
            Some(
                seen.insert(profile.id.clone())
                    .then(|| ShellCredentialSummary {
                        id: profile.id.clone(),
                        name: profile.name.clone(),
                        provider_or_service_id: profile.provider_or_service_id.clone(),
                        status: shell_credential_status(profile.status),
                    }),
            )
        })
        .flatten()
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    summaries.truncate(MAX_SHELL_CREDENTIAL_PROFILES);
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::custom_requests::{
        ShellIdentity, ShellInstructionProfile, ShellSessionProvisioning,
        SHELL_PROVISIONING_SCHEMA_VERSION,
    };
    use tempfile::TempDir;

    fn write_project_skill(working_dir: &Path, name: &str) {
        let skill_dir = working_dir.join(".agents/skills").join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Workspace override skill\n---\n\n# {name}\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn effective_working_dir_overrides_the_provisioned_workspace_for_skill_validation() {
        let data = TempDir::new().unwrap();
        let workspace_dir = TempDir::new().unwrap();
        let override_dir = TempDir::new().unwrap();
        let skill_id = "shell-workspace-override-only-skill";
        write_project_skill(override_dir.path(), skill_id);
        let workspace_service = WorkspaceService::initialize(data.path(), workspace_dir.path())
            .await
            .unwrap();
        let workspace_id = workspace_service.list().unwrap().1;
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "test_shell".into(),
                display_name: "Test Shell".into(),
                version: "1".into(),
                runtime_namespace: "test_shell".into(),
            },
            session: ShellSessionProvisioning {
                workspace_id: Some(workspace_id),
                skill_ids: Some(vec![skill_id.into()]),
                ..ShellSessionProvisioning::default()
            },
            ..ShellProvisioning::default()
        };

        let workspace_report = validate_shell_provisioning(
            &provisioning,
            Config::global(),
            &workspace_service,
            &[],
            workspace_dir.path(),
        )
        .await;
        assert!(workspace_report
            .issues
            .iter()
            .any(|issue| issue.code == ShellProvisioningIssueCode::MissingSkill));

        let override_report = validate_shell_provisioning_for_working_dir(
            &provisioning,
            Config::global(),
            &workspace_service,
            &[],
            workspace_dir.path(),
            Some(override_dir.path()),
        )
        .await;
        assert!(override_report.valid, "{:?}", override_report.issues);
    }

    fn sentinel_profile(id: &str, provider: &str) -> crate::workspace::CredentialProfile {
        crate::workspace::CredentialProfile {
            id: id.into(),
            name: format!("Profile {id}"),
            provider_or_service_id: provider.into(),
            configured_secret_fields: vec!["SENTINEL_SECRET_FIELD".into()],
            non_secret_fields: std::collections::BTreeMap::from([(
                "SENTINEL_PARAMETER".to_string(),
                "SENTINEL_VALUE".to_string(),
            )]),
            status: CredentialProfileStatus::Configured,
            created_at: "SENTINEL_CREATED_AT".into(),
            updated_at: "SENTINEL_UPDATED_AT".into(),
            ..crate::workspace::CredentialProfile::default()
        }
    }

    #[test]
    fn credential_summaries_drop_every_sentinel_field_and_stay_deterministic() {
        let profiles = vec![
            sentinel_profile("b", "anthropic"),
            sentinel_profile("a", "anthropic"),
            sentinel_profile("a", "anthropic"),
            sentinel_profile("c", "openai"),
        ];

        let summaries = shell_credential_summaries(&profiles, None);
        assert_eq!(
            summaries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let serialized = serde_json::to_string(&summaries).unwrap();
        for sentinel in [
            "SENTINEL_SECRET_FIELD",
            "SENTINEL_PARAMETER",
            "SENTINEL_VALUE",
            "SENTINEL_CREATED_AT",
            "SENTINEL_UPDATED_AT",
        ] {
            assert!(!serialized.contains(sentinel), "{sentinel} leaked");
        }
    }

    #[test]
    fn credential_summaries_apply_the_provisioned_provider_constraint() {
        let profiles = vec![
            sentinel_profile("a", "anthropic"),
            sentinel_profile("c", "openai"),
        ];
        let summaries = shell_credential_summaries(&profiles, Some("openai"));
        assert_eq!(
            summaries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["c"]
        );
    }

    #[test]
    fn credential_summaries_normalize_unusable_profiles_to_relink_required() {
        let mut profile = sentinel_profile("a", "anthropic");
        profile.status = CredentialProfileStatus::NeedsAuthentication;
        assert_eq!(
            shell_credential_summaries(&[profile], None)[0].status,
            ShellCredentialStatus::RelinkRequired
        );
    }

    #[tokio::test]
    async fn selectable_catalog_cannot_be_combined_with_a_fixed_profile() {
        let data = TempDir::new().unwrap();
        let working_dir = TempDir::new().unwrap();
        let workspace_service = WorkspaceService::initialize(data.path(), working_dir.path())
            .await
            .unwrap();
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "default_shell".into(),
                display_name: "Default Shell".into(),
                version: "1".into(),
                runtime_namespace: "default_shell".into(),
            },
            session: ShellSessionProvisioning {
                credential_policy: ShellCredentialPolicy::SelectableCatalog,
                credential_profile_id: Some("pinned".into()),
                ..ShellSessionProvisioning::default()
            },
            ..ShellProvisioning::default()
        };

        let report = validate_shell_provisioning(
            &provisioning,
            Config::global(),
            &workspace_service,
            &[],
            working_dir.path(),
        )
        .await;

        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == ShellProvisioningIssueCode::InvalidCredentialPolicy));
        assert_eq!(
            report.resolution.credential_policy,
            ShellCredentialPolicy::SelectableCatalog
        );
    }

    #[tokio::test]
    async fn provisioning_without_a_fixed_profile_never_requires_the_credential_catalog() {
        let data = TempDir::new().unwrap();
        let working_dir = TempDir::new().unwrap();
        let workspace_service = WorkspaceService::initialize(data.path(), working_dir.path())
            .await
            .unwrap();
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "default_shell".into(),
                display_name: "Default Shell".into(),
                version: "1".into(),
                runtime_namespace: "default_shell".into(),
            },
            session: ShellSessionProvisioning {
                credential_policy: ShellCredentialPolicy::SelectableCatalog,
                ..ShellSessionProvisioning::default()
            },
            ..ShellProvisioning::default()
        };

        let report = validate_shell_provisioning_for_working_dir_with_profiles(
            &provisioning,
            Config::global(),
            &workspace_service,
            &[],
            working_dir.path(),
            None,
            Some(Err("credential catalog must not be read".into())),
        )
        .await;

        assert!(report.valid, "{:?}", report.issues);
    }

    #[tokio::test]
    async fn fixed_profile_reports_a_bounded_catalog_failure() {
        let data = TempDir::new().unwrap();
        let working_dir = TempDir::new().unwrap();
        let workspace_service = WorkspaceService::initialize(data.path(), working_dir.path())
            .await
            .unwrap();
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "default_shell".into(),
                display_name: "Default Shell".into(),
                version: "1".into(),
                runtime_namespace: "default_shell".into(),
            },
            session: ShellSessionProvisioning {
                credential_profile_id: Some("fixed-profile".into()),
                ..ShellSessionProvisioning::default()
            },
            ..ShellProvisioning::default()
        };

        let report = validate_shell_provisioning_for_working_dir_with_profiles(
            &provisioning,
            Config::global(),
            &workspace_service,
            &[],
            working_dir.path(),
            None,
            Some(Err("lookup timed out".into())),
        )
        .await;

        assert!(report.issues.iter().any(|issue| {
            issue.code == ShellProvisioningIssueCode::CredentialProfileUnavailable
                && issue.message.contains("lookup timed out")
        }));
    }

    #[tokio::test]
    async fn rejects_empty_shell_instruction_profiles_without_exposing_prompt_text() {
        let data = TempDir::new().unwrap();
        let working_dir = TempDir::new().unwrap();
        let workspace_service = WorkspaceService::initialize(data.path(), working_dir.path())
            .await
            .unwrap();
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "default_shell".into(),
                display_name: "Default Shell".into(),
                version: "1".into(),
                runtime_namespace: "default_shell".into(),
            },
            instructions: Some(ShellInstructionProfile {
                system_prompt: "  ".into(),
            }),
            ..ShellProvisioning::default()
        };

        let report = validate_shell_provisioning(
            &provisioning,
            Config::global(),
            &workspace_service,
            &[],
            working_dir.path(),
        )
        .await;

        let issue = report
            .issues
            .iter()
            .find(|issue| issue.code == ShellProvisioningIssueCode::InvalidInstructions)
            .unwrap();
        assert_eq!(issue.path, "instructions.systemPrompt");
        assert!(!issue.message.contains("  "));
    }
}
