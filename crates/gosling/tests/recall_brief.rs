use async_trait::async_trait;
use gosling::conversation::message::Message;
use gosling::providers::base::{stream_from_single_message, MessageStream, Provider};
use gosling::recall_brief::{evidence_only_brief, normalize_recall_result, synthesize_brief};
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use gosling_sdk_types::recall_brief::{RecallBriefStatus, RecallClaimKind};
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::{json, Value};

fn santa_payload() -> Value {
    serde_json::from_str(include_str!("fixtures/recall_brief_santa.json")).unwrap()
}

fn santa_result() -> CallToolResult {
    CallToolResult::structured(santa_payload())
}

fn add_santa_source(payload: &mut Value, suffix: &str, revision: u64, title: &str, content: &str) {
    let mut hit = payload["MemorySearchResult"]["hits"][0].clone();
    let id = format!("MEM-000000000000700080000000000000{suffix}");
    let uri = format!("muninn://memory/personal/{id}?revision={revision}");
    let chars = content.chars().count() as u64;
    hit["id"] = json!(id);
    hit["revision"] = json!(revision);
    hit["title"] = json!(title);
    hit["content"] = json!(content);
    hit["content_chars"] = json!(chars);
    hit["content_window"]["end"] = json!(chars);
    hit["content_window"]["chars"] = json!(chars);
    hit["content_window"]["total_chars"] = json!(chars);
    hit["resource_uri"] = json!(uri);
    hit["document_resource_uri"] = json!(uri);
    hit["source_ref"] = json!(format!("chat:conversation:santa-{suffix}"));
    hit["valid_time"] = Value::Null;
    payload["MemorySearchResult"]["hits"]
        .as_array_mut()
        .unwrap()
        .push(hit);
}

#[test]
fn distinct_santa_revisions_remain_separately_citable_even_with_one_effort_group() {
    let bundle = normalize_recall_result(&santa_result()).unwrap();
    assert_eq!(bundle.sources.len(), 2);
    assert_eq!(bundle.sources[0].source_key, "S1");
    assert_eq!(bundle.sources[1].source_key, "S2");
    assert_ne!(
        bundle.sources[0].resource_uri,
        bundle.sources[1].resource_uri
    );
    let report = evidence_only_brief(bundle, "mock", "mock-model", "test");
    assert_eq!(report.status, RecallBriefStatus::EvidenceOnly);
    assert!(report
        .rendered
        .contains("I believed in Santa Claus when I was 7"));
    assert!(report
        .rendered
        .contains("I stopped believing in Santa Claus when I was 8"));
    assert!(report.rendered.contains("## Inference\n\nNone."));
    assert_eq!(report.receipt.unwrap().min_unique, Some(3));
}

#[test]
fn only_the_same_exact_revision_is_collapsed_for_display() {
    let mut payload = santa_payload();
    let hits = payload["MemorySearchResult"]["hits"]
        .as_array_mut()
        .unwrap();
    hits.push(hits[0].clone());
    let bundle = normalize_recall_result(&CallToolResult::structured(payload)).unwrap();
    assert_eq!(bundle.sources.len(), 2);
    assert_eq!(bundle.omitted_count, 0);
    assert_eq!(bundle.returned_count, 3);
}

#[test]
fn mismatched_revision_is_omitted_with_visible_partial_status() {
    let mut payload = santa_payload();
    payload["MemorySearchResult"]["hits"][0]["resource_uri"] =
        json!("muninn://memory/personal/MEM-00000000000070008000000000000001?revision=9");
    let bundle = normalize_recall_result(&CallToolResult::structured(payload)).unwrap();
    assert_eq!(bundle.omitted_count, 1);
    assert_eq!(bundle.sources.len(), 1);
    let report = evidence_only_brief(bundle, "mock", "mock-model", "source_identity_unavailable");
    assert_eq!(report.status, RecallBriefStatus::Partial);
    assert!(!report.rendered.contains("?revision=9"));
    assert!(report.notice.contains("1 returned hit"));
}

#[test]
fn malformed_envelope_and_tool_error_never_become_a_brief() {
    let mut malformed = santa_payload();
    malformed.as_object_mut().unwrap().remove("ContextSafety");
    assert!(normalize_recall_result(&CallToolResult::structured(malformed)).is_err());
    assert!(
        normalize_recall_result(&CallToolResult::error(vec![Content::text("failed")])).is_err()
    );
}

#[test]
fn strict_text_fallback_and_empty_success_keep_unknown_receipt_fields_unknown() {
    let text = include_str!("fixtures/recall_brief_santa.json");
    let bundle =
        normalize_recall_result(&CallToolResult::success(vec![Content::text(text)])).unwrap();
    assert_eq!(bundle.sources.len(), 2);
    let mut empty = santa_payload();
    empty["MemorySearchResult"]["hits"] = json!([]);
    empty["MemorySearchResult"]["meta"]["recall"]["policy"] = json!({});
    let bundle = normalize_recall_result(&CallToolResult::structured(empty)).unwrap();
    let report = evidence_only_brief(bundle, "mock", "mock-model", "empty");
    assert_eq!(report.status, RecallBriefStatus::Empty);
    assert_eq!(report.receipt.unwrap().min_unique, None);
}

#[test]
fn failed_and_empty_facets_remain_distinct_and_make_the_report_partial() {
    let mut payload = santa_payload();
    let meta = &mut payload["MemorySearchResult"]["meta"];
    meta["recall"]["partial"] = json!(true);
    meta["recall"]["lanes"] = json!([
        {"lane": "facet:0", "state": "success_empty", "candidates_fetched": 0},
        {"lane": "facet:1", "state": "failed", "candidates_fetched": 0},
        {"lane": "facet:2", "state": "unsearched", "candidates_fetched": 0}
    ]);
    meta["facet_coverage"] = json!({
        "facets_covered": [],
        "facets_empty": ["historical Santa"],
        "facets_failed": ["literal gift-deliverer"]
    });
    let report = evidence_only_brief(
        normalize_recall_result(&CallToolResult::structured(payload)).unwrap(),
        "mock",
        "mock-model",
        "test",
    );
    assert_eq!(report.status, RecallBriefStatus::Partial);
    let receipt = report.receipt.unwrap();
    assert_eq!(receipt.partial, Some(true));
    assert_eq!(receipt.facets_empty.unwrap(), vec!["historical Santa"]);
    assert_eq!(
        receipt.facets_failed.unwrap(),
        vec!["literal gift-deliverer"]
    );
    assert!(report.rendered.contains("facet:2: unsearched"));
}

#[test]
fn failed_receipt_details_make_evidence_only_reports_partial_without_a_partial_flag() {
    for flag in [Value::Null, json!(false)] {
        for failure in ["facet", "lane"] {
            let mut payload = santa_payload();
            let meta = &mut payload["MemorySearchResult"]["meta"];
            meta["recall"]["partial"] = flag.clone();
            if failure == "facet" {
                meta["facet_coverage"]["facets_failed"] = json!(["literal gift-deliverer"]);
            } else {
                meta["recall"]["lanes"] = json!([
                    {"lane": "facet:1", "state": "failed", "candidates_fetched": 0}
                ]);
            }
            let report = evidence_only_brief(
                normalize_recall_result(&CallToolResult::structured(payload)).unwrap(),
                "mock",
                "mock-model",
                "test",
            );
            assert_eq!(report.status, RecallBriefStatus::Partial, "{failure}");
            assert!(report.notice.contains("Partial recall"));
            assert!(report.rendered.contains("recall was partial"));
            assert_eq!(report.receipt.unwrap().partial, flag.as_bool());
        }
    }

    let mut payload = santa_payload();
    payload["MemorySearchResult"]["meta"]["recall"]["partial"] = json!(false);
    payload["MemorySearchResult"]["meta"]["recall"]["lanes"] = json!([
        {"lane": "facet:0", "state": "success_empty", "candidates_fetched": 0},
        {"lane": "facet:1", "state": "unsearched", "candidates_fetched": 0}
    ]);
    let report = evidence_only_brief(
        normalize_recall_result(&CallToolResult::structured(payload)).unwrap(),
        "mock",
        "mock-model",
        "test",
    );
    assert_eq!(report.status, RecallBriefStatus::EvidenceOnly);

    let mut empty = santa_payload();
    empty["MemorySearchResult"]["hits"] = json!([]);
    empty["MemorySearchResult"]["meta"]["recall"]["partial"] = Value::Null;
    empty["MemorySearchResult"]["meta"]["recall"]["lanes"] = json!([
        {"lane": "facet:1", "state": "failed", "candidates_fetched": 0}
    ]);
    let report = evidence_only_brief(
        normalize_recall_result(&CallToolResult::structured(empty)).unwrap(),
        "mock",
        "mock-model",
        "test",
    );
    assert_eq!(report.status, RecallBriefStatus::Partial);
    assert!(report.rendered.contains("None returned."));
}

#[test]
fn unpinned_or_mismatched_windows_cannot_be_cited() {
    let mut payload = santa_payload();
    payload["MemorySearchResult"]["hits"][0]["resource_uri"] =
        json!("muninn://memory/personal/MEM-00000000000070008000000000000001");
    payload["MemorySearchResult"]["hits"][1]["content_window"]["end"] = json!(46);
    let bundle = normalize_recall_result(&CallToolResult::structured(payload)).unwrap();
    assert_eq!(bundle.omitted_count, 2);
    assert!(bundle.sources.is_empty());
}

#[test]
fn untrusted_source_title_cannot_create_a_markdown_link_in_the_report() {
    let mut payload = santa_payload();
    payload["MemorySearchResult"]["hits"][0]["title"] =
        json!("[fake citation](https://untrusted.example)");
    let report = evidence_only_brief(
        normalize_recall_result(&CallToolResult::structured(payload)).unwrap(),
        "mock",
        "mock-model",
        "test",
    );
    assert!(report.rendered.contains("\\[fake citation\\]"));
    assert!(!report.rendered.contains("[fake citation](https://"));
}

struct MockProvider {
    response: String,
}

#[async_trait]
impl Provider for MockProvider {
    fn get_name(&self) -> &str {
        "mock-recall"
    }

    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system: &str,
        _messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        assert!(tools.is_empty());
        Ok(stream_from_single_message(
            Message::assistant().with_text(&self.response),
            ProviderUsage::new("mock-model".to_string(), Usage::default()),
        ))
    }
}

#[tokio::test]
async fn valid_santa_proposal_reports_beliefs_and_leaves_world_existence_unresolved() {
    let provider = MockProvider {
        response: json!({
            "findings": [
                {
                    "claimKind": "reported_belief",
                    "statement": "You reported believing in Santa Claus at age seven.",
                    "subject": "user",
                    "referentSense": "fairy_tale_gift_deliverer",
                    "timeLabel": "age seven (as stated)",
                    "sourceEvidence": [{"sourceKey": "S1", "quote": "I believed in Santa Claus when I was 7"}],
                    "inference": null
                },
                {
                    "claimKind": "reported_belief",
                    "statement": "You reported no longer believing in Santa Claus at age eight.",
                    "subject": "user",
                    "referentSense": "fairy_tale_gift_deliverer",
                    "timeLabel": "age eight (as stated)",
                    "sourceEvidence": [{"sourceKey": "S2", "quote": "I stopped believing in Santa Claus when I was 8"}],
                    "inference": null
                },
                {
                    "claimKind": "inference",
                    "statement": "Your reported belief appears to have changed between the two records.",
                    "subject": "user",
                    "referentSense": "fairy_tale_gift_deliverer",
                    "timeLabel": "between ages seven and eight (as stated)",
                    "sourceEvidence": [
                        {"sourceKey": "S1", "quote": "I believed in Santa Claus when I was 7"},
                        {"sourceKey": "S2", "quote": "I stopped believing in Santa Claus when I was 8"}
                    ],
                    "inference": "The two records report opposing belief states at different stated ages."
                }
            ],
            "source_evidence": [],
            "unresolved": [{
                "question": "Does a literal gift-deliverer exist?",
                "reason": "The belief reports do not establish external existence.",
                "examinedSources": ["S1", "S2"]
            }]
        }).to_string(),
    };
    let report = synthesize_brief(
        "test-session",
        normalize_recall_result(&santa_result()).unwrap(),
        &provider,
        &ModelConfig::new("mock-model"),
    )
    .await;
    assert_eq!(report.status, RecallBriefStatus::Synthesized);
    assert_eq!(
        report.findings[0].claim_kind,
        RecallClaimKind::ReportedBelief
    );
    assert_eq!(
        report.findings[1].claim_kind,
        RecallClaimKind::ReportedBelief
    );
    assert_eq!(report.findings[2].claim_kind, RecallClaimKind::Inference);
    assert!(report.rendered.contains("## Inference\n\n- Inference:"));
    assert!(report
        .rendered
        .contains("Does a literal gift-deliverer exist?"));
    assert!(!report.rendered.contains("Verified world fact"));

    let mut failed_payload = santa_payload();
    failed_payload["MemorySearchResult"]["meta"]["recall"]["partial"] = json!(false);
    failed_payload["MemorySearchResult"]["meta"]["facet_coverage"]["facets_failed"] =
        json!(["literal gift-deliverer"]);
    let failed_report = synthesize_brief(
        "test-session",
        normalize_recall_result(&CallToolResult::structured(failed_payload)).unwrap(),
        &provider,
        &ModelConfig::new("mock-model"),
    )
    .await;
    assert_eq!(failed_report.status, RecallBriefStatus::Partial);
    assert!(failed_report.notice.contains("Partial recall"));
    assert!(failed_report.rendered.contains("recall was partial"));
}

#[tokio::test]
async fn source_assertion_and_historical_referent_do_not_overwrite_the_belief_reports() {
    let mut payload = santa_payload();
    add_santa_source(
        &mut payload,
        "03",
        3,
        "Literal Santa source assertion",
        "Santa was never literally real",
    );
    add_santa_source(
        &mut payload,
        "04",
        1,
        "Historical Santa inspiration",
        "Saint Nicholas was a historical bishop",
    );
    let provider = MockProvider {
        response: json!({
            "findings": [
                {"claimKind":"reported_belief","statement":"You reported believing in Santa Claus at age seven.","subject":"user","referentSense":"fairy_tale_gift_deliverer","timeLabel":"age seven (as stated)","sourceEvidence":[{"sourceKey":"S1","quote":"I believed in Santa Claus when I was 7"}],"inference":null},
                {"claimKind":"reported_belief","statement":"You reported no longer believing in Santa Claus at age eight.","subject":"user","referentSense":"fairy_tale_gift_deliverer","timeLabel":"age eight (as stated)","sourceEvidence":[{"sourceKey":"S2","quote":"I stopped believing in Santa Claus when I was 8"}],"inference":null},
                {"claimKind":"reported_world_assertion","statement":"A memory asserts Santa was never literally real.","subject":"Santa Claus","referentSense":"fairy_tale_gift_deliverer","timeLabel":null,"sourceEvidence":[{"sourceKey":"S3","quote":"Santa was never literally real"}],"inference":null},
                {"claimKind":"historical_referent_report","statement":"A memory reports Saint Nicholas as a historical bishop.","subject":"Saint Nicholas","referentSense":"historical_person","timeLabel":null,"sourceEvidence":[{"sourceKey":"S4","quote":"Saint Nicholas was a historical bishop"}],"inference":null}
            ],
            "source_evidence": [],
            "unresolved": [{"question":"Does a literal gift-deliverer exist?","reason":"These reports do not independently verify external existence.","examinedSources":["S1","S2","S3","S4"]}]
        })
        .to_string(),
    };
    let report = synthesize_brief(
        "test-session",
        normalize_recall_result(&CallToolResult::structured(payload)).unwrap(),
        &provider,
        &ModelConfig::new("mock-model"),
    )
    .await;
    assert_eq!(
        report.status,
        RecallBriefStatus::Synthesized,
        "{}",
        report.notice
    );
    assert_eq!(report.source_evidence.len(), 4);
    assert_eq!(
        report.findings[2].claim_kind,
        RecallClaimKind::ReportedWorldAssertion
    );
    assert_eq!(
        report.findings[3].claim_kind,
        RecallClaimKind::HistoricalReferentReport
    );
    assert_ne!(
        report.findings[2].referent_sense,
        report.findings[3].referent_sense
    );
    assert!(report.rendered.contains("S1"));
    assert!(report.rendered.contains("S2"));
    assert!(report.rendered.contains("S3"));
    assert!(report.rendered.contains("S4"));
}

#[tokio::test]
async fn fabricated_citation_falls_back_to_exact_evidence_only() {
    let provider = MockProvider {
        response: json!({
            "findings": [{
                "claimKind": "reported_belief",
                "statement": "You reported believing in Santa Claus.",
                "subject": "user",
                "referentSense": "unspecified",
                "timeLabel": null,
                "sourceEvidence": [{"sourceKey": "S99", "quote": "I believed in Santa Claus when I was 7"}],
                "inference": null
            }],
            "source_evidence": [],
            "unresolved": []
        }).to_string(),
    };
    let report = synthesize_brief(
        "test-session",
        normalize_recall_result(&santa_result()).unwrap(),
        &provider,
        &ModelConfig::new("mock-model"),
    )
    .await;
    assert_eq!(report.status, RecallBriefStatus::EvidenceOnly);
    assert!(report.findings.is_empty());
    assert!(report.rendered.contains("## Inference\n\nNone."));
}

#[tokio::test]
async fn a_belief_quote_cannot_be_marked_as_a_verified_world_fact() {
    let provider = MockProvider {
        response: json!({
            "findings": [{
                "claimKind": "reported_world_assertion",
                "statement": "The source proves Santa Claus was a verified real gift-deliverer.",
                "subject": "Santa Claus",
                "referentSense": "fairy_tale_gift_deliverer",
                "timeLabel": null,
                "sourceEvidence": [{"sourceKey": "S1", "quote": "I believed in Santa Claus when I was 7"}],
                "inference": null
            }],
            "source_evidence": [{"sourceKey": "S2", "quote": "I stopped believing in Santa Claus when I was 8"}],
            "unresolved": []
        }).to_string(),
    };
    let report = synthesize_brief(
        "test-session",
        normalize_recall_result(&santa_result()).unwrap(),
        &provider,
        &ModelConfig::new("mock-model"),
    )
    .await;
    assert_eq!(report.status, RecallBriefStatus::EvidenceOnly);
    assert!(report.findings.is_empty());
    assert!(report.notice.contains("model_evidence_invalid"));
}
