use super::{ConversationConfig, InteractionType, ProviderKind, Speaker, TurnRequest};
use serde_json::{Value, json};
use std::sync::Arc;

pub trait DialogueProvider: Send + Sync {
    /// Returns the JSON object emitted by the model, not an executable command.
    fn respond(&self, request: &TurnRequest) -> Result<String, String>;
}

pub fn configured_provider(config: &ConversationConfig) -> Arc<dyn DialogueProvider> {
    match config.provider {
        ProviderKind::Mock => Arc::new(MockProvider),
        ProviderKind::Local | ProviderKind::Remote => Arc::new(OpenAiCompatible {
            config: config.clone(),
        }),
    }
}

pub struct MockProvider;
impl DialogueProvider for MockProvider {
    fn respond(&self, request: &TurnRequest) -> Result<String, String> {
        let (line, expression) = match (request.speaker, request.kind) {
            (Speaker::Ananya, InteractionType::Reply) => (
                "I see your point. What would you have done in my seat?",
                "thinking",
            ),
            (Speaker::Freya, InteractionType::Reply) => {
                ("Now you've got my attention. Tell me more.", "happy")
            }
            (Speaker::Yuna, InteractionType::Reply) => {
                ("Perhaps. There is more to it than the cards.", "thinking")
            }
            (Speaker::Ananya, _) => (
                "Interesting decision. Let's see how it develops.",
                "confident",
            ),
            (Speaker::Freya, _) => (
                "Now we're talking! This table needed a little energy.",
                "happy",
            ),
            (Speaker::Yuna, _) => ("A quiet hand can still tell a story.", "neutral"),
            (Speaker::Human, _) => return Err("human is not an NPC speaker".into()),
        };
        Ok(json!({
            "speaker": request.speaker.id(),
            "dialogue": line,
            "expression": expression,
            "interaction_type": request.kind.as_str(),
        })
        .to_string())
    }
}

pub struct OpenAiCompatible {
    config: ConversationConfig,
}
impl DialogueProvider for OpenAiCompatible {
    fn respond(&self, request: &TurnRequest) -> Result<String, String> {
        let system = super::system_prompt(request.speaker);
        let context =
            serde_json::to_string(&request.context).map_err(|_| "context encoding failed")?;
        let recent = request
            .history
            .iter()
            .map(|m| format!("{}: {}", m.speaker.label(), m.text))
            .collect::<Vec<_>>()
            .join("\n");
        let user = format!(
            "Public table snapshot: {context}\nRecent table conversation:\n{recent}\nInteraction: {}. {}\nIf interjecting, address the most recent NPC naturally. Respond as {} in one short utterance. Return only JSON with speaker, dialogue, expression, interaction_type.",
            request.kind.as_str(),
            request
                .player_text
                .as_deref()
                .unwrap_or("Comment on the public table situation if it feels natural."),
            request.speaker.label(),
        );
        let body = json!({
            "model": self.config.model,
            "messages": [{"role":"system","content":system},{"role":"user","content":user}],
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_output_tokens,
            "stream": false,
        });
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(self.config.timeout()))
            .max_redirects(0)
            .build()
            .new_agent();
        let endpoint = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let mut call = agent.post(&endpoint);
        if let Ok(key) = std::env::var(&self.config.api_key_env)
            && !key.is_empty()
        {
            call = call.header("Authorization", format!("Bearer {key}"));
        } else if self.config.provider == ProviderKind::Remote {
            return Err("remote API key is not configured".into());
        }
        let mut response = call
            .send_json(body)
            .map_err(|e| format!("provider request failed: {}", short_error(&e)))?;
        let value: Value = response
            .body_mut()
            .with_config()
            .limit(16_384)
            .read_json()
            .map_err(|_| "invalid provider response".to_string())?;
        value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "provider returned no dialogue".into())
    }
}

fn short_error(error: &ureq::Error) -> &'static str {
    match error {
        ureq::Error::Timeout(_) => "timeout",
        ureq::Error::StatusCode(_) => "HTTP status",
        _ => "network or protocol error",
    }
}
