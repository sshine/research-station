use std::collections::{HashMap, HashSet};

use research_core::{
    Claim, ClaimId, Evidence, PassageId, Plan, SourceId, SourceRecord, Subquestion,
    SubquestionCoverage, SupportStatus, Synthesis,
};

/// Deterministic extractive synthesis. It quotes stored source text instead of
/// generating prose, so every claim is auditable by construction. Phase 3 can
/// replace this with a model that emits structured claims against evidence IDs.
pub fn synthesize(
    plan: &Plan,
    sources: &[SourceRecord],
    contents: &HashMap<SourceId, String>,
) -> Synthesis {
    let mut claims = Vec::new();
    let mut coverage = Vec::with_capacity(plan.subquestions.len());
    let mut seen_quotes: HashSet<String> = HashSet::new();

    for subquestion in &plan.subquestions {
        let assigned: Vec<&SourceRecord> = sources
            .iter()
            .filter(|source| source.subquestion_id == subquestion.id)
            .collect();

        let mut claim_count = 0;
        for source in assigned.iter().take(2) {
            let Some(content) = contents.get(&source.id) else {
                continue;
            };
            let Some(quote) = choose_quote(content, subquestion) else {
                continue;
            };
            if !seen_quotes.insert(quote.clone()) {
                continue;
            }

            claims.push(Claim {
                id: ClaimId::new(),
                subquestion_id: subquestion.id,
                text: quote.clone(),
                evidence: vec![Evidence {
                    source_id: source.id,
                    passage_id: PassageId::new(),
                    quote,
                }],
                confidence: 0.6,
                support: SupportStatus::Supported,
            });
            claim_count += 1;
        }

        coverage.push(SubquestionCoverage {
            subquestion_id: subquestion.id,
            claim_count,
            source_count: assigned.len(),
        });
    }

    Synthesis { claims, coverage }
}

/// Pick the highest-scoring exact sentence from `content`.
///
/// The returned string is always a substring of `content`, which lets the
/// auditor verify it without normalization.
fn choose_quote(content: &str, subquestion: &Subquestion) -> Option<String> {
    let keywords = keywords(&subquestion.question);
    let mut best: Option<(usize, String)> = None;

    for sentence in sentences(content) {
        let length = sentence.chars().count();
        if !(80..=500).contains(&length) || !sentence.chars().any(char::is_alphabetic) {
            continue;
        }

        let lower = sentence.to_lowercase();
        let score = keywords
            .iter()
            .filter(|keyword| lower.contains(keyword.as_str()))
            .count();

        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, sentence.to_owned()));
        }
    }

    best.map(|(_, quote)| quote)
}

fn sentences(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;

    for (index, ch) in text.char_indices() {
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let end = index + ch.len_utf8();
            let candidate = text[start..end].trim();
            if !candidate.is_empty() {
                result.push(candidate);
            }
            start = end;
        }
    }

    let tail = text[start..].trim();
    if !tail.is_empty() {
        result.push(tail);
    }

    result
}

fn keywords(question: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "what", "when", "where", "which", "does", "with", "this", "that", "from", "have", "been",
        "about", "into", "their", "there", "they", "them", "than", "then", "also", "over", "under",
        "more", "most", "such", "some", "will", "would", "could", "should",
    ];

    question
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| word.len() > 3)
        .map(str::to_lowercase)
        .filter(|word| !STOP.contains(&word.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use research_core::{JobBudget, PlanConstraints, PlanId, SourceClass, SubquestionId};

    fn plan() -> Plan {
        Plan {
            id: PlanId::new(),
            version: 1,
            raw_topic: "Tavily".to_owned(),
            sharpened_topic: "Tavily".to_owned(),
            subquestions: vec![Subquestion {
                id: SubquestionId::new(),
                question: "What are the limitations of Tavily?".to_owned(),
                source_class: SourceClass::Secondary,
                query_seeds: vec!["Tavily limitations".to_owned()],
                success_criterion: "Two sources".to_owned(),
            }],
            constraints: PlanConstraints::default(),
            budget: JobBudget::default(),
            created_at: Utc::now(),
        }
    }

    fn source(subquestion: &Subquestion) -> SourceRecord {
        SourceRecord {
            id: SourceId::new(),
            subquestion_id: subquestion.id,
            url: "https://example.com/a".to_owned(),
            canonical_url: "https://example.com/a".to_owned(),
            title: "Example".to_owned(),
            publisher: "example.com".to_owned(),
            published_date: None,
            retrieved_at: Utc::now(),
            source_type: SourceClass::Secondary,
            content_hash: "hash".to_owned(),
            quality_rationale: "test".to_owned(),
            independence_group: "example.com".to_owned(),
            content_path: "notes/a.md".to_owned(),
            excerpt: String::new(),
        }
    }

    #[test]
    fn quotes_are_exact_substrings_of_the_source() {
        let plan = plan();
        let content = "Tavily is a search API for agents. \
            Its main limitation is that advanced search costs more credits than basic search, \
            which matters for large research jobs.";
        let source = source(&plan.subquestions[0]);
        let contents = HashMap::from([(source.id, content.to_owned())]);

        let synthesis = synthesize(&plan, &[source], &contents);
        assert_eq!(synthesis.claims.len(), 1);
        let quote = &synthesis.claims[0].evidence[0].quote;
        assert!(content.contains(quote.as_str()));
    }

    #[test]
    fn short_fragments_do_not_become_claims() {
        let plan = plan();
        let content = "Too short.";
        let source = source(&plan.subquestions[0]);
        let contents = HashMap::from([(source.id, content.to_owned())]);

        let synthesis = synthesize(&plan, &[source], &contents);
        assert!(synthesis.claims.is_empty());
        assert_eq!(synthesis.coverage[0].claim_count, 0);
    }
}
