//! A bounded, public-information conversation host; no Bevy or poker-rule mutations.
mod config;
mod provider;
pub use config::{ConversationConfig, ProviderKind};
pub use provider::{DialogueProvider, MockProvider, configured_provider};

use crate::{
    characters::{CharacterExpression, DialogueLine},
    npc::profiles::display_name,
    poker::{Observation, Seat, state::PublicAction},
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Speaker {
    Human,
    Ananya,
    Freya,
    Yuna,
}
impl Speaker {
    pub fn from_seat(seat: Seat) -> Self {
        match seat {
            Seat::Human => Self::Human,
            Seat::Npc => Self::Ananya,
            Seat::Jax => Self::Freya,
            Seat::Nova => Self::Yuna,
        }
    }
    pub fn seat(self) -> Option<Seat> {
        match self {
            Self::Human => None,
            Self::Ananya => Some(Seat::Npc),
            Self::Freya => Some(Seat::Jax),
            Self::Yuna => Some(Seat::Nova),
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Ananya => "ananya",
            Self::Freya => "freya",
            Self::Yuna => "yuna",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Human => "You",
            Self::Ananya => "Ananya",
            Self::Freya => "Freya",
            Self::Yuna => "Yuna",
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "ananya" => Some(Self::Ananya),
            "freya" => Some(Self::Freya),
            "yuna" => Some(Self::Yuna),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Target {
    Ananya,
    Freya,
    Yuna,
    #[default]
    Table,
}
impl Target {
    pub const ALL: [Self; 4] = [Self::Table, Self::Ananya, Self::Freya, Self::Yuna];
    pub fn label(self) -> &'static str {
        match self {
            Self::Table => "Everyone",
            Self::Ananya => "Ananya",
            Self::Freya => "Freya",
            Self::Yuna => "Yuna",
        }
    }
    fn speaker(self) -> Option<Speaker> {
        match self {
            Self::Table => None,
            Self::Ananya => Some(Speaker::Ananya),
            Self::Freya => Some(Speaker::Freya),
            Self::Yuna => Some(Speaker::Yuna),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionType {
    Reply,
    TableComment,
    Interjection,
}
impl InteractionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reply => "reply",
            Self::TableComment => "table_comment",
            Self::Interjection => "interjection",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub speaker: Speaker,
    pub text: String,
}

/// Whitelist only public fields. Never serialize Observation or GameEvent wholesale.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicContext {
    pub hand: u64,
    pub street: String,
    pub board: Vec<String>,
    pub pot: u32,
    pub stacks: Vec<(String, u32)>,
    pub actor: Option<String>,
    pub recent_actions: Vec<String>,
    pub recent_winners: Vec<String>,
}
pub fn public_context(view: &Observation, winners: &[String]) -> PublicContext {
    PublicContext {
        hand: view.hand_number,
        street: view.phase.label().into(),
        board: view.board.iter().map(ToString::to_string).collect(),
        pot: view.pot,
        stacks: view
            .stacks
            .iter()
            .enumerate()
            .map(|(i, n)| (display_name(Seat::ALL[i]).into(), *n))
            .collect(),
        actor: view.actor.map(|s| display_name(s).into()),
        recent_actions: view
            .history
            .iter()
            .rev()
            .take(5)
            .rev()
            .map(public_action)
            .collect(),
        recent_winners: winners.iter().rev().take(2).rev().cloned().collect(),
    }
}
fn public_action(a: &PublicAction) -> String {
    format!(
        "{} {} {} (+{})",
        display_name(a.seat),
        a.phase.label(),
        a.action,
        a.paid
    )
}

#[derive(Clone, Debug)]
pub struct TurnRequest {
    pub session: u64,
    pub hand: u64,
    pub speaker: Speaker,
    pub kind: InteractionType,
    pub context: PublicContext,
    pub history: Vec<ChatMessage>,
    pub player_text: Option<String>,
    pub may_interject: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ConversationCue {
    pub event_id: usize,
    pub speaker: Speaker,
    pub priority: u8,
}

pub fn system_prompt(speaker: Speaker) -> &'static str {
    match speaker {
        Speaker::Ananya => {
            "You are Ananya, a 32-year-old Indian woman and composed poker analyst. Speak in thoughtful, concise observations with understated humor and occasional gentle challenge. No stereotypes or exaggerated accent. Never claim to know hidden cards. You are speaking socially, not choosing bets. Output exactly one JSON object: speaker=ananya, dialogue (max 180 characters), expression (neutral/thinking/confident/happy/surprised/disappointed), interaction_type (reply/table_comment/interjection). Do not reveal or invent private cards or issue game commands."
        }
        Speaker::Freya => {
            "You are Freya, a 24-year-old Swedish woman, outgoing, playful and competitive. Use lively but concise banter; flirt lightly only when natural. No stereotypes or exaggerated accent. Never claim to know hidden cards. You are speaking socially, not choosing bets. Output exactly one JSON object: speaker=freya, dialogue (max 180 characters), expression (neutral/thinking/confident/happy/surprised/disappointed), interaction_type (reply/table_comment/interjection). Do not reveal or invent private cards or issue game commands."
        }
        Speaker::Yuna => {
            "You are Yuna, a 28-year-old Japanese woman: reserved, attentive and quietly witty. Speak less and choose a few precise words. No stereotypes or exaggerated accent. Never claim to know hidden cards. You are speaking socially, not choosing bets. Output exactly one JSON object: speaker=yuna, dialogue (max 180 characters), expression (neutral/thinking/confident/happy/surprised/disappointed), interaction_type (reply/table_comment/interjection). Do not reveal or invent private cards or issue game commands."
        }
        Speaker::Human => "",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResponseError {
    Malformed,
    WrongSpeaker,
    Empty,
    TooLong,
    InvalidExpression,
    InvalidType,
}
pub fn parse_response(
    raw: &str,
    expected: Speaker,
) -> Result<(String, Option<CharacterExpression>), ResponseError> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| ResponseError::Malformed)?;
    let speaker = value
        .get("speaker")
        .and_then(|v| v.as_str())
        .and_then(Speaker::parse)
        .ok_or(ResponseError::WrongSpeaker)?;
    if speaker != expected {
        return Err(ResponseError::WrongSpeaker);
    }
    let dialogue = value
        .get("dialogue")
        .and_then(|v| v.as_str())
        .ok_or(ResponseError::Empty)?
        .trim();
    if dialogue.is_empty() || dialogue.chars().any(|c| c.is_control()) {
        return Err(ResponseError::Empty);
    }
    if dialogue.chars().count() > 180 {
        return Err(ResponseError::TooLong);
    }
    if let Some(kind) = value.get("interaction_type")
        && !matches!(
            kind.as_str(),
            Some("reply" | "table_comment" | "interjection")
        )
    {
        return Err(ResponseError::InvalidType);
    }
    if value
        .get("expression")
        .is_some_and(|v| !v.is_null() && !v.is_string())
    {
        return Err(ResponseError::InvalidExpression);
    }
    let expression = match value.get("expression").and_then(|v| v.as_str()) {
        None => None,
        Some("neutral") => Some(CharacterExpression::Neutral),
        Some("thinking") => Some(CharacterExpression::Thinking),
        Some("confident") => Some(CharacterExpression::Confident),
        Some("happy") => Some(CharacterExpression::Happy),
        Some("surprised") => Some(CharacterExpression::Surprised),
        Some("disappointed") => Some(CharacterExpression::Disappointed),
        _ => return Err(ResponseError::InvalidExpression),
    };
    Ok((dialogue.into(), expression))
}

struct Pending {
    receiver: Mutex<mpsc::Receiver<Result<String, String>>>,
    request: TurnRequest,
    begun: Instant,
}
struct WorkerGuard(Arc<AtomicBool>);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStatus {
    Mock,
    Ready,
    Working,
    Fallback,
    Disabled,
}

/// One in-flight call and one priority human request; no response can modify poker.
pub struct ConversationManager {
    config: ConversationConfig,
    provider: Arc<dyn DialogueProvider>,
    session: u64,
    history: VecDeque<ChatMessage>,
    pending: Option<Pending>,
    worker_busy: Arc<AtomicBool>,
    queued: Option<TurnRequest>,
    last_spoke: f32,
    last_event: usize,
    event_sequence: u64,
    human_count: u64,
    last_line: [String; 4],
    pub status: ProviderStatus,
}
impl ConversationManager {
    pub fn new(config: ConversationConfig, provider: Arc<dyn DialogueProvider>) -> Self {
        let status = if !config.enabled {
            ProviderStatus::Disabled
        } else if config.provider == ProviderKind::Mock {
            ProviderStatus::Mock
        } else {
            ProviderStatus::Ready
        };
        Self {
            config,
            provider,
            session: 0,
            history: VecDeque::new(),
            pending: None,
            worker_busy: Arc::new(AtomicBool::new(false)),
            queued: None,
            last_spoke: -100.0,
            last_event: 0,
            event_sequence: 0,
            human_count: 0,
            last_line: std::array::from_fn(|_| String::new()),
            status,
        }
    }
    pub fn reset(&mut self, session: u64) {
        self.session = session;
        self.history.clear();
        self.pending = None;
        self.queued = None;
        self.last_spoke = -100.0;
        self.last_event = 0;
        self.event_sequence = 0;
        self.human_count = 0;
        self.last_line = std::array::from_fn(|_| String::new());
        self.status = if !self.config.enabled {
            ProviderStatus::Disabled
        } else if self.config.provider == ProviderKind::Mock {
            ProviderStatus::Mock
        } else {
            ProviderStatus::Ready
        };
    }
    /// A new graphical match starts quietly; the opening cue can speak after a brief pause.
    pub fn reset_at(&mut self, session: u64, now: f32) {
        self.reset(session);
        self.last_spoke = now - 11.0;
    }
    pub fn session(&self) -> u64 {
        self.session
    }
    pub fn history(&self) -> &VecDeque<ChatMessage> {
        &self.history
    }
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }
    pub fn label(&self) -> &'static str {
        match self.status {
            ProviderStatus::Mock => "Mock dialogue",
            ProviderStatus::Ready => "Dialogue online",
            ProviderStatus::Working => "Thinking...",
            ProviderStatus::Fallback => "Dialogue fallback",
            ProviderStatus::Disabled => "Dialogue off",
        }
    }
    fn push(&mut self, line: ChatMessage) {
        self.history.push_back(line);
        while self.history.len() > self.config.history_limit {
            self.history.pop_front();
        }
    }
    pub fn human_message(
        &mut self,
        text: &str,
        target: Target,
        context: PublicContext,
        now: f32,
    ) -> bool {
        let text = text.trim();
        if !self.config.enabled
            || text.is_empty()
            || text.chars().count() > 240
            || text.chars().any(|c| c.is_control())
            || target
                .speaker()
                .and_then(Speaker::seat)
                .is_some_and(|s| s.index() >= context.stacks.len())
        {
            return false;
        }
        self.push(ChatMessage {
            speaker: Speaker::Human,
            text: text.into(),
        });
        let speaker = target.speaker().unwrap_or_else(|| {
            if context.stacks.len() == 2 {
                Speaker::Ananya
            } else {
                self.table_speaker()
            }
        });
        self.human_count += 1;
        let mut request = self.request(speaker, InteractionType::Reply, context, Some(text.into()));
        request.may_interject = target == Target::Table
            && request.context.stacks.len() > 2
            && self.human_count.is_multiple_of(3);
        // Human speech supersedes unsolicited chatter but never spawns parallel calls.
        self.queued = Some(request);
        self.last_spoke = now;
        true
    }
    fn table_speaker(&mut self) -> Speaker {
        let choice = self.event_sequence % 6;
        self.event_sequence += 1;
        match choice {
            0 | 3 | 5 => Speaker::Freya,
            1 | 4 => Speaker::Ananya,
            _ => Speaker::Yuna,
        }
    }
    fn request(
        &self,
        speaker: Speaker,
        kind: InteractionType,
        context: PublicContext,
        player_text: Option<String>,
    ) -> TurnRequest {
        TurnRequest {
            session: self.session,
            hand: context.hand,
            speaker,
            kind,
            context,
            history: self.history.iter().cloned().collect(),
            player_text,
            may_interject: false,
        }
    }
    /// Candidate messages are built by the trusted host from public engine events only.
    pub fn offer_event(
        &mut self,
        event_id: usize,
        speaker: Speaker,
        priority: u8,
        context: PublicContext,
        now: f32,
    ) {
        if event_id <= self.last_event {
            return;
        }
        self.last_event = event_id;
        if speaker
            .seat()
            .is_none_or(|s| s.index() >= context.stacks.len())
        {
            return;
        }
        if !self.config.enabled
            || self.queued.is_some()
            || self.pending.is_some()
            || !self.can_initiate(speaker, priority, now)
        {
            return;
        }
        self.queued = Some(self.request(speaker, InteractionType::TableComment, context, None));
        self.last_spoke = now;
    }
    fn can_initiate(&self, speaker: Speaker, priority: u8, now: f32) -> bool {
        if self.config.initiative_frequency <= 0.0
            || now - self.last_spoke < 14.0 / self.config.initiative_frequency
        {
            return false;
        }
        let rate = match speaker {
            Speaker::Freya => 3,
            Speaker::Ananya => 2,
            Speaker::Yuna => 1,
            Speaker::Human => 0,
        };
        priority >= 2
            || (self.event_sequence + self.last_event as u64).is_multiple_of((6 / rate).max(1))
    }
    pub fn offer_idle(&mut self, context: PublicContext, now: f32) {
        if !self.config.enabled
            || self.config.initiative_frequency <= 0.0
            || self.queued.is_some()
            || self.pending.is_some()
        {
            return;
        }
        let opening = self.last_event > 0
            && self.history.is_empty()
            && now - self.last_spoke >= 14.0 / self.config.initiative_frequency;
        if !opening && now - self.last_spoke < 48.0 / self.config.initiative_frequency {
            return;
        }
        let speaker = if context.stacks.len() == 2 {
            Speaker::Ananya
        } else {
            Speaker::Freya
        };
        self.queued = Some(self.request(speaker, InteractionType::TableComment, context, None));
        self.last_spoke = now;
    }
    fn deliver(
        &mut self,
        request: TurnRequest,
        result: Result<String, String>,
    ) -> Option<DialogueLine> {
        // A queued human message takes precedence over a pending unsolicited comment.
        if self
            .queued
            .as_ref()
            .is_some_and(|q| q.player_text.is_some())
            && request.player_text.is_none()
        {
            return None;
        }
        let (text, expression, fallback) = match result
            .and_then(|s| parse_response(&s, request.speaker).map_err(|e| format!("{e:?}")))
        {
            Ok((text, expression))
                if text != self.last_line[request.speaker.seat().unwrap().index()] =>
            {
                (text, expression, false)
            }
            _ => (
                fallback_line(request.speaker, request.kind).into(),
                None,
                true,
            ),
        };
        self.status = if fallback {
            ProviderStatus::Fallback
        } else if self.config.provider == ProviderKind::Mock {
            ProviderStatus::Mock
        } else {
            ProviderStatus::Ready
        };
        let speaker = request.speaker;
        self.last_line[speaker.seat().unwrap().index()] = text.clone();
        self.push(ChatMessage {
            speaker,
            text: text.clone(),
        });
        if request.may_interject && !fallback && self.queued.is_none() {
            let next = match speaker {
                Speaker::Freya => Speaker::Ananya,
                Speaker::Ananya => Speaker::Yuna,
                _ => Speaker::Freya,
            };
            self.queued =
                Some(self.request(next, InteractionType::Interjection, request.context, None));
        }
        Some(DialogueLine {
            session: self.session,
            hand: request.hand,
            speaker: speaker.seat().unwrap(),
            text,
            expression,
            duration: 5.0,
        })
    }
    pub fn tick(&mut self, current_session: u64, hand: u64) -> Option<DialogueLine> {
        if current_session != self.session {
            self.reset(current_session);
            return None;
        }
        if let Some(pending) = self.pending.take() {
            let received = pending
                .receiver
                .lock()
                .expect("conversation receiver lock")
                .try_recv();
            match received {
                Ok(result) => {
                    if pending.request.session == self.session && pending.request.hand == hand {
                        return self.deliver(pending.request, result);
                    }
                }
                Err(mpsc::TryRecvError::Empty)
                    if pending.begun.elapsed()
                        < self.config.timeout() + std::time::Duration::from_millis(250) =>
                {
                    self.pending = Some(pending)
                }
                Err(_) => {
                    if pending.request.session == self.session && pending.request.hand == hand {
                        return self.deliver(pending.request, Err("provider timed out".into()));
                    }
                }
            }
        }
        if self.pending.is_none()
            && !self.worker_busy.load(Ordering::Acquire)
            && let Some(request) = self.queued.take()
            && request.session == self.session
            && request.hand == hand
        {
            let (sender, receiver) = mpsc::channel();
            let provider = Arc::clone(&self.provider);
            let worker_request = request.clone();
            self.worker_busy.store(true, Ordering::Release);
            let guard = WorkerGuard(Arc::clone(&self.worker_busy));
            match std::thread::Builder::new()
                .name("poker-dialogue".into())
                .spawn(move || {
                    let _guard = guard;
                    let _ = sender.send(provider.respond(&worker_request));
                }) {
                Ok(_) => {
                    self.pending = Some(Pending {
                        receiver: Mutex::new(receiver),
                        request,
                        begun: Instant::now(),
                    });
                    self.status = ProviderStatus::Working;
                }
                Err(_) => return self.deliver(request, Err("worker unavailable".into())),
            }
        }
        if self.pending.is_none() && self.queued.is_none() && self.status == ProviderStatus::Working
        {
            self.status = if self.config.provider == ProviderKind::Mock {
                ProviderStatus::Mock
            } else {
                ProviderStatus::Ready
            };
        }
        None
    }
}
fn fallback_line(speaker: Speaker, kind: InteractionType) -> &'static str {
    match (speaker, kind) {
        (Speaker::Ananya, InteractionType::Reply) => "Interesting thought. Tell me more.",
        (Speaker::Freya, InteractionType::Reply) => "Ha! I like where this is going.",
        (Speaker::Yuna, InteractionType::Reply) => "I'm listening.",
        (Speaker::Ananya, _) => "A curious turn of events.",
        (Speaker::Freya, _) => "Now we're talking!",
        (Speaker::Yuna, _) => "The table has changed.",
        _ => "",
    }
}

#[cfg(test)]
mod tests;
