use std::collections::HashMap;
use std::fmt::Write;

use research_core::{AuditReport, Job, Plan, SourceId, SourceRecord, Synthesis};

/// The original request, written once when the job starts.
pub fn render_brief(job: &Job) -> String {
    let budget = job.request.budget;
    format!(
        "# Research brief\n\n\
         - **Question:** {question}\n\
         - **Depth:** {depth}\n\
         - **Review:** {review}\n\
         - **Job:** `{id}`\n\
         - **Created:** {created}\n\
         - **Output:** `{output}`\n\
         - **Budget:** {queries} queries, {sources} sources, {credits} credits, {seconds}s wall time\n",
        question = job.request.question,
        depth = job.request.depth.as_str(),
        review = match job.request.review {
            research_core::ReviewPolicy::Automatic => "automatic",
            research_core::ReviewPolicy::Manual => "manual",
        },
        id = job.id,
        created = job.created_at.to_rfc3339(),
        output = job.output_dir.display(),
        queries = budget.max_queries,
        sources = budget.max_sources,
        credits = budget.max_credits,
        seconds = budget.max_wall_seconds,
    )
}

pub fn render_plan(plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Plan v{}\n", plan.version);
    let _ = writeln!(out, "- **Plan:** `{}`", plan.id);
    let _ = writeln!(out, "- **Topic:** {}", plan.sharpened_topic);
    let _ = writeln!(out, "- **Created:** {}", plan.created_at.to_rfc3339());
    let _ = writeln!(out, "- **Subquestions:** {}\n", plan.subquestions.len());

    for (index, subquestion) in plan.subquestions.iter().enumerate() {
        let _ = writeln!(out, "## {}. {}\n", index + 1, subquestion.question);
        let _ = writeln!(
            out,
            "- **Source class:** {}",
            subquestion.source_class.as_str()
        );
        let _ = writeln!(
            out,
            "- **Success criterion:** {}",
            subquestion.success_criterion
        );
        let _ = writeln!(
            out,
            "- **Query seeds:** {}",
            subquestion
                .query_seeds
                .iter()
                .map(|seed| format!("`{seed}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        out.push('\n');
    }

    out
}

pub fn render_report(
    plan: &Plan,
    synthesis: &Synthesis,
    sources: &[SourceRecord],
    audit: &AuditReport,
) -> String {
    let mut order: Vec<SourceId> = Vec::new();
    for claim in &synthesis.claims {
        for evidence in &claim.evidence {
            if !order.contains(&evidence.source_id) {
                order.push(evidence.source_id);
            }
        }
    }

    let positions: HashMap<SourceId, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index + 1))
        .collect();

    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", plan.raw_topic);
    let _ = writeln!(
        out,
        "**Coverage:** {}/{} subquestions with supported claims · \
         **Claims:** {}/{} supported by exact quotes\n",
        audit.covered_subquestions,
        audit.total_subquestions,
        audit.supported_claims,
        audit.total_claims
    );

    for (index, subquestion) in plan.subquestions.iter().enumerate() {
        let _ = writeln!(out, "## {}. {}\n", index + 1, subquestion.question);

        let claims: Vec<_> = synthesis
            .claims
            .iter()
            .filter(|claim| claim.subquestion_id == subquestion.id)
            .collect();

        if claims.is_empty() {
            let _ = writeln!(out, "_No supported evidence found._\n");
            continue;
        }

        for claim in claims {
            let _ = writeln!(out, "> {}\n", claim.text.replace('\n', " "));
            let mut cited = Vec::new();
            for evidence in &claim.evidence {
                if let Some(position) = positions.get(&evidence.source_id) {
                    if !cited.contains(position) {
                        cited.push(*position);
                    }
                }
            }
            let citations = cited
                .into_iter()
                .map(|position| format!("[S{position}]"))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(out, "> — {citations}\n");
        }
    }

    let _ = writeln!(out, "## Sources\n");
    for id in &order {
        if let Some(source) = sources.iter().find(|source| source.id == *id) {
            let position = positions[id];
            let _ = writeln!(
                out,
                "{position}. [{}]({}) — {} · {} · retrieved {}",
                escape_link_text(&source.title),
                source.url,
                source.publisher,
                source.source_type.as_str(),
                source.retrieved_at.format("%Y-%m-%d")
            );
        }
    }

    let _ = writeln!(
        out,
        "\n---\n\nAudit: {} (see `audit.md`). Generated deterministically from stored evidence.",
        if audit.passed {
            "passed"
        } else {
            "needs attention"
        }
    );

    out
}

pub fn render_audit(plan: &Plan, synthesis: &Synthesis, audit: &AuditReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Audit\n");
    let _ = writeln!(out, "- **Claims:** {}", audit.total_claims);
    let _ = writeln!(
        out,
        "- **Supported by exact quotes:** {}",
        audit.supported_claims
    );
    let _ = writeln!(
        out,
        "- **Subquestion coverage:** {}/{}",
        audit.covered_subquestions, audit.total_subquestions
    );
    let _ = writeln!(
        out,
        "- **Result:** {}\n",
        if audit.passed {
            "passed"
        } else {
            "needs attention"
        }
    );

    let _ = writeln!(out, "| Subquestion | Sources | Claims |");
    let _ = writeln!(out, "|---|---:|---:|");
    for (index, subquestion) in plan.subquestions.iter().enumerate() {
        let coverage = synthesis
            .coverage
            .iter()
            .find(|coverage| coverage.subquestion_id == subquestion.id);
        let _ = writeln!(
            out,
            "| {}. {} | {} | {} |",
            index + 1,
            subquestion.question.replace('|', "\\|"),
            coverage.map_or(0, |coverage| coverage.source_count),
            coverage.map_or(0, |coverage| coverage.claim_count)
        );
    }

    let _ = writeln!(out, "\n## Findings\n");
    if audit.findings.is_empty() {
        let _ = writeln!(
            out,
            "No unresolved quotes. Every claim is an exact substring of its stored source."
        );
    } else {
        for finding in &audit.findings {
            let _ = writeln!(
                out,
                "- Claim `{}` (source `{}`): {}",
                finding.claim_id, finding.source_id, finding.reason
            );
            let _ = writeln!(out, "  > {}\n", finding.quote.replace('\n', " "));
        }
    }

    out
}

fn escape_link_text(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use research_core::{
        Claim, ClaimId, Evidence, JobBudget, JobId, PassageId, PlanConstraints, PlanId,
        ResearchRequest, SourceClass, Subquestion, SubquestionCoverage, SubquestionId,
        SupportStatus,
    };

    fn fixture() -> (Plan, Synthesis, Vec<SourceRecord>, AuditReport) {
        let subquestion = Subquestion {
            id: SubquestionId::new(),
            question: "What is the topic?".to_owned(),
            source_class: SourceClass::Reference,
            query_seeds: vec!["topic".to_owned()],
            success_criterion: "two sources".to_owned(),
        };
        let plan = Plan {
            id: PlanId::new(),
            version: 1,
            raw_topic: "topic".to_owned(),
            sharpened_topic: "topic".to_owned(),
            subquestions: vec![subquestion.clone()],
            constraints: PlanConstraints::default(),
            budget: JobBudget::default(),
            created_at: Utc::now(),
        };
        let source = SourceRecord {
            id: SourceId::new(),
            subquestion_id: subquestion.id,
            url: "https://example.com/a".to_owned(),
            canonical_url: "https://example.com/a".to_owned(),
            title: "Example".to_owned(),
            publisher: "example.com".to_owned(),
            published_date: None,
            retrieved_at: Utc::now(),
            source_type: SourceClass::Reference,
            content_hash: "hash".to_owned(),
            quality_rationale: "test".to_owned(),
            independence_group: "example.com".to_owned(),
            content_path: "notes/a.md".to_owned(),
            excerpt: String::new(),
        };
        let claim = Claim {
            id: ClaimId::new(),
            subquestion_id: subquestion.id,
            text: "An exact quote.".to_owned(),
            evidence: vec![Evidence {
                source_id: source.id,
                passage_id: PassageId::new(),
                quote: "An exact quote.".to_owned(),
            }],
            confidence: 0.6,
            support: SupportStatus::Supported,
        };
        let synthesis = Synthesis {
            claims: vec![claim],
            coverage: vec![SubquestionCoverage {
                subquestion_id: subquestion.id,
                claim_count: 1,
                source_count: 1,
            }],
        };
        let audit = AuditReport {
            total_claims: 1,
            supported_claims: 1,
            covered_subquestions: 1,
            total_subquestions: 1,
            findings: Vec::new(),
            passed: true,
        };
        (plan, synthesis, vec![source], audit)
    }

    #[test]
    fn report_cites_sources() {
        let (plan, synthesis, sources, audit) = fixture();
        let report = render_report(&plan, &synthesis, &sources, &audit);
        assert!(report.contains("[S1]"));
        assert!(report.contains("https://example.com/a"));
        assert!(report.contains("An exact quote."));
    }

    #[test]
    fn brief_includes_budgets() {
        let mut request = ResearchRequest::new("A question");
        request.budget = JobBudget::for_depth(request.depth);
        let job = Job {
            id: JobId::new(),
            request,
            status: research_core::JobStatus::Queued,
            output_dir: "research/test".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            lease_owner: None,
            lease_expires_at: None,
            attempts: 0,
            last_error: None,
            idempotency_key: None,
            cancel_requested: false,
        };
        let brief = render_brief(&job);
        assert!(brief.contains("A question"));
        assert!(brief.contains("queries"));
    }
}
