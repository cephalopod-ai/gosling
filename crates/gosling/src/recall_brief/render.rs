use super::evidence::EvidenceBundle;
use gosling_sdk_types::recall_brief::{
    RecallBriefResponse, RecallBriefStatus, RecallClaimKind, RecallFinding, RecallReceipt,
    RecallUnresolved,
};

pub fn unavailable_brief(
    provider_name: &str,
    model_name: &str,
    reason: &str,
) -> RecallBriefResponse {
    let notice = format!("Recall Brief unavailable ({reason}). No memory claims were synthesized.");
    RecallBriefResponse {
        status: RecallBriefStatus::Unavailable,
        provider_name: provider_name.to_string(),
        model_name: model_name.to_string(),
        findings: Vec::new(),
        source_evidence: Vec::new(),
        unresolved: Vec::new(),
        receipt: None,
        rendered: format!("# Recall Brief\n\n{notice}\n"),
        notice,
    }
}

pub fn evidence_only_brief(
    bundle: EvidenceBundle,
    provider_name: &str,
    model_name: &str,
    reason: &str,
) -> RecallBriefResponse {
    let retrieval_partial = bundle.receipt.as_ref().is_some_and(receipt_reports_partial);
    let status = if bundle.omitted_count > 0 || retrieval_partial {
        RecallBriefStatus::Partial
    } else if bundle.returned_count == 0 {
        RecallBriefStatus::Empty
    } else {
        RecallBriefStatus::EvidenceOnly
    };
    let notice = match status {
        RecallBriefStatus::Partial if bundle.omitted_count > 0 => format!(
            "Partial evidence: {} returned hit(s) could not be bound to an authorized exact revision. Synthesis unavailable ({reason}).",
            bundle.omitted_count
        ),
        RecallBriefStatus::Partial => format!(
            "Partial recall: one or more retrieval lanes failed. Synthesis unavailable ({reason}); showing returned excerpts only."
        ),
        RecallBriefStatus::Empty => "Recall completed with no returned hits. No inference was made.".to_string(),
        _ => format!("Synthesis unavailable ({reason}); showing exact returned excerpts only."),
    };
    let notice = if matches!(
        reason,
        "model_unavailable"
            | "model_response_oversized"
            | "model_response_malformed"
            | "model_evidence_invalid"
    ) {
        format!("{notice} Selected excerpts were submitted to {provider_name} / {model_name} for synthesis.")
    } else {
        notice
    };
    report_response(
        bundle,
        provider_name,
        model_name,
        status,
        Vec::new(),
        Vec::new(),
        notice,
    )
}

pub(super) fn synthesized_brief(
    bundle: EvidenceBundle,
    provider_name: &str,
    model_name: &str,
    findings: Vec<RecallFinding>,
    unresolved: Vec<RecallUnresolved>,
) -> RecallBriefResponse {
    let partial = bundle.receipt.as_ref().is_some_and(receipt_reports_partial);
    let notice = format!(
        "Selected Muninn excerpts were sent to {provider_name} / {model_name}. This report describes returned memory evidence; it does not verify autobiography or external world facts."
    );
    let notice = if partial {
        format!("Partial recall: one or more retrieval lanes failed. {notice}")
    } else {
        notice
    };
    report_response(
        bundle,
        provider_name,
        model_name,
        if partial {
            RecallBriefStatus::Partial
        } else {
            RecallBriefStatus::Synthesized
        },
        findings,
        unresolved,
        notice,
    )
}

fn report_response(
    bundle: EvidenceBundle,
    provider_name: &str,
    model_name: &str,
    status: RecallBriefStatus,
    findings: Vec<RecallFinding>,
    unresolved: Vec<RecallUnresolved>,
    notice: String,
) -> RecallBriefResponse {
    let mut rendered = String::from("# Recall Brief\n\n## Findings\n\n");
    let reported: Vec<_> = findings
        .iter()
        .filter(|finding| finding.claim_kind != RecallClaimKind::Inference)
        .collect();
    if reported.is_empty() {
        rendered.push_str("None.\n");
    } else {
        for finding in reported {
            rendered.push_str(&format!(
                "- {}: {} [{}]\n",
                claim_label(finding.claim_kind),
                one_line(&finding.statement),
                finding
                    .source_evidence
                    .iter()
                    .map(|quote| quote.source_key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            render_finding_quotes(&mut rendered, finding);
        }
    }
    rendered.push_str("\n## Source evidence\n\n");
    if bundle.sources.is_empty() {
        rendered.push_str("None returned.\n");
    } else {
        for source in &bundle.sources {
            rendered.push_str(&format!(
                "- {} — {} ({}; revision {})\n  Citation: {}\n  Returned excerpt:\n",
                source.source_key,
                one_line(&source.title),
                one_line(&source.source_kind),
                source.revision,
                source.resource_uri,
            ));
            for line in source.quote.lines() {
                rendered.push_str(&format!("  > {}\n", escape_markdown(line)));
            }
            if source.content_truncated {
                rendered.push_str("  Excerpt is truncated.\n");
            }
        }
    }
    rendered.push_str("\n## Inference\n\n");
    let inferences: Vec<_> = findings
        .iter()
        .filter(|finding| finding.claim_kind == RecallClaimKind::Inference)
        .collect();
    if inferences.is_empty() {
        rendered.push_str("None.\n");
    } else {
        for finding in inferences {
            rendered.push_str(&format!(
                "- Inference: {} [{}]\n",
                one_line(&finding.statement),
                finding
                    .source_evidence
                    .iter()
                    .map(|quote| quote.source_key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            render_finding_quotes(&mut rendered, finding);
        }
    }
    if !unresolved.is_empty() {
        rendered.push_str("\n## Unresolved\n\n");
        for item in &unresolved {
            rendered.push_str(&format!(
                "- {} — {} [{}]\n",
                one_line(&item.question),
                one_line(&item.reason),
                item.examined_sources.join(", ")
            ));
        }
    }
    if bundle.receipt.as_ref().is_some_and(receipt_is_exceptional) {
        rendered.push_str("\n## Recall receipt\n\n");
        render_receipt(
            &mut rendered,
            bundle.receipt.as_ref().expect("checked above"),
        );
    }
    rendered.push_str(&format!("\n{}\n", notice));
    RecallBriefResponse {
        status,
        provider_name: provider_name.to_string(),
        model_name: model_name.to_string(),
        findings,
        source_evidence: bundle.sources,
        unresolved,
        receipt: bundle.receipt,
        rendered,
        notice,
    }
}

fn claim_label(kind: RecallClaimKind) -> &'static str {
    match kind {
        RecallClaimKind::RecordExists => "Returned record",
        RecallClaimKind::ReportedBelief => "Reported belief",
        RecallClaimKind::ReportedChange => "Reported change",
        RecallClaimKind::ReportedWorldAssertion => "Reported source assertion",
        RecallClaimKind::HistoricalReferentReport => "Reported historical referent",
        RecallClaimKind::Inference => "Inference",
    }
}

fn render_finding_quotes(rendered: &mut String, finding: &RecallFinding) {
    for quote in &finding.source_evidence {
        rendered.push_str(&format!(
            "  > {}: {}\n",
            quote.source_key,
            escape_markdown(&quote.quote)
        ));
    }
}

fn one_line(value: &str) -> String {
    escape_markdown(&value.replace(['\n', '\r'], " "))
        .trim()
        .to_string()
}

fn escape_markdown(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '#' | '!' | '<' | '>' | '|' => {
                escaped.push('\\');
                escaped.push(character);
            }
            character if character.is_control() => escaped.push(' '),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn receipt_is_exceptional(receipt: &RecallReceipt) -> bool {
    receipt_reports_partial(receipt)
        || receipt.selected_count == Some(0)
        || receipt
            .next_cursor
            .as_ref()
            .is_some_and(|cursor| !cursor.is_empty())
        || receipt.generation_changed == Some(true)
        || receipt
            .facets_empty
            .as_ref()
            .is_some_and(|facets| !facets.is_empty())
        || matches!(
            receipt.stop_reason.as_deref(),
            Some("configured_ceiling" | "lanes_exhausted")
        )
        || receipt
            .lanes
            .iter()
            .any(|lane| matches!(lane.state.as_str(), "success_empty" | "unsearched"))
        || matches!((receipt.unique_findings, receipt.min_unique), (Some(found), Some(minimum)) if found < minimum)
}

fn receipt_reports_partial(receipt: &RecallReceipt) -> bool {
    receipt.partial == Some(true)
        || receipt
            .facets_failed
            .as_ref()
            .is_some_and(|facets| !facets.is_empty())
        || receipt.lanes.iter().any(|lane| lane.state == "failed")
}

fn render_receipt(rendered: &mut String, receipt: &RecallReceipt) {
    if receipt_reports_partial(receipt) {
        rendered.push_str("- recall was partial; some lanes failed\n");
    }
    for (label, value) in [
        ("minimum unique", receipt.min_unique),
        ("target unique", receipt.target_unique),
        ("item budget", receipt.item_budget),
        ("selected", receipt.selected_count),
        ("unique findings", receipt.unique_findings),
    ] {
        if let Some(value) = value {
            rendered.push_str(&format!("- {label}: {value}\n"));
        }
    }
    if let Some(reason) = &receipt.stop_reason {
        rendered.push_str(&format!("- stop reason: {}\n", one_line(reason)));
    }
    if let Some(cursor) = &receipt.next_cursor {
        if !cursor.is_empty() {
            rendered.push_str("- continuation available; this page is not exhaustive\n");
        }
    }
    if receipt.generation_changed == Some(true) {
        rendered.push_str("- retrieval generation changed; this page is not exhaustive\n");
    }
    for (label, facets) in [
        ("facets with no returned evidence", &receipt.facets_empty),
        ("failed facets", &receipt.facets_failed),
    ] {
        if let Some(facets) = facets {
            for facet in facets {
                rendered.push_str(&format!("- {label}: {}\n", one_line(facet)));
            }
        }
    }
    for lane in &receipt.lanes {
        if matches!(
            lane.state.as_str(),
            "failed" | "success_empty" | "unsearched"
        ) {
            rendered.push_str(&format!(
                "- {}: {}\n",
                one_line(&lane.lane),
                one_line(&lane.state)
            ));
        }
    }
}
