use gosling_sdk_types::custom_requests::SourceEntry;
use std::collections::HashSet;

pub struct SkillSearchMatch<'a> {
    pub skill: &'a SourceEntry,
    pub score: u32,
}

pub fn search_skills<'a>(
    skills: &'a [SourceEntry],
    query: &str,
    limit: usize,
) -> Vec<SkillSearchMatch<'a>> {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let query_tokens = tokens(&normalized_query);
    let mut matches = skills
        .iter()
        .filter_map(|skill| {
            let score = score_skill(skill, &normalized_query, &query_tokens);
            (score > 0).then_some(SkillSearchMatch { skill, score })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.skill.name.cmp(&right.skill.name))
            .then_with(|| left.skill.path.cmp(&right.skill.path))
    });
    matches.truncate(limit.min(20));
    matches
}

fn score_skill(skill: &SourceEntry, normalized_query: &str, query_tokens: &HashSet<String>) -> u32 {
    let name = skill.name.to_ascii_lowercase();
    let routing = skill.properties.get("routing");
    if routing.is_some_and(|routing| {
        string_array(routing, "excludes")
            .iter()
            .any(|value| value_matches(query_tokens, value))
    }) {
        return 0;
    }

    let mut score = 0;
    if name == normalized_query {
        score += 1_000;
    } else if name.contains(normalized_query) {
        score += 400;
    }
    score += overlap_score(query_tokens, &tokens(&name), 80);
    score += overlap_score(
        query_tokens,
        &tokens(&skill.description.to_ascii_lowercase()),
        10,
    );

    if let Some(routing) = routing {
        score += field_score(query_tokens, routing, "actions", 120);
        score += field_score(query_tokens, routing, "roles", 90);
        score += field_score(query_tokens, routing, "targets", 110);
        score += field_score(query_tokens, routing, "keywords", 45);
        score += field_score(query_tokens, routing, "aliases", 80);
        if routing
            .get("surface")
            .and_then(|value| value.as_str())
            .is_some_and(|surface| value_matches(query_tokens, surface))
        {
            score += 100;
        }
    }
    score += catalog_route_score(skill, query_tokens);
    score
}

fn catalog_route_score(skill: &SourceEntry, query_tokens: &HashSet<String>) -> u32 {
    skill
        .properties
        .get("catalog")
        .and_then(|catalog| catalog.get("routes"))
        .and_then(|routes| routes.as_array())
        .into_iter()
        .flatten()
        .filter_map(|route| {
            let route_match = route.get("match")?;
            let matches = ["actions", "roles", "targets", "allKeywords"]
                .into_iter()
                .all(|field| {
                    string_array(route_match, field)
                        .iter()
                        .all(|value| value_matches(query_tokens, value))
                })
                && route_match
                    .get("surface")
                    .and_then(|value| value.as_str())
                    .is_none_or(|surface| value_matches(query_tokens, surface))
                && {
                    let any_keywords = string_array(route_match, "anyKeywords");
                    any_keywords.is_empty()
                        || any_keywords
                            .iter()
                            .any(|value| value_matches(query_tokens, value))
                }
                && string_array(route_match, "notKeywords")
                    .iter()
                    .all(|value| !value_matches(query_tokens, value));
            matches.then(|| {
                let priority = route
                    .get("priority")
                    .and_then(|value| value.as_i64())
                    .unwrap_or_default()
                    .max(0) as u32;
                2_000 + priority
            })
        })
        .max()
        .unwrap_or_default()
}

fn field_score(
    query_tokens: &HashSet<String>,
    routing: &serde_json::Value,
    field: &str,
    weight: u32,
) -> u32 {
    string_array(routing, field)
        .iter()
        .filter(|value| value_matches(query_tokens, value))
        .count() as u32
        * weight
}

fn value_matches(query_tokens: &HashSet<String>, value: &str) -> bool {
    let value_tokens = tokens(&value.to_ascii_lowercase());
    !value_tokens.is_empty() && value_tokens.is_subset(query_tokens)
}

fn string_array(value: &serde_json::Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn overlap_score(left: &HashSet<String>, right: &HashSet<String>, weight: u32) -> u32 {
    left.intersection(right).count() as u32 * weight
}

fn tokens(value: &str) -> HashSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gosling_sdk_types::custom_requests::{SourceEntry, SourceType};
    use serde_json::json;
    use std::collections::HashMap;

    fn skill(name: &str, routing: serde_json::Value) -> SourceEntry {
        SourceEntry {
            source_type: SourceType::Skill,
            name: name.to_string(),
            description: format!("Synthetic {} workflow", name),
            properties: HashMap::from([("routing".to_string(), routing)]),
            ..Default::default()
        }
    }

    #[test]
    fn structured_routing_outranks_description_only_match() {
        let skills = vec![
            skill(
                "generic-planner",
                json!({
                    "actions": ["plan"],
                    "roles": ["architect"],
                    "surface": "registry",
                    "targets": ["skills"],
                    "keywords": ["index"]
                }),
            ),
            skill("index-notes", json!({})),
        ];

        let matches = search_skills(&skills, "plan architect registry skills index", 5);

        assert_eq!(matches[0].skill.name, "generic-planner");
    }

    #[test]
    fn exclusion_removes_an_otherwise_matching_skill() {
        let skills = vec![skill(
            "audit-runtime",
            json!({
                "actions": ["audit"],
                "roles": ["auditor"],
                "surface": "runtime",
                "targets": ["service"],
                "keywords": ["runtime"],
                "excludes": ["static"]
            }),
        )];

        assert!(search_skills(&skills, "static runtime audit", 5).is_empty());
    }

    #[test]
    fn authored_catalog_route_outranks_generic_keyword_match() {
        let mut routed = skill("special-route", json!({}));
        routed.properties.insert(
            "catalog".to_string(),
            json!({
                "routes": [{
                    "skillId": "special-route",
                    "match": {
                        "actions": ["plan"],
                        "surface": "registry",
                        "targets": ["skills"]
                    },
                    "priority": 10
                }]
            }),
        );
        let skills = vec![skill("plan-registry-skills", json!({})), routed];

        let matches = search_skills(&skills, "plan registry skills", 5);

        assert_eq!(matches[0].skill.name, "special-route");
    }
}
