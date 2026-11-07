use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::{LlmError, LlmProvider, LlmRequest, LlmResponse, LlmRole, LlmUsage};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum FakeMode {
    #[default]
    Echo,
    Agent,
    AgentReddit,
}

#[derive(Debug, Default)]
struct FakeState {
    mode: FakeMode,
    action_turns: usize,
    responses: VecDeque<String>,
    requests: Vec<LlmRequest>,
}

/// Deterministic model for tests and credential-free development.
///
/// Responses are returned in order. Once the queue is empty, the fake replies
/// with `"fake response"` so callers still get a valid completion.
#[derive(Debug, Clone, Default)]
pub struct FakeLlm {
    state: Arc<Mutex<FakeState>>,
}

impl FakeLlm {
    pub fn new(responses: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                mode: FakeMode::Echo,
                action_turns: 0,
                responses: responses.into_iter().map(Into::into).collect(),
                requests: Vec::new(),
            })),
        }
    }

    pub fn single(response: impl Into<String>) -> Self {
        Self::new([response])
    }

    /// Fake model for the agent loop: emits a valid plan, then alternates a
    /// search and a finish action per subquestion. Synthesis returns no claims,
    /// which exercises the extractive fallback.
    pub fn agent() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                mode: FakeMode::Agent,
                action_turns: 0,
                responses: VecDeque::new(),
                requests: Vec::new(),
            })),
        }
    }

    /// Fake model that plans a community subquestion, emits one Reddit action,
    /// then finishes. Used by the Reddit integration tests.
    pub fn agent_with_reddit() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                mode: FakeMode::AgentReddit,
                action_turns: 0,
                responses: VecDeque::new(),
                requests: Vec::new(),
            })),
        }
    }

    /// Requests observed by the fake, in call order.
    pub fn requests(&self) -> Vec<LlmRequest> {
        self.state
            .lock()
            .expect("fake llm state poisoned")
            .requests
            .clone()
    }

    /// Responses still waiting to be returned.
    pub fn remaining(&self) -> usize {
        self.state
            .lock()
            .expect("fake llm state poisoned")
            .responses
            .len()
    }
}

#[async_trait]
impl LlmProvider for FakeLlm {
    fn id(&self) -> &str {
        "fake"
    }

    fn model(&self) -> &str {
        "fake"
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        if request.messages.is_empty() {
            return Err(LlmError::InvalidRequest(
                "at least one message is required".to_owned(),
            ));
        }

        let mut state = self.state.lock().expect("fake llm state poisoned");
        state.requests.push(request.clone());
        let last_user = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == LlmRole::User)
            .map(|message| message.content.as_str())
            .unwrap_or_default();
        let mode = state.mode;
        let content = if matches!(mode, FakeMode::Agent | FakeMode::AgentReddit) {
            match agent_response(last_user, &mut state.action_turns, mode) {
                Some(response) => response,
                None => state
                    .responses
                    .pop_front()
                    .unwrap_or_else(|| "fake response".to_owned()),
            }
        } else {
            state
                .responses
                .pop_front()
                .unwrap_or_else(|| "fake response".to_owned())
        };

        let prompt_tokens = request
            .messages
            .iter()
            .map(|message| message.content.split_whitespace().count() as u64)
            .sum();
        let completion_tokens = content.split_whitespace().count() as u64;

        Ok(LlmResponse {
            provider: "fake".to_owned(),
            model: "fake".to_owned(),
            content,
            reasoning: None,
            finish_reason: Some("stop".to_owned()),
            usage: LlmUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens.saturating_add(completion_tokens),
                cost_usd: Some(0.0),
                billing_source: Some("fake".to_owned()),
            },
            request_id: Some(format!("fake-{}", state.requests.len())),
            latency_ms: 0,
        })
    }
}

fn agent_response(prompt: &str, action_turns: &mut usize, mode: FakeMode) -> Option<String> {
    if prompt.contains("TASK: PLAN") {
        let plan = if mode == FakeMode::AgentReddit {
            r#"{"sharpened_topic":"fake reddit topic","subquestions":[
                {"question":"What do people say about the fake topic?","source_class":"community","success_criterion":"one community source","query_seeds":["fake reddit community search"]},
                {"question":"What is the main finding?","source_class":"reference","success_criterion":"one source","query_seeds":["fake agent search two"]}
            ]}"#
        } else {
            r#"{"sharpened_topic":"fake agent topic","subquestions":[
                {"question":"What is the main finding?","source_class":"reference","success_criterion":"one source","query_seeds":["fake agent search one"]},
                {"question":"What are the limitations?","source_class":"secondary","success_criterion":"one source","query_seeds":["fake agent search two"]}
            ]}"#
        };
        return Some(plan.to_owned());
    }
    if prompt.contains("TASK: ACTION") {
        let response = if mode == FakeMode::AgentReddit && *action_turns == 0 {
            r#"{"action":"reddit_search","query":"fake reddit community search","include_comments":true,"max_results":3}"#
                .to_owned()
        } else if *action_turns % 2 == 0 {
            format!(
                r#"{{"action":"search","query":"fake agent search {}","max_results":3}}"#,
                *action_turns
            )
        } else {
            r#"{"action":"finish","satisfied":true,"reason":"fake evidence is sufficient"}"#
                .to_owned()
        };
        *action_turns += 1;
        return Some(response);
    }
    if prompt.contains("TASK: SYNTHESIS") {
        return Some(r#"{"claims":[]}"#.to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LlmMessage;

    #[tokio::test]
    async fn returns_scripted_responses_in_order_and_records_requests() {
        let fake = FakeLlm::new(["first", "second"]);
        let first = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("one")]))
            .await
            .unwrap();
        let second = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("two")]))
            .await
            .unwrap();
        let fallback = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("three")]))
            .await
            .unwrap();

        assert_eq!(first.content, "first");
        assert_eq!(second.content, "second");
        assert_eq!(fallback.content, "fake response");
        assert_eq!(fake.remaining(), 0);

        let requests = fake.requests();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].messages[0].content, "one");
        assert_eq!(requests[2].messages[0].content, "three");
    }

    #[tokio::test]
    async fn empty_messages_are_rejected() {
        let fake = FakeLlm::default();
        assert!(matches!(
            fake.complete(LlmRequest::new(Vec::new())).await,
            Err(LlmError::InvalidRequest(_))
        ));
    }

    #[tokio::test]
    async fn reddit_agent_mode_emits_a_community_plan_and_reddit_action() {
        let fake = FakeLlm::agent_with_reddit();
        let plan = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("TASK: PLAN")]))
            .await
            .unwrap();
        assert!(plan.content.contains("\"source_class\":\"community\""));

        let action = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("TASK: ACTION")]))
            .await
            .unwrap();
        assert!(action.content.contains("reddit_search"));
    }

    #[tokio::test]
    async fn agent_mode_emits_plan_search_finish_and_synthesis() {
        let fake = FakeLlm::agent();
        let plan = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("TASK: PLAN")]))
            .await
            .unwrap();
        assert!(plan.content.contains("subquestions"));

        let search = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("TASK: ACTION")]))
            .await
            .unwrap();
        assert!(search.content.contains("search"));

        let finish = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("TASK: ACTION")]))
            .await
            .unwrap();
        assert!(finish.content.contains("finish"));

        let synthesis = fake
            .complete(LlmRequest::new(vec![LlmMessage::user("TASK: SYNTHESIS")]))
            .await
            .unwrap();
        assert_eq!(synthesis.content, r#"{"claims":[]}"#);
    }
}
