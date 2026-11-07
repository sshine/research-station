use research_core::{Plan, RedditMode, SourceClass, Subquestion};

/// The positive phrases that mean "community opinion is relevant".
const OPINION_PHRASES: &[&str] = &[
    "what do people say",
    "what are people saying",
    "what does everyone think",
    "what do users think",
    "what people think",
    "opinions on",
    "opinions about",
    "thoughts on",
    "experiences with",
    "experience with",
    "anyone using",
    "has anyone used",
    "worth it",
    "is it worth",
    "should i buy",
    "should i use",
    "recommendations for",
    "recommend a",
    "complaints about",
    "review of",
    "reviews of",
    "user reviews",
    "pros and cons",
    "real-world experience",
    "user feedback",
    "people's experience",
];

/// Phrases that look like opinion requests but are not community research.
const NEGATIVE_GUARDS: &[&str] = &[
    "literature review",
    "review of the literature",
    "reviews of the literature",
    "systematic review",
    "scoping review",
    "meta-analysis",
    "peer review",
    "code review",
    "pull request review",
    "performance review",
    "review the",
    "reviewing the",
    "official documentation",
    "specification",
    "rfc",
    "source code",
    "api reference",
];

/// Whether the question explicitly asks for Reddit.
pub fn explicit_reddit_mention(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    if lower.contains("reddit") || lower.contains("subreddit") {
        return true;
    }
    // A bare r/<name> reference is also explicit, but avoid matching a path
    // in prose such as "r/rust" only when it is actually a subreddit-shaped
    // token. The slash plus an alphanumeric name is the signal.
    let mut rest = lower.as_str();
    while let Some(position) = rest.find("r/") {
        let after = &rest[position + 2..];
        let name = after
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .next()
            .unwrap_or("");
        if !name.is_empty() {
            return true;
        }
        rest = &after[name.len()..];
    }
    false
}

/// Whether the question asks for first-hand community opinion.
pub fn opinion_intent(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    if NEGATIVE_GUARDS.iter().any(|guard| lower.contains(guard)) {
        return false;
    }
    OPINION_PHRASES.iter().any(|phrase| lower.contains(phrase))
}

/// Whether Reddit should be available for this job.
pub fn should_enable_reddit(mode: RedditMode, question: &str, plan: &Plan) -> bool {
    match mode {
        RedditMode::Never => false,
        RedditMode::Always => true,
        RedditMode::Auto => {
            explicit_reddit_mention(question)
                || opinion_intent(question)
                || plan
                    .subquestions
                    .iter()
                    .any(|subquestion| subquestion.source_class == SourceClass::Community)
        }
    }
}

/// Whether to run one Reddit search before the model loop for this
/// subquestion. `trigger_preseeded` prevents an explicit or opinion trigger
/// from pre-seeding every subquestion when the planner did not create a
/// community subquestion.
pub fn should_preseed_reddit(
    mode: RedditMode,
    question: &str,
    subquestion: &Subquestion,
    trigger_preseeded: bool,
) -> bool {
    match mode {
        RedditMode::Never => false,
        RedditMode::Always => true,
        RedditMode::Auto => {
            subquestion.source_class == SourceClass::Community
                || ((explicit_reddit_mention(question) || opinion_intent(question))
                    && !trigger_preseeded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use research_core::{PlanConstraints, PlanId, SubquestionId};

    fn plan_with(classes: &[SourceClass]) -> Plan {
        Plan {
            id: PlanId::new(),
            version: 1,
            raw_topic: "topic".to_owned(),
            sharpened_topic: "topic".to_owned(),
            subquestions: classes
                .iter()
                .map(|class| Subquestion {
                    id: SubquestionId::new(),
                    question: "q".to_owned(),
                    source_class: *class,
                    query_seeds: vec!["q".to_owned()],
                    success_criterion: "one source".to_owned(),
                })
                .collect(),
            constraints: PlanConstraints::default(),
            budget: research_core::JobBudget::default(),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn explicit_mentions_are_detected() {
        assert!(explicit_reddit_mention("What does Reddit say about NixOS?"));
        assert!(explicit_reddit_mention(
            "Search r/framework for hinge issues"
        ));
        assert!(explicit_reddit_mention("Look at this subreddit"));
        assert!(!explicit_reddit_mention("What is a router?"));
    }

    #[test]
    fn opinion_intent_is_detected() {
        for question in [
            "What do people say about the Framework laptop?",
            "Opinions on NixOS?",
            "Is the Steam Deck worth it?",
            "Reviews of mechanical keyboards?",
            "What are people saying about AI agents?",
        ] {
            assert!(opinion_intent(question), "missed: {question}");
        }
    }

    #[test]
    fn negative_guards_win() {
        for question in [
            "Write a literature review on graph databases",
            "Review the RFC for HTTP semantics",
            "Do a code review of this function",
            "Summarise the peer review comments",
            "Give me a performance review template",
        ] {
            assert!(!opinion_intent(question), "false positive: {question}");
        }
    }

    #[test]
    fn auto_enables_for_explicit_opinion_or_community() {
        let reference = plan_with(&[SourceClass::Reference]);
        assert!(should_enable_reddit(
            RedditMode::Auto,
            "What does Reddit think of NixOS?",
            &reference
        ));
        assert!(should_enable_reddit(
            RedditMode::Auto,
            "Opinions on NixOS?",
            &reference
        ));
        assert!(should_enable_reddit(
            RedditMode::Auto,
            "How does NixOS work?",
            &plan_with(&[SourceClass::Community])
        ));
        assert!(!should_enable_reddit(
            RedditMode::Auto,
            "How does NixOS work?",
            &reference
        ));
        assert!(!should_enable_reddit(
            RedditMode::Never,
            "What does Reddit think?",
            &reference
        ));
    }

    #[test]
    fn preseed_prefers_community_and_only_one_explicit_fallback() {
        let community = Subquestion {
            id: SubquestionId::new(),
            question: "q".to_owned(),
            source_class: SourceClass::Community,
            query_seeds: vec![],
            success_criterion: "one source".to_owned(),
        };
        let reference = Subquestion {
            source_class: SourceClass::Reference,
            ..community.clone()
        };

        assert!(should_preseed_reddit(
            RedditMode::Auto,
            "How does NixOS work?",
            &community,
            false
        ));
        assert!(should_preseed_reddit(
            RedditMode::Auto,
            "What does Reddit say?",
            &reference,
            false
        ));
        assert!(!should_preseed_reddit(
            RedditMode::Auto,
            "What does Reddit say?",
            &reference,
            true
        ));
        assert!(should_preseed_reddit(
            RedditMode::Auto,
            "What do people say about NixOS?",
            &reference,
            false
        ));
    }
}
