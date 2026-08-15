use crate::acp::custom_requests::{
    DomainAdapterDescriptor, DomainAdapterStatus, ShellExtensionSelection, ShellModuleKind,
    ShellModuleStatus, ShellModuleSummary,
};
use std::collections::{BTreeMap, HashSet};

pub const MAX_SHELL_MODULES: usize = 64;
pub const MAX_SHELL_MODULE_CAPABILITIES: usize = 64;
const MAX_SHELL_MODULE_ID_BYTES: usize = 256;

/// Everything the module registry is allowed to observe. Every field is already-resolved backend
/// truth; the registry itself performs no discovery and reaches no process.
pub struct ShellModuleInputs<'a> {
    pub session_capabilities: &'a [String],
    pub selected_extensions: &'a [ShellExtensionSelection],
    pub available_extensions: &'a HashSet<String>,
    pub selected_skills: &'a [String],
    pub available_skills: &'a HashSet<String>,
    pub skills_extension_available: bool,
    pub adapter: Option<&'a DomainAdapterDescriptor>,
    pub adapter_status: Option<DomainAdapterStatus>,
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_SHELL_MODULE_ID_BYTES && !value.contains('\0')
}

fn capabilities(mut values: Vec<String>) -> Vec<String> {
    values.retain(|value| bounded(value));
    values.sort();
    values.dedup();
    values.truncate(MAX_SHELL_MODULE_CAPABILITIES);
    values
}

fn availability(available: bool) -> ShellModuleStatus {
    if available {
        ShellModuleStatus::Ready
    } else {
        ShellModuleStatus::Unavailable
    }
}

fn adapter_module_status(status: Option<DomainAdapterStatus>) -> ShellModuleStatus {
    match status {
        Some(DomainAdapterStatus::Ready) => ShellModuleStatus::Ready,
        Some(DomainAdapterStatus::Incompatible) => ShellModuleStatus::Incompatible,
        Some(DomainAdapterStatus::Crashed) | Some(DomainAdapterStatus::Hung) | None => {
            ShellModuleStatus::Unavailable
        }
    }
}

/// Projects the intersection of provisioned selection and live backend resolution.
///
/// A module the product never provisioned is never listed, and a provisioned module the backend
/// could not resolve is listed as `unavailable` rather than dropped, so recovery stays visible.
pub fn resolve_shell_modules(inputs: ShellModuleInputs<'_>) -> Vec<ShellModuleSummary> {
    let mut modules = BTreeMap::<String, ShellModuleSummary>::new();

    modules.insert(
        "core:session".into(),
        ShellModuleSummary {
            id: "core:session".into(),
            kind: ShellModuleKind::Core,
            status: ShellModuleStatus::Ready,
            version: None,
            capabilities: capabilities(inputs.session_capabilities.to_vec()),
        },
    );

    for selection in inputs.selected_extensions {
        if !bounded(&selection.name) {
            continue;
        }
        let id = format!("extension:{}", selection.name);
        modules
            .entry(id.clone())
            .or_insert_with(|| ShellModuleSummary {
                id,
                kind: ShellModuleKind::Extension,
                status: availability(inputs.available_extensions.contains(&selection.name)),
                version: None,
                capabilities: capabilities(selection.available_tools.clone().unwrap_or_default()),
            });
    }

    for skill in inputs.selected_skills {
        if !bounded(skill) {
            continue;
        }
        let id = format!("skill:{skill}");
        modules
            .entry(id.clone())
            .or_insert_with(|| ShellModuleSummary {
                id,
                kind: ShellModuleKind::Skill,
                status: availability(
                    inputs.skills_extension_available && inputs.available_skills.contains(skill),
                ),
                version: None,
                capabilities: Vec::new(),
            });
    }

    if let Some(adapter) = inputs.adapter {
        if bounded(&adapter.domain_id) {
            let id = format!("adapter:{}", adapter.domain_id);
            modules.insert(
                id.clone(),
                ShellModuleSummary {
                    id,
                    kind: ShellModuleKind::Adapter,
                    status: adapter_module_status(inputs.adapter_status),
                    version: Some(adapter.version.clone()),
                    capabilities: capabilities(
                        adapter
                            .actions
                            .iter()
                            .map(|action| action.name.clone())
                            .collect(),
                    ),
                },
            );
        }
    }

    modules.into_values().take(MAX_SHELL_MODULES).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::custom_requests::{DomainAdapterAction, DomainAdapterActionKind};

    fn extension(name: &str, tools: Option<Vec<&str>>) -> ShellExtensionSelection {
        ShellExtensionSelection {
            name: name.into(),
            available_tools: tools
                .map(|tools| tools.into_iter().map(String::from).collect::<Vec<_>>()),
        }
    }

    fn descriptor() -> DomainAdapterDescriptor {
        DomainAdapterDescriptor {
            domain_id: "neutral-fixture".into(),
            display_name: "Neutral Fixture".into(),
            version: "0.1.0".into(),
            protocol_version: "1.0.0".into(),
            actions: vec![
                DomainAdapterAction {
                    name: "toggle".into(),
                    kind: DomainAdapterActionKind::Mutate,
                    schema_ref: "neutral-fixture/toggle@1".into(),
                },
                DomainAdapterAction {
                    name: "inspect".into(),
                    kind: DomainAdapterActionKind::Read,
                    schema_ref: "neutral-fixture/inspect@1".into(),
                },
            ],
        }
    }

    fn inputs<'a>(
        selected_extensions: &'a [ShellExtensionSelection],
        available_extensions: &'a HashSet<String>,
        selected_skills: &'a [String],
        available_skills: &'a HashSet<String>,
    ) -> ShellModuleInputs<'a> {
        ShellModuleInputs {
            session_capabilities: &[],
            selected_extensions,
            available_extensions,
            selected_skills,
            available_skills,
            skills_extension_available: false,
            adapter: None,
            adapter_status: None,
        }
    }

    #[test]
    fn an_installed_but_unprovisioned_extension_is_never_listed() {
        let selected = [extension("skills", None)];
        let available = HashSet::from(["skills".to_string(), "developer".to_string()]);
        let modules = resolve_shell_modules(inputs(&selected, &available, &[], &HashSet::new()));

        let ids = modules
            .iter()
            .map(|module| module.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["core:session", "extension:skills"]);
    }

    #[test]
    fn a_provisioned_but_unresolved_module_stays_visible_as_unavailable() {
        let selected = [extension("computercontroller", Some(vec!["screenshot"]))];
        let skills = ["research".to_string()];
        let modules = resolve_shell_modules(inputs(
            &selected,
            &HashSet::new(),
            &skills,
            &HashSet::from(["research".to_string()]),
        ));

        let extension = modules
            .iter()
            .find(|module| module.id == "extension:computercontroller")
            .unwrap();
        assert_eq!(extension.status, ShellModuleStatus::Unavailable);
        assert_eq!(extension.capabilities, ["screenshot"]);
        let skill = modules
            .iter()
            .find(|module| module.id == "skill:research")
            .unwrap();
        assert_eq!(skill.status, ShellModuleStatus::Unavailable);
    }

    #[test]
    fn a_skill_requires_both_the_skills_extension_and_a_resolved_skill() {
        let selected = [extension("skills", None)];
        let available_extensions = HashSet::from(["skills".to_string()]);
        let skills = ["research".to_string()];
        let available_skills = HashSet::from(["research".to_string()]);
        let mut resolved = inputs(&selected, &available_extensions, &skills, &available_skills);
        resolved.skills_extension_available = true;

        let modules = resolve_shell_modules(resolved);
        assert_eq!(
            modules
                .iter()
                .find(|module| module.id == "skill:research")
                .unwrap()
                .status,
            ShellModuleStatus::Ready
        );
    }

    #[test]
    fn duplicate_selections_collapse_and_ordering_is_deterministic() {
        let selected = [extension("skills", None), extension("skills", None)];
        let available = HashSet::from(["skills".to_string()]);
        let modules = resolve_shell_modules(inputs(&selected, &available, &[], &HashSet::new()));

        assert_eq!(modules.len(), 2);
        assert!(modules.windows(2).all(|pair| pair[0].id < pair[1].id));
    }

    #[test]
    fn adapter_status_maps_live_supervision_without_exposing_transport() {
        let adapter = descriptor();
        let available = HashSet::new();
        let skills = HashSet::new();
        for (live, expected) in [
            (DomainAdapterStatus::Ready, ShellModuleStatus::Ready),
            (DomainAdapterStatus::Crashed, ShellModuleStatus::Unavailable),
            (DomainAdapterStatus::Hung, ShellModuleStatus::Unavailable),
            (
                DomainAdapterStatus::Incompatible,
                ShellModuleStatus::Incompatible,
            ),
        ] {
            let mut resolved = inputs(&[], &available, &[], &skills);
            resolved.adapter = Some(&adapter);
            resolved.adapter_status = Some(live);
            let modules = resolve_shell_modules(resolved);
            let module = modules
                .iter()
                .find(|module| module.id == "adapter:neutral-fixture")
                .unwrap();
            assert_eq!(module.status, expected);
            assert_eq!(module.capabilities, ["inspect", "toggle"]);
            assert_eq!(module.version.as_deref(), Some("0.1.0"));
        }
    }
}
