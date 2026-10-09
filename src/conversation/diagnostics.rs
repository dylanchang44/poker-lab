//! Host-owned diagnostics. Never retain provider bodies, URLs, prompts or secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogueIssue {
    ModelUnavailable,
    ModelNotLoaded,
    ModelSelection,
    Timeout,
    OutputTokenLimit,
    InvalidJson,
    UnsupportedSchema,
    RequestRejected,
    EmptyDialogue,
    DialogueTooLong,
    RepeatedReply,
    StaleResponse,
    WorkerUnavailable,
    InvalidReply,
}
impl DialogueIssue {
    pub(super) fn classify(error: &str) -> Self {
        match error {
            e if e.contains("worker unavailable") => Self::WorkerUnavailable,
            e if e.contains("timeout") || e.contains("timed out") => Self::Timeout,
            e if e.contains("no loaded") => Self::ModelNotLoaded,
            e if e.contains("multiple loaded") => Self::ModelSelection,
            e if e.contains("token limit") => Self::OutputTokenLimit,
            e if e.contains("network") => Self::ModelUnavailable,
            e if e.contains("HTTP status") || e.contains("API key") => Self::RequestRejected,
            "Malformed" | "invalid provider response" | "invalid model discovery response" => {
                Self::InvalidJson
            }
            "InvalidMetadata" | "InvalidType" | "WrongSpeaker" | "InvalidExpression" => {
                Self::UnsupportedSchema
            }
            "Empty" | "provider returned no dialogue" => Self::EmptyDialogue,
            "TooLong" => Self::DialogueTooLong,
            _ => Self::InvalidReply,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::ModelUnavailable => "Scripted fallback: server offline",
            Self::ModelNotLoaded => "Scripted fallback: load a model",
            Self::ModelSelection => "Scripted fallback: select a model",
            Self::Timeout => "Scripted fallback: timed out",
            Self::OutputTokenLimit => "Scripted fallback: token limit",
            Self::InvalidJson => "Scripted fallback: invalid JSON",
            Self::UnsupportedSchema => "Scripted fallback: unsupported response schema",
            Self::RequestRejected => "Scripted fallback: API rejected request",
            Self::EmptyDialogue => "Scripted fallback: empty dialogue",
            Self::DialogueTooLong => "Scripted fallback: dialogue too long",
            Self::RepeatedReply => "Scripted fallback: repeated reply",
            Self::StaleResponse => "Reply discarded: earlier hand/session",
            Self::WorkerUnavailable => "Scripted fallback: worker unavailable",
            Self::InvalidReply => "Scripted fallback: invalid reply",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DialogueDiagnostics {
    pub model_replies: u64,
    pub scripted_replies: u64,
    pub fallbacks: u64,
    pub silent_replies: u64,
    pub stale_responses: u64,
    pub last_issue: Option<DialogueIssue>,
}
