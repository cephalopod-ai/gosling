use crate::config::Config;
use anyhow::{bail, Context, Result};
use gosling_sdk_types::custom_requests::{SourceEntry, SourceType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

pub const CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub skills: Vec<CatalogSkill>,
    #[serde(default)]
    pub routes: Vec<CatalogRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogSkill {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub summary: String,
    pub directory: String,
    pub routing: CatalogRouting,
    #[serde(default)]
    pub execution: CatalogExecution,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRouting {
    pub actions: Vec<String>,
    pub roles: Vec<String>,
    pub surface: String,
    pub targets: Vec<String>,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogExecution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    #[serde(default)]
    pub requires_human_approval_for: Vec<String>,
    #[serde(default)]
    pub critic_required: bool,
    #[serde(default)]
    pub overlays_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRoute {
    pub skill_id: String,
    #[serde(rename = "match")]
    pub route_match: CatalogRouteMatch,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRouteMatch {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub all_keywords: Vec<String>,
    #[serde(default)]
    pub any_keywords: Vec<String>,
    #[serde(default)]
    pub not_keywords: Vec<String>,
}

pub fn configured_catalog_paths() -> Vec<PathBuf> {
    Config::global()
        .get_gosling_skill_catalogs()
        .unwrap_or_default()
        .into_iter()
        .map(|path| PathBuf::from(&*shellexpand::tilde(&path)))
        .collect()
}

pub fn load_configured_catalogs() -> Vec<SourceEntry> {
    configured_catalog_paths()
        .into_iter()
        .flat_map(|path| match load_catalog(&path) {
            Ok(skills) => skills,
            Err(error) => {
                tracing::warn!(
                    catalog = %path.display(),
                    error = %error,
                    "Failed to load external skill catalog"
                );
                Vec::new()
            }
        })
        .collect()
}

pub fn load_catalog(path: &Path) -> Result<Vec<SourceEntry>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read skill catalog {}", path.display()))?;
    let catalog: SkillCatalog = serde_json::from_str(&raw)
        .with_context(|| format!("Could not parse skill catalog {}", path.display()))?;
    validate_catalog(&catalog)?;

    let root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .with_context(|| format!("Could not resolve catalog root for {}", path.display()))?;

    let routes_by_skill = routes_by_skill(&catalog.routes);
    let mut sources = Vec::new();
    for descriptor in catalog.skills.iter().filter(|skill| !skill.deprecated) {
        let skill_dir = resolve_skill_directory(&root, &descriptor.directory)?;
        if !skill_dir.join("SKILL.md").is_file() {
            bail!(
                "Catalog skill '{}' does not contain a SKILL.md",
                descriptor.id
            );
        }

        let mut source = SourceEntry {
            source_type: SourceType::Skill,
            name: descriptor.id.clone(),
            description: descriptor.summary.clone(),
            path: skill_dir.to_string_lossy().into_owned(),
            global: true,
            writable: false,
            ..Default::default()
        };
        source.properties.insert(
            "routing".to_string(),
            serde_json::to_value(&descriptor.routing)?,
        );
        source.properties.insert(
            "execution".to_string(),
            serde_json::to_value(&descriptor.execution)?,
        );

        let mut catalog_metadata = Map::new();
        catalog_metadata.insert("id".to_string(), Value::String(catalog.catalog_id.clone()));
        if let Some(version) = &descriptor.version {
            catalog_metadata.insert("skillVersion".to_string(), Value::String(version.clone()));
        }
        if let Some(content_hash) = &descriptor.content_hash {
            catalog_metadata.insert(
                "contentHash".to_string(),
                Value::String(content_hash.clone()),
            );
        }
        if let Some(routes) = routes_by_skill.get(descriptor.id.as_str()) {
            catalog_metadata.insert("routes".to_string(), serde_json::to_value(routes)?);
        }
        source
            .properties
            .insert("catalog".to_string(), Value::Object(catalog_metadata));
        sources.push(source);
    }

    Ok(sources)
}

fn validate_catalog(catalog: &SkillCatalog) -> Result<()> {
    if catalog.schema_version != CATALOG_SCHEMA_VERSION {
        bail!(
            "Unsupported catalog schema version {}; expected {}",
            catalog.schema_version,
            CATALOG_SCHEMA_VERSION
        );
    }
    validate_slug("catalogId", &catalog.catalog_id)?;

    let mut ids = HashSet::new();
    for skill in &catalog.skills {
        validate_slug("skill id", &skill.id)?;
        if !ids.insert(skill.id.as_str()) {
            bail!("Duplicate catalog skill id '{}'", skill.id);
        }
        if skill.summary.trim().is_empty() {
            bail!("Catalog skill '{}' has an empty summary", skill.id);
        }
        if skill
            .version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            bail!("Catalog skill '{}' has an empty version", skill.id);
        }
        if skill
            .content_hash
            .as_ref()
            .is_some_and(|content_hash| content_hash.trim().is_empty())
        {
            bail!("Catalog skill '{}' has an empty content hash", skill.id);
        }
        validate_relative_directory(&skill.directory)?;
        validate_routing(&skill.id, &skill.routing)?;
        validate_optional_term(
            &skill.id,
            "execution.authority",
            skill.execution.authority.as_deref(),
        )?;
        validate_terms(
            &skill.id,
            "execution.requiresHumanApprovalFor",
            &skill.execution.requires_human_approval_for,
            false,
        )?;
    }

    for route in &catalog.routes {
        if !ids.contains(route.skill_id.as_str()) {
            bail!(
                "Catalog route references unknown skill '{}'",
                route.skill_id
            );
        }
        let route_match = &route.route_match;
        validate_terms(
            &route.skill_id,
            "route.match.actions",
            &route_match.actions,
            false,
        )?;
        validate_terms(
            &route.skill_id,
            "route.match.roles",
            &route_match.roles,
            false,
        )?;
        validate_optional_term(
            &route.skill_id,
            "route.match.surface",
            route_match.surface.as_deref(),
        )?;
        validate_terms(
            &route.skill_id,
            "route.match.targets",
            &route_match.targets,
            false,
        )?;
        validate_terms(
            &route.skill_id,
            "route.match.allKeywords",
            &route_match.all_keywords,
            false,
        )?;
        validate_terms(
            &route.skill_id,
            "route.match.anyKeywords",
            &route_match.any_keywords,
            false,
        )?;
        validate_terms(
            &route.skill_id,
            "route.match.notKeywords",
            &route_match.not_keywords,
            false,
        )?;
        if route_match.actions.is_empty()
            && route_match.roles.is_empty()
            && route_match.surface.is_none()
            && route_match.targets.is_empty()
            && route_match.all_keywords.is_empty()
            && route_match.any_keywords.is_empty()
        {
            bail!(
                "Catalog route for '{}' must define at least one positive match condition",
                route.skill_id
            );
        }
    }
    Ok(())
}

fn validate_routing(skill_id: &str, routing: &CatalogRouting) -> Result<()> {
    validate_terms(skill_id, "routing.actions", &routing.actions, true)?;
    validate_terms(skill_id, "routing.roles", &routing.roles, true)?;
    validate_optional_term(skill_id, "routing.surface", Some(&routing.surface))?;
    validate_terms(skill_id, "routing.targets", &routing.targets, true)?;
    validate_terms(skill_id, "routing.keywords", &routing.keywords, true)?;
    validate_terms(skill_id, "routing.aliases", &routing.aliases, false)?;
    validate_terms(skill_id, "routing.excludes", &routing.excludes, false)?;
    Ok(())
}

fn validate_optional_term(skill_id: &str, field: &str, value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        bail!("Catalog skill '{}' has an empty {}", skill_id, field);
    }
    Ok(())
}

fn validate_terms(skill_id: &str, field: &str, values: &[String], required: bool) -> Result<()> {
    if required && values.is_empty() {
        bail!("Catalog skill '{}' must define {}", skill_id, field);
    }
    if values.iter().any(|value| value.trim().is_empty()) {
        bail!("Catalog skill '{}' has an empty {} entry", skill_id, field);
    }
    let unique = values.iter().collect::<HashSet<_>>();
    if unique.len() != values.len() {
        bail!(
            "Catalog skill '{}' has duplicate {} entries",
            skill_id,
            field
        );
    }
    Ok(())
}

fn validate_slug(field: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        bail!("Invalid {} '{}'", field, value);
    }
    Ok(())
}

fn validate_relative_directory(directory: &str) -> Result<()> {
    let path = Path::new(directory);
    if directory.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("Invalid catalog skill directory '{}'", directory);
    }
    Ok(())
}

fn resolve_skill_directory(root: &Path, directory: &str) -> Result<PathBuf> {
    validate_relative_directory(directory)?;
    let resolved = root
        .join(directory)
        .canonicalize()
        .with_context(|| format!("Could not resolve catalog skill directory '{}'", directory))?;
    if !resolved.starts_with(root) {
        bail!(
            "Catalog skill directory '{}' resolves outside the catalog root",
            directory
        );
    }
    Ok(resolved)
}

fn routes_by_skill(routes: &[CatalogRoute]) -> HashMap<&str, Vec<&CatalogRoute>> {
    let mut result: HashMap<&str, Vec<&CatalogRoute>> = HashMap::new();
    for route in routes {
        result.entry(&route.skill_id).or_default().push(route);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn synthetic_catalog(directory: &str) -> SkillCatalog {
        SkillCatalog {
            schema_version: CATALOG_SCHEMA_VERSION,
            catalog_id: "private-catalog".to_string(),
            skills: vec![CatalogSkill {
                id: "plan-example".to_string(),
                version: Some("1.0".to_string()),
                summary: "Plan a synthetic example".to_string(),
                directory: directory.to_string(),
                routing: CatalogRouting {
                    actions: vec!["plan".to_string()],
                    roles: vec!["planner".to_string()],
                    surface: "example".to_string(),
                    targets: vec!["workflow".to_string()],
                    keywords: vec!["synthetic".to_string()],
                    aliases: Vec::new(),
                    excludes: Vec::new(),
                },
                execution: CatalogExecution::default(),
                deprecated: false,
                content_hash: None,
            }],
            routes: Vec::new(),
        }
    }

    #[test]
    fn loads_catalog_without_embedding_catalog_content() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("skills/plan-example");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: plan-example\ndescription: Legacy description\n---\nPlan carefully.",
        )
        .unwrap();
        fs::write(skill_dir.join("reference.md"), "Supporting details.").unwrap();
        let catalog_path = temp_dir.path().join("catalog.json");
        fs::write(
            &catalog_path,
            serde_json::to_string(&synthetic_catalog("skills/plan-example")).unwrap(),
        )
        .unwrap();

        let skills = load_catalog(&catalog_path).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "plan-example");
        assert_eq!(skills[0].description, "Plan a synthetic example");
        assert!(!skills[0].writable);
        assert!(skills[0].content.is_empty());
        assert!(skills[0].supporting_files.is_empty());
        assert_eq!(skills[0].properties["routing"]["surface"], "example");
    }

    #[test]
    fn rejects_catalog_directory_escape() {
        let temp_dir = TempDir::new().unwrap();
        let catalog_path = temp_dir.path().join("catalog.json");
        fs::write(
            &catalog_path,
            serde_json::to_string(&synthetic_catalog("../outside")).unwrap(),
        )
        .unwrap();

        let error = load_catalog(&catalog_path).unwrap_err();

        assert!(error
            .to_string()
            .contains("Invalid catalog skill directory"));
    }

    #[test]
    fn rejects_empty_routing_terms_without_schema_engine() {
        let mut catalog = synthetic_catalog("skills/plan-example");
        catalog.skills[0].routing.keywords = vec![String::new()];

        let error = validate_catalog(&catalog).unwrap_err();

        assert!(error.to_string().contains("empty routing.keywords entry"));
    }

    #[test]
    fn rejects_duplicate_routing_terms_without_schema_engine() {
        let mut catalog = synthetic_catalog("skills/plan-example");
        catalog.skills[0].routing.roles = vec!["planner".to_string(), "planner".to_string()];

        let error = validate_catalog(&catalog).unwrap_err();

        assert!(error
            .to_string()
            .contains("duplicate routing.roles entries"));
    }

    #[test]
    fn rejects_negative_only_route_without_schema_engine() {
        let mut catalog = synthetic_catalog("skills/plan-example");
        catalog.routes.push(CatalogRoute {
            skill_id: "plan-example".to_string(),
            route_match: CatalogRouteMatch {
                not_keywords: vec!["audit".to_string()],
                ..Default::default()
            },
            priority: 0,
        });

        let error = validate_catalog(&catalog).unwrap_err();

        assert!(error
            .to_string()
            .contains("at least one positive match condition"));
    }
}
