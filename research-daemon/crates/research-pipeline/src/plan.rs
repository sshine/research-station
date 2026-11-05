use chrono::Utc;
use research_core::{
    Depth, Plan, PlanConstraints, PlanId, ResearchRequest, SourceClass, Subquestion, SubquestionId,
};

/// Phase 2 planner. It is deliberately deterministic: no model call, no
/// network, and the same request produces the same shape of plan every time.
/// Phase 3 replaces this with a schema-validated LLM planner behind the same
/// `Plan` contract.
pub struct RuleBasedPlanner;

impl RuleBasedPlanner {
    pub fn plan(request: &ResearchRequest) -> Plan {
        let topic = sharpened_topic(&request.question);
        let templates = templates(&topic);
        let count = match request.depth {
            Depth::Quick => 2,
            Depth::Thorough => 4,
            Depth::Exhaustive => 6,
        };

        let subquestions = templates
            .into_iter()
            .take(count)
            .map(|template| Subquestion {
                id: SubquestionId::new(),
                question: template.question,
                source_class: template.source_class,
                query_seeds: template.query_seeds,
                success_criterion: template.success_criterion.to_owned(),
            })
            .collect();

        Plan {
            id: PlanId::new(),
            version: 1,
            raw_topic: request.question.trim().to_owned(),
            sharpened_topic: topic,
            subquestions,
            constraints: PlanConstraints {
                include_domains: request.include_domains.clone(),
                exclude_domains: request.exclude_domains.clone(),
            },
            budget: request.budget,
            created_at: Utc::now(),
        }
    }
}

struct Template {
    question: String,
    source_class: SourceClass,
    query_seeds: Vec<String>,
    success_criterion: &'static str,
}

fn templates(topic: &str) -> Vec<Template> {
    let owned = |text: String| text;
    vec![
        Template {
            question: owned(format!("Overview and definitions: {topic}")),
            source_class: SourceClass::Reference,
            query_seeds: vec![
                format!("{topic} overview"),
                format!("what is {topic} definition"),
            ],
            success_criterion: "At least two independent sources define the topic consistently.",
        },
        Template {
            question: owned(format!("Primary sources and documentation: {topic}")),
            source_class: SourceClass::Primary,
            query_seeds: vec![
                format!("{topic} official documentation"),
                format!("{topic} primary source"),
            ],
            success_criterion: "At least one primary source is retrieved.",
        },
        Template {
            question: owned(format!("Limitations, risks, and criticisms: {topic}")),
            source_class: SourceClass::Secondary,
            query_seeds: vec![
                format!("{topic} limitations"),
                format!("{topic} criticism risks"),
            ],
            success_criterion: "At least two sources name a concrete limitation or risk.",
        },
        Template {
            question: owned(format!("Recent developments and comparisons: {topic}")),
            source_class: SourceClass::News,
            query_seeds: vec![
                format!("{topic} recent developments"),
                format!("{topic} comparison alternatives"),
            ],
            success_criterion: "At least one source is dated within the last year, or recency is noted as unavailable.",
        },
        Template {
            question: owned(format!("Alternatives and trade-offs: {topic}")),
            source_class: SourceClass::Secondary,
            query_seeds: vec![
                format!("{topic} alternatives comparison"),
                format!("{topic} versus"),
            ],
            success_criterion: "At least two alternatives are named with a trade-off.",
        },
        Template {
            question: owned(format!("Practitioner experience and case studies: {topic}")),
            source_class: SourceClass::Community,
            query_seeds: vec![
                format!("{topic} practitioner experience"),
                format!("{topic} case study"),
            ],
            success_criterion: "At least one first-hand practitioner account is found.",
        },
    ]
}

fn sharpened_topic(question: &str) -> String {
    let trimmed = question.trim().trim_end_matches(['?', '.', '!']).trim();
    if trimmed.is_empty() {
        "the requested topic".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(depth: Depth) -> ResearchRequest {
        let mut request = ResearchRequest::new("How do Tavily and Kagi compare?");
        request.depth = depth;
        request
    }

    #[test]
    fn depth_controls_the_number_of_subquestions() {
        assert_eq!(
            RuleBasedPlanner::plan(&request(Depth::Quick))
                .subquestions
                .len(),
            2
        );
        assert_eq!(
            RuleBasedPlanner::plan(&request(Depth::Thorough))
                .subquestions
                .len(),
            4
        );
        assert_eq!(
            RuleBasedPlanner::plan(&request(Depth::Exhaustive))
                .subquestions
                .len(),
            6
        );
    }

    #[test]
    fn plans_have_seeds_and_success_criteria() {
        let plan = RuleBasedPlanner::plan(&request(Depth::Thorough));
        assert_eq!(plan.raw_topic, "How do Tavily and Kagi compare?");
        assert_eq!(plan.sharpened_topic, "How do Tavily and Kagi compare");
        assert!(
            plan.subquestions
                .iter()
                .all(|sq| { !sq.query_seeds.is_empty() && !sq.success_criterion.is_empty() })
        );
    }

    #[test]
    fn empty_questions_still_get_a_topic() {
        let plan = RuleBasedPlanner::plan(&ResearchRequest::new("   "));
        assert_eq!(plan.sharpened_topic, "the requested topic");
    }
}
