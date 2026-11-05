use std::collections::{HashMap, HashSet};

use research_core::{AuditFinding, AuditReport, Plan, SourceId, Synthesis};

/// Verify every claim against the exact stored text it cites.
///
/// A claim is supported only when all of its quotes are exact substrings of the
/// corresponding note. Missing content, altered quotes, and uncovered
/// subquestions are separate findings.
pub fn audit(
    plan: &Plan,
    synthesis: &Synthesis,
    contents: &HashMap<SourceId, String>,
) -> AuditReport {
    let mut findings = Vec::new();
    let mut supported_claims = 0;
    let mut covered: HashSet<_> = HashSet::new();

    for claim in &synthesis.claims {
        let mut claim_supported = !claim.evidence.is_empty();

        for evidence in &claim.evidence {
            match contents.get(&evidence.source_id) {
                Some(content) if content.contains(&evidence.quote) => {}
                Some(_) => {
                    claim_supported = false;
                    findings.push(AuditFinding {
                        claim_id: claim.id,
                        source_id: evidence.source_id,
                        quote: evidence.quote.clone(),
                        reason: "quote is not an exact substring of the stored source".to_owned(),
                    });
                }
                None => {
                    claim_supported = false;
                    findings.push(AuditFinding {
                        claim_id: claim.id,
                        source_id: evidence.source_id,
                        quote: evidence.quote.clone(),
                        reason: "source content is missing from the bundle".to_owned(),
                    });
                }
            }
        }

        if claim_supported {
            supported_claims += 1;
            covered.insert(claim.subquestion_id);
        }
    }

    let total_subquestions = plan.subquestions.len();
    let covered_subquestions = covered.len();

    AuditReport {
        total_claims: synthesis.claims.len(),
        supported_claims,
        covered_subquestions,
        total_subquestions,
        passed: findings.is_empty()
            && supported_claims == synthesis.claims.len()
            && covered_subquestions == total_subquestions,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use research_core::{
        Claim, ClaimId, Evidence, JobBudget, PassageId, PlanConstraints, PlanId, SourceClass,
        Subquestion, SubquestionCoverage, SubquestionId, SupportStatus, Synthesis,
    };

    fn fixture(quote: &str, content: &str) -> (Plan, Synthesis, HashMap<SourceId, String>) {
        let subquestion = Subquestion {
            id: SubquestionId::new(),
            question: "What is the evidence?".to_owned(),
            source_class: SourceClass::Secondary,
            query_seeds: vec!["evidence".to_owned()],
            success_criterion: "one source".to_owned(),
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
        let source_id = SourceId::new();
        let claim = Claim {
            id: ClaimId::new(),
            subquestion_id: subquestion.id,
            text: quote.to_owned(),
            evidence: vec![Evidence {
                source_id,
                passage_id: PassageId::new(),
                quote: quote.to_owned(),
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
        (
            plan,
            synthesis,
            HashMap::from([(source_id, content.to_owned())]),
        )
    }

    #[test]
    fn exact_quotes_pass() {
        let (plan, synthesis, contents) = fixture(
            "This is an exact quote from the source.",
            "Prefix. This is an exact quote from the source. Suffix.",
        );
        let report = audit(&plan, &synthesis, &contents);
        assert!(report.passed);
        assert_eq!(report.supported_claims, 1);
    }

    #[test]
    fn altered_quotes_fail_the_audit() {
        let (plan, synthesis, contents) = fixture(
            "This quote was edited.",
            "The source says something different.",
        );
        let report = audit(&plan, &synthesis, &contents);
        assert!(!report.passed);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.supported_claims, 0);
    }

    #[test]
    fn missing_sources_fail_the_audit() {
        let (plan, synthesis, _) = fixture("A quote.", "A quote.");
        let report = audit(&plan, &synthesis, &HashMap::new());
        assert!(!report.passed);
        assert_eq!(
            report.findings[0].reason,
            "source content is missing from the bundle"
        );
    }
}
