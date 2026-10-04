pub use crate::npc::profiles::NpcId;
use crate::poker::Seat;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Audience {
    #[default]
    Public,
    Private(NpcId),
}
impl Audience {
    pub fn permits(self, npc: NpcId) -> bool {
        self == Self::Public || self == Self::Private(npc)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub familiarity: i32,
    pub trust: i32,
    pub respect: i32,
    pub tension: i32,
    pub warmth: i32,
}
impl Relationship {
    pub fn apply(&mut self, delta: &Self) {
        self.familiarity = (self.familiarity + delta.familiarity).clamp(0, 1000);
        self.trust = (self.trust + delta.trust).clamp(0, 1000);
        self.respect = (self.respect + delta.respect).clamp(0, 1000);
        self.tension = (self.tension + delta.tension).clamp(0, 1000);
        self.warmth = (self.warmth + delta.warmth).clamp(0, 1000);
    }
    pub fn tier(&self, npc: NpcId) -> &'static str {
        if self.familiarity < 20 {
            "Stranger"
        } else if self.familiarity < 60 {
            "Familiar"
        } else if self.respect >= 80 && self.tension >= 60 {
            match npc {
                NpcId::Ananya => "Respected Rival",
                NpcId::Freya => "Playful Rival",
                NpcId::Yuna => "Friendly Rival",
            }
        } else if self.warmth >= 80 && self.trust >= 50 {
            "Friend"
        } else {
            "Acquaintance"
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MemoryType {
    PokerEvent,
    PlayerPreference,
    SocialInteraction,
    RelationshipMilestone,
    BehavioralObservation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub npc: NpcId,
    pub kind: MemoryType,
    pub summary: String,
    pub importance: i32,
    pub occurrences: i32,
    pub created_at: i64,
    pub last_recalled_at: Option<i64>,
    pub recall_count: i32,
    pub source_event_id: i64,
    pub audience: Audience,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterSnapshot {
    pub npc: NpcId,
    pub relationship: Relationship,
    pub sessions: i64,
    pub hands: i64,
    pub memories: Vec<Memory>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SocialContext {
    pub relationship: Option<Relationship>,
    pub tier: String,
    pub memories: Vec<String>,
}

/// Facts supplied by the trusted host, never inferred from a model's claimed facts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Fact {
    SessionStarted,
    HandStarted {
        hand: u64,
        participants: Vec<NpcId>,
    },
    HandFinished {
        hand: u64,
        pot: u32,
        awards: Vec<u32>,
        public_showdown: bool,
        shown_river_aggression: bool,
    },
    Eliminated {
        hand: u64,
        seat: usize,
    },
    Conversation {
        speaker: String,
        audience: Audience,
        text: String,
    },
    Behavior {
        hand: u64,
        opportunities: u32,
        raises: u32,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservedEvent {
    pub witnesses: Vec<NpcId>,
    pub fact: Fact,
}
#[derive(Clone, Debug)]
pub struct MemoryCandidate {
    pub key: String,
    pub kind: MemoryType,
    pub summary: String,
    pub importance: i32,
    pub delta: Relationship,
    pub audience: Audience,
}

/// Conservative deterministic extraction: preferences remain exact attributed quotes.
pub fn conversation_kind(speaker: &str, text: &str) -> Option<MemoryType> {
    use crate::social::{HumanIntent, classify_human};
    let lower = text.to_lowercase();
    if speaker == "human"
        && [
            "i like ",
            "i enjoy ",
            "i prefer ",
            "my favorite ",
            "i hate ",
            "i don't like ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        Some(MemoryType::PlayerPreference)
    } else if (speaker == "human"
        && matches!(
            classify_human(text),
            HumanIntent::Compliment
                | HumanIntent::Criticism
                | HumanIntent::Apology
                | HumanIntent::Invitation
                | HumanIntent::Disclosure
                | HumanIntent::Flirtation
        ))
        || [
            "thank you",
            "well played",
            "clever",
            "idiot",
            "probability",
            "strategy",
            "funny",
            "haha",
        ]
        .iter()
        .any(|word| lower.contains(word))
        || (speaker == "human" && text.chars().count() >= 40)
    {
        Some(MemoryType::SocialInteraction)
    } else {
        None
    }
}
pub fn derive(npc: NpcId, event: &ObservedEvent) -> Option<MemoryCandidate> {
    if !event.witnesses.contains(&npc) {
        return None;
    }
    let mut delta = Relationship::default();
    let (key, kind, summary, importance) = match &event.fact {
        Fact::HandFinished {
            hand,
            pot,
            awards,
            public_showdown,
            shown_river_aggression,
        } if *pot >= 200 || *shown_river_aggression => {
            let winners = awards
                .iter()
                .enumerate()
                .filter(|(_, a)| **a > 0)
                .map(|(i, a)| format!("{} +{a}", crate::npc::profiles::display_name(Seat::ALL[i])))
                .collect::<Vec<_>>()
                .join(", ");
            if awards.first().copied().unwrap_or(0) > 0 {
                delta.respect = if npc == NpcId::Ananya { 4 } else { 2 };
                delta.tension = if npc == NpcId::Freya { 5 } else { 2 };
            }
            (
                format!("pot:{hand}"),
                MemoryType::PokerEvent,
                format!(
                    "Hand {hand}: a {pot}-chip pot awarded {winners}. {}{}",
                    if *public_showdown {
                        "Public showdown. "
                    } else {
                        "Won without a showdown. "
                    },
                    if *shown_river_aggression {
                        "The player publicly showed high card after river aggression; bluff intent is unknown."
                    } else {
                        ""
                    }
                ),
                if *pot >= 500 || *shown_river_aggression {
                    85
                } else {
                    50
                },
            )
        }
        Fact::Eliminated { hand, seat } => {
            delta.tension = if npc == NpcId::Freya { 6 } else { 3 };
            (
                format!("elimination:{hand}:{seat}"),
                MemoryType::PokerEvent,
                format!(
                    "{} was eliminated after hand {hand}.",
                    crate::npc::profiles::display_name(Seat::ALL[*seat])
                ),
                90,
            )
        }
        Fact::Conversation {
            speaker,
            audience,
            text,
        } => {
            if !audience.permits(npc) {
                return None;
            }
            let kind = conversation_kind(speaker, text)?;
            // Only player conduct changes relationships; NPC-generated claims do not.
            if speaker == "human" {
                delta.familiarity = 1;
                let lower = text.to_lowercase();
                if crate::social::classify_human(text) == crate::social::HumanIntent::Criticism
                    || lower.contains("idiot")
                {
                    delta.trust = -2;
                    delta.warmth = -2;
                } else {
                    delta.trust = 1;
                    delta.warmth = if npc == NpcId::Yuna { 2 } else { 1 };
                    if npc == NpcId::Ananya
                        && (lower.contains("probability") || lower.contains("strategy"))
                    {
                        delta.respect = 3;
                    }
                    if npc == NpcId::Freya && (lower.contains("haha") || lower.contains("risk")) {
                        delta.warmth = 3;
                    }
                }
            }
            (
                format!(
                    "quote:{speaker}:{}",
                    text.split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase()
                ),
                kind,
                format!(
                    "{} said: “{}”",
                    if speaker == "human" {
                        "The player"
                    } else {
                        speaker
                    },
                    text
                ),
                if kind == MemoryType::PlayerPreference {
                    70
                } else {
                    45
                },
            )
        }
        Fact::Behavior {
            opportunities,
            raises,
            ..
        } if *opportunities >= 10 => (
            "behavior:preflop".into(),
            MemoryType::BehavioralObservation,
            format!(
                "In the last {opportunities} observed preflop decisions, the player raised {raises} times. This is an observation, not a prediction."
            ),
            35,
        ),
        _ => return None,
    };
    let audience = if let Fact::Conversation { audience, .. } = event.fact {
        audience
    } else {
        Audience::Public
    };
    let key = format!("{audience:?}:{key}");
    Some(MemoryCandidate {
        key,
        kind,
        summary,
        importance,
        delta,
        audience,
    })
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
