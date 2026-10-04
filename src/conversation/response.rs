use super::{ResponseError, Speaker};
use crate::characters::CharacterExpression;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Gentle,
    Playful,
    Direct,
    Reserved,
    Warm,
    Terse,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SocialIntent {
    #[default]
    Acknowledge,
    Agree,
    Disagree,
    DeclineInvitation,
    AcceptInvitation,
    AskQuestion,
    Joke,
    Tease,
    ChangeTopic,
    EndConversation,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipSignal {
    #[default]
    Neutral,
    Warm,
    Competitive,
    Cool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SocialResponse {
    pub dialogue: String,
    pub expression: Option<CharacterExpression>,
    pub tone: Tone,
    pub intent: SocialIntent,
    /// A presentation hint, never an instruction to change stored scores.
    pub relationship_signal: RelationshipSignal,
    pub continuation: bool,
    pub silent: bool,
}
impl SocialResponse {
    pub fn presentation_expression(&self) -> Option<CharacterExpression> {
        self.expression
            .or(match self.intent {
                SocialIntent::Joke | SocialIntent::AcceptInvitation => {
                    Some(CharacterExpression::Happy)
                }
                SocialIntent::Tease => Some(CharacterExpression::Confident),
                SocialIntent::AskQuestion | SocialIntent::Disagree => {
                    Some(CharacterExpression::Thinking)
                }
                SocialIntent::DeclineInvitation => Some(CharacterExpression::Neutral),
                _ => None,
            })
            .or(match self.tone {
                Tone::Playful | Tone::Warm => Some(CharacterExpression::Happy),
                Tone::Terse | Tone::Reserved => Some(CharacterExpression::Neutral),
                _ => None,
            })
    }
}
pub fn parse_social_response(
    raw: &str,
    expected: Speaker,
    max_chars: usize,
) -> Result<SocialResponse, ResponseError> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| ResponseError::Malformed)?;
    let object = value.as_object().ok_or(ResponseError::Malformed)?;
    const ALLOWED: [&str; 8] = [
        "speaker",
        "dialogue",
        "expression",
        "interaction_type",
        "tone",
        "social_intent",
        "relationship_signal",
        "conversation_continuation",
    ];
    if object
        .keys()
        .any(|k| !ALLOWED.contains(&k.as_str()) && k != "silent")
    {
        return Err(ResponseError::InvalidMetadata);
    }
    fn boolean(value: &serde_json::Value, key: &str, default: bool) -> Result<bool, ResponseError> {
        value.get(key).map_or(Ok(default), |v| {
            v.as_bool().ok_or(ResponseError::InvalidMetadata)
        })
    }
    fn metadata<T: serde::de::DeserializeOwned + Default>(
        value: &serde_json::Value,
        key: &str,
    ) -> Result<T, ResponseError> {
        value.get(key).map_or(Ok(T::default()), |v| {
            serde_json::from_value(v.clone()).map_err(|_| ResponseError::InvalidMetadata)
        })
    }
    let silent = boolean(&value, "silent", false)?;
    let (dialogue, expression) = super::parse_dialogue(raw, expected, max_chars, silent)?;
    if silent && (!dialogue.is_empty() || expression.is_some()) {
        return Err(ResponseError::InvalidMetadata);
    }
    let intent: SocialIntent = metadata(&value, "social_intent")?;
    Ok(SocialResponse {
        dialogue,
        expression,
        tone: metadata(&value, "tone")?,
        intent,
        relationship_signal: metadata(&value, "relationship_signal")?,
        continuation: boolean(&value, "conversation_continuation", true)?
            && !silent
            && intent != SocialIntent::EndConversation,
        silent,
    })
}
