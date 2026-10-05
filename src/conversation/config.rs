use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Duration};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Mock,
    Local,
    Remote,
}

/// Optional OpenAI-compatible hint. `None` omits the parameter for providers
/// that do not support it; `Off` serializes to the API value "none".
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    #[serde(rename = "none")]
    Off,
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ConversationConfig {
    pub enabled: bool,
    pub provider: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// Name of an environment variable, never the credential itself.
    pub api_key_env: String,
    pub timeout_seconds: u64,
    pub max_output_tokens: u32,
    pub temperature: f32,
    /// 0 disables unsolicited comments; 1 is normal cadence, 2 is talkative.
    pub initiative_frequency: f32,
    pub history_limit: usize,
    pub max_dialogue_chars: usize,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub structured_output: bool,
}
impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: ProviderKind::Mock,
            base_url: "http://127.0.0.1:1234/v1".into(),
            model: "local-model".into(),
            api_key_env: "POKER_LAB_API_KEY".into(),
            timeout_seconds: 12,
            max_output_tokens: 256,
            temperature: 0.7,
            initiative_frequency: 1.0,
            history_limit: 16,
            max_dialogue_chars: 180,
            reasoning_effort: None,
            structured_output: false,
        }
    }
}
impl ConversationConfig {
    /// Normal desktop launches prefer local inference; tests explicitly retain
    /// the deterministic `Default` mock and never contact a model by accident.
    pub fn local_development() -> Self {
        Self {
            provider: ProviderKind::Local,
            model: "auto".into(),
            timeout_seconds: 60,
            max_output_tokens: 512,
            max_dialogue_chars: 240,
            reasoning_effort: Some(ReasoningEffort::Off),
            structured_output: true,
            ..Self::default()
        }
    }
    pub fn for_application(explicit: Option<&Path>, local: &Path) -> Result<Self, String> {
        if let Some(path) = explicit {
            Self::load(Some(path))
        } else if local.exists() {
            Self::load(Some(local))
        } else {
            Ok(Self::local_development())
        }
    }
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let mut config = if let Some(path) = path {
            serde_json::from_str::<Self>(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            Self::default()
        };
        config.validate()?;
        config.history_limit = config.history_limit.min(32);
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.enabled
            && self.provider != ProviderKind::Mock
            && (!(self.base_url.starts_with("http://") || self.base_url.starts_with("https://"))
                || self.base_url.contains(['?', '#', '@'])
                || self.model.trim().is_empty())
        {
            return Err("conversation base_url must be HTTP(S) and model must be set".into());
        }
        if self.enabled
            && self.provider == ProviderKind::Remote
            && !self.base_url.starts_with("https://")
        {
            return Err("remote provider requires HTTPS".into());
        }
        if self.provider == ProviderKind::Remote && self.model == "auto" {
            return Err("remote provider requires an explicit model".into());
        }
        if !(1..=120).contains(&self.timeout_seconds)
            || !(16..=1024).contains(&self.max_output_tokens)
            || !(0.0..=2.0).contains(&self.temperature)
            || !(0.0..=3.0).contains(&self.initiative_frequency)
            || !(4..=32).contains(&self.history_limit)
            || !(80..=360).contains(&self.max_dialogue_chars)
            || !self
                .api_key_env
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err("conversation settings outside supported bounds".into());
        }
        Ok(())
    }
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}
