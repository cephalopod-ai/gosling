use anyhow::{anyhow, bail, Result};
use gosling_sdk_types::recall_brief::{RecallFacetState, RecallReceipt, RecallSourceEvidence};
use rmcp::model::CallToolResult;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use url::Url;

const MAX_RESULT_BYTES: usize = 1_000_000;
const MAX_RETURNED_HITS: usize = 32;
const MAX_EXCERPT_CHARS: usize = 2_000;
const MAX_SOURCE_QUOTE_CHARS: usize = 360;

pub struct EvidenceBundle {
    pub sources: Vec<RecallSourceEvidence>,
    pub windows: Vec<String>,
    pub receipt: Option<RecallReceipt>,
    pub omitted_count: usize,
    pub returned_count: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrievalEnvelope {
    #[serde(rename = "ContextBoundaryBegin")]
    begin: String,
    #[serde(rename = "ContextSafety")]
    safety: ContextSafety,
    #[serde(rename = "MemorySearchResult")]
    result: Value,
    #[serde(rename = "ContextBoundaryEnd")]
    end: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextSafety {
    schema_version: String,
    content_role: String,
    instruction_authority: String,
    notice: String,
    flag_policy: String,
    flagged_item_count: u64,
}

#[derive(Deserialize)]
struct SearchHit {
    id: String,
    revision: u64,
    title: String,
    status: String,
    trust_tier: String,
    source_kind: String,
    source_ref: String,
    source_ref_truncated: bool,
    recorded_at: String,
    valid_time: Option<Value>,
    content_safety: Value,
    content: String,
    content_chars: u64,
    content_truncated: bool,
    content_window: ContentWindow,
    resource_uri: String,
    document_resource_uri: Option<String>,
}

#[derive(Deserialize)]
struct ContentWindow {
    coordinate_space: String,
    offset: u64,
    end: u64,
    total_chars: u64,
    truncated: bool,
}

struct ExactIdentity {
    store_id: String,
    memory_id: String,
    revision: u64,
    offset: u64,
}

/// Decode only Muninn's untrusted retrieval envelope and authorized pinned hits.
/// Malformed hit identities are disclosed as omissions; no replacement head is fetched.
pub fn normalize_recall_result(result: &CallToolResult) -> Result<EvidenceBundle> {
    if result.is_error == Some(true) {
        bail!("Muninn recall returned an error");
    }
    let payload = match result.structured_content.as_ref() {
        Some(value) => value.clone(),
        None => {
            let text = result
                .content
                .iter()
                .filter_map(|content| content.as_text())
                .map(|content| content.text.as_str())
                .find(|content| content.trim_start().starts_with('{'))
                .ok_or_else(|| anyhow!("Muninn recall did not return structured content"))?;
            serde_json::from_str(text).map_err(|_| anyhow!("Muninn recall text is not JSON"))?
        }
    };
    if serde_json::to_vec(&payload)?.len() > MAX_RESULT_BYTES {
        bail!("Muninn recall result exceeds the report budget");
    }
    let envelope: RetrievalEnvelope = serde_json::from_value(payload)
        .map_err(|_| anyhow!("Muninn recall envelope is malformed"))?;
    if envelope.begin != "=== BEGIN MUNINN CONTEXT ==="
        || envelope.end != "=== END MUNINN CONTEXT ==="
        || envelope.safety.schema_version != "muninn.untrusted-context.v1"
        || envelope.safety.content_role != "untrusted_reference"
        || envelope.safety.instruction_authority != "none"
        || envelope.safety.flag_policy != "signal_only_not_filtered"
        || envelope.safety.notice.is_empty()
    {
        bail!("Muninn recall safety envelope is unsupported");
    }
    let _flagged_item_count = envelope.safety.flagged_item_count;
    let search = envelope
        .result
        .as_object()
        .ok_or_else(|| anyhow!("Muninn MemorySearchResult is malformed"))?;
    let hits = search
        .get("hits")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Muninn MemorySearchResult has no hits array"))?;
    if hits.len() > MAX_RETURNED_HITS {
        bail!("Muninn recall returned too many hits for one brief");
    }
    let meta = search
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Muninn MemorySearchResult has no metadata"))?;
    let receipt = recall_receipt(&Value::Object(meta.clone()), search.get("next_cursor"));
    let mut sources = Vec::new();
    let mut windows = Vec::new();
    let mut omitted_count = 0;
    let mut identities = HashSet::new();
    for value in hits {
        let hit: SearchHit = match serde_json::from_value(value.clone()) {
            Ok(hit) => hit,
            Err(_) => {
                omitted_count += 1;
                continue;
            }
        };
        let identity = match validate_hit(&hit) {
            Ok(identity) => identity,
            Err(_) => {
                omitted_count += 1;
                continue;
            }
        };
        if !identities.insert((
            identity.store_id.clone(),
            identity.memory_id.clone(),
            identity.revision,
        )) {
            continue;
        }
        let source_key = format!("S{}", sources.len() + 1);
        windows.push(hit.content.clone());
        sources.push(RecallSourceEvidence {
            source_key,
            resource_uri: hit.resource_uri,
            store_id: identity.store_id,
            memory_id: identity.memory_id,
            revision: identity.revision,
            title: hit.title,
            quote: hit.content.chars().take(MAX_SOURCE_QUOTE_CHARS).collect(),
            content_truncated: hit.content_truncated
                || hit.content.chars().count() > MAX_SOURCE_QUOTE_CHARS,
            source_ref: hit.source_ref,
            source_ref_truncated: hit.source_ref_truncated,
            source_kind: hit.source_kind,
            trust_tier: hit.trust_tier,
            status: hit.status,
            recorded_at: hit.recorded_at,
            valid_time: hit.valid_time,
            content_safety: hit.content_safety,
        });
    }
    Ok(EvidenceBundle {
        sources,
        windows,
        receipt,
        omitted_count,
        returned_count: hits.len(),
    })
}

fn validate_hit(hit: &SearchHit) -> Result<ExactIdentity> {
    let identity = parse_pinned_uri(&hit.resource_uri)?;
    if identity.memory_id != hit.id
        || identity.revision != hit.revision
        || identity.offset != hit.content_window.offset
    {
        bail!("pinned source identity disagrees with hit");
    }
    if let Some(uri) = hit.document_resource_uri.as_ref() {
        let document = parse_pinned_uri(uri)?;
        if document.store_id != identity.store_id
            || document.memory_id != identity.memory_id
            || document.revision != identity.revision
            || document.offset != 0
        {
            bail!("document link disagrees with pinned source identity");
        }
    }
    let excerpt_chars = hit.content.chars().count();
    if hit.content.is_empty()
        || excerpt_chars > MAX_EXCERPT_CHARS
        || hit.content_window.coordinate_space != "portable_memory_content"
        || hit.content_window.end < hit.content_window.offset
        || hit.content_window.end - hit.content_window.offset != excerpt_chars as u64
        || hit.content_window.total_chars != hit.content_chars
        || hit.content_window.end > hit.content_window.total_chars
        || hit.content_window.truncated
            != (hit.content_window.offset > 0
                || hit.content_window.end < hit.content_window.total_chars)
        || hit.content_truncated != hit.content_window.truncated
    {
        bail!("source content window is malformed");
    }
    if !bounded_source_text(&hit.title, 256)
        || !bounded_source_text(&hit.source_ref, 2_048)
        || !bounded_source_text(&hit.source_kind, 128)
        || !bounded_source_text(&hit.trust_tier, 128)
        || !bounded_source_text(&hit.status, 128)
        || chrono::DateTime::parse_from_rfc3339(&hit.recorded_at).is_err()
    {
        bail!("source metadata is malformed");
    }
    let safety = hit
        .content_safety
        .as_object()
        .ok_or_else(|| anyhow!("content safety signal is missing"))?;
    if safety.get("schema_version").and_then(Value::as_str) != Some("muninn.content-safety.v1")
        || safety.get("content_role").and_then(Value::as_str) != Some("untrusted_reference")
        || safety.get("instruction_authority").and_then(Value::as_str) != Some("none")
        || safety.get("flagged").and_then(Value::as_bool).is_none()
    {
        bail!("content safety signal is unsupported");
    }
    Ok(identity)
}

fn bounded_source_text(value: &str, maximum: usize) -> bool {
    value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

fn parse_pinned_uri(value: &str) -> Result<ExactIdentity> {
    if value.len() > 512
        || !value.is_ascii()
        || value
            .chars()
            .any(|character| matches!(character, '%' | '+' | '\\'))
    {
        bail!("memory resource URI is invalid");
    }
    let uri = Url::parse(value).map_err(|_| anyhow!("memory resource URI is invalid"))?;
    if uri.scheme() != "muninn"
        || uri.host_str() != Some("memory")
        || uri.fragment().is_some()
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.port().is_some()
    {
        bail!("memory resource URI is invalid");
    }
    let segments: Vec<_> = uri.path().split('/').collect();
    if segments.len() != 3 || !segments[0].is_empty() {
        bail!("memory resource URI path is invalid");
    }
    let store_id = segments[1];
    if store_id.is_empty()
        || store_id.len() > 64
        || !store_id.as_bytes()[0].is_ascii_lowercase()
        || !store_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        bail!("memory store identity is invalid");
    }
    let memory_id = segments[2];
    let Some(hex) = memory_id.strip_prefix("MEM-") else {
        bail!("memory identity is invalid");
    };
    if hex.len() != 32
        || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        || hex.as_bytes()[12] != b'7'
    {
        bail!("memory identity is invalid");
    }
    let mut revision = None;
    let mut offset = 0;
    let mut seen = HashSet::new();
    if let Some(query) = uri.query() {
        for part in query.split('&') {
            let (key, raw) = part
                .split_once('=')
                .ok_or_else(|| anyhow!("memory resource query is invalid"))?;
            if !seen.insert(key) || !matches!(key, "revision" | "offset" | "max_chars") {
                bail!("memory resource query is invalid");
            }
            if raw.is_empty()
                || raw.len() > 20
                || (raw.len() > 1 && raw.starts_with('0'))
                || !raw.bytes().all(|byte| byte.is_ascii_digit())
            {
                bail!("memory resource query is invalid");
            }
            let number = raw.parse::<u64>()?;
            match key {
                "revision" if number > 0 => revision = Some(number),
                "offset" => offset = number,
                "max_chars" if (1..=20_000).contains(&number) => {}
                _ => bail!("memory resource query is invalid"),
            }
        }
    }
    Ok(ExactIdentity {
        store_id: store_id.to_string(),
        memory_id: memory_id.to_string(),
        revision: revision.ok_or_else(|| anyhow!("memory resource is not pinned to a revision"))?,
        offset,
    })
}

fn recall_receipt(meta: &Value, top_cursor: Option<&Value>) -> Option<RecallReceipt> {
    let recall = meta.get("recall");
    let policy = recall.and_then(|value| value.get("policy"));
    let facet_coverage = meta.get("facet_coverage");
    let next_cursor = top_cursor
        .and_then(Value::as_str)
        .or_else(|| meta.get("next_cursor").and_then(Value::as_str))
        .map(ToString::to_string);
    let lanes = recall
        .and_then(|value| value.get("lanes"))
        .and_then(Value::as_array)
        .map(|lanes| {
            lanes
                .iter()
                .filter_map(|lane| {
                    Some(RecallFacetState {
                        lane: lane.get("lane")?.as_str()?.to_string(),
                        state: lane.get("state")?.as_str()?.to_string(),
                        candidates_fetched: lane.get("candidates_fetched").and_then(Value::as_u64),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let receipt = RecallReceipt {
        partial: recall
            .and_then(|value| value.get("partial"))
            .and_then(Value::as_bool),
        min_unique: policy
            .and_then(|value| value.get("min_unique"))
            .and_then(Value::as_u64),
        target_unique: policy
            .and_then(|value| value.get("target_unique"))
            .and_then(Value::as_u64),
        item_budget: policy
            .and_then(|value| value.get("item_budget"))
            .and_then(Value::as_u64),
        selected_count: recall
            .and_then(|value| value.get("selected_count"))
            .and_then(Value::as_u64),
        unique_findings: recall
            .and_then(|value| value.get("unique_findings"))
            .and_then(Value::as_u64),
        stop_reason: recall
            .and_then(|value| value.get("stop_reason"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        next_cursor,
        generation_changed: recall
            .and_then(|value| value.get("generation_changed"))
            .and_then(Value::as_bool),
        facets_covered: facet_labels(facet_coverage, "facets_covered"),
        facets_empty: facet_labels(facet_coverage, "facets_empty"),
        facets_failed: facet_labels(facet_coverage, "facets_failed"),
        lanes,
    };
    (recall.is_some() || facet_coverage.is_some() || receipt.next_cursor.is_some())
        .then_some(receipt)
}

fn facet_labels(coverage: Option<&Value>, field: &str) -> Option<Vec<String>> {
    coverage?
        .get(field)?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(ToString::to_string))
        .collect()
}
