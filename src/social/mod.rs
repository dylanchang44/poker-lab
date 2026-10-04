//! Temporary social state and stable fictional character direction. No Bevy or poker decisions.
use crate::npc::profiles::NpcId;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Relaxed,
    Cheerful,
    Focused,
    Excited,
    Irritated,
    Disappointed,
    Tired,
}
impl Mood {
    pub fn label(self) -> &'static str {
        match self {
            Self::Relaxed => "Relaxed",
            Self::Cheerful => "Cheerful",
            Self::Focused => "Focused",
            Self::Excited => "Excited",
            Self::Irritated => "Irritated",
            Self::Disappointed => "Disappointed",
            Self::Tired => "Tired",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanIntent {
    Greeting,
    Casual,
    Question,
    Joke,
    Compliment,
    Teasing,
    Flirtation,
    Invitation,
    Apology,
    Criticism,
    Disclosure,
    Poker,
    PastReference,
}

/// This is only deterministic social bookkeeping. The model interprets the actual message.
/// No phrase here selects or scripts an NPC response.
pub fn classify_human(text: &str) -> HumanIntent {
    let lower = text.trim().to_lowercase();
    let words: Vec<_> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| !s.is_empty())
        .collect();
    let has = |needle: &str| words.contains(&needle);
    if ["sorry", "apologize", "apologies"].iter().any(|w| has(w)) {
        HumanIntent::Apology
    } else if ["annoying", "idiot", "terrible", "stupid"]
        .iter()
        .any(|w| has(w))
        && (has("you") || has("you're"))
        && !has("not")
        && !has("never")
    {
        HumanIntent::Criticism
    } else if (has("go") && has("out"))
        || (has("coffee") && ["grab", "want", "like", "join"].iter().any(|w| has(w)))
        || (has("dinner") && has("sometime"))
    {
        HumanIntent::Invitation
    } else if has("remember") || has("yesterday") || has("last") && has("time") {
        HumanIntent::PastReference
    } else if ["beautiful", "clever", "brilliant", "lovely"]
        .iter()
        .any(|w| has(w))
        && !has("not")
        && !has("never")
    {
        HumanIntent::Compliment
    } else if has("flirt") || has("date") {
        HumanIntent::Flirtation
    } else if has("tired") || has("sad") || has("missed") {
        HumanIntent::Disclosure
    } else if has("poker") || has("hand") || has("bluff") || has("chips") {
        HumanIntent::Poker
    } else if has("joke") || has("funny") || has("haha") {
        HumanIntent::Joke
    } else if has("tease") {
        HumanIntent::Teasing
    } else if has("morning") || has("hello") || has("hi") {
        HumanIntent::Greeting
    } else if lower.ends_with('?') {
        HumanIntent::Question
    } else {
        HumanIntent::Casual
    }
}

pub struct SpeechProfile {
    pub id: NpcId,
    pub disposition: &'static str,
    pub preferences: &'static str,
    pub boundaries: &'static str,
    pub goal: &'static str,
    pub initiative: u8,
    pub baseline_energy: u8,
    pub baseline_engagement: u8,
    pub baseline_confidence: u8,
    pub baseline_competition: u8,
}
pub const PROFILES: [SpeechProfile; 3] = [
    SpeechProfile {
        id: NpcId::Ananya,
        disposition: "Composed, analytical, dry humor; concise and direct. Challenges weak reasoning calmly. Teasing is rare and precise.",
        preferences: "Enjoys careful arguments, probability, books and unhurried conversation.",
        boundaries: "Dislikes being rushed into personal disclosure; does not flatter to avoid disagreement.",
        goal: "Understand what motivates this player while maintaining a thoughtful competitive presence.",
        initiative: 2,
        baseline_energy: 52,
        baseline_engagement: 48,
        baseline_confidence: 62,
        baseline_competition: 55,
    },
    SpeechProfile {
        id: NpcId::Freya,
        disposition: "Lively, bold, playful, competitive; quick humor and occasional light flirtation. Comfortable disagreeing openly.",
        preferences: "Enjoys music, spontaneity, a good joke and ambitious plays.",
        boundaries: "Can decline dull plans or excessive pressure; affection is never automatic.",
        goal: "Keep the table lively and test whether the player can match her energy.",
        initiative: 3,
        baseline_energy: 72,
        baseline_engagement: 65,
        baseline_confidence: 65,
        baseline_competition: 70,
    },
    SpeechProfile {
        id: NpcId::Yuna,
        disposition: "Reserved, perceptive, quietly witty; brief answers that grow more open with trust. Disagreement is gentle but real.",
        preferences: "Enjoys music, quiet places, thoughtful games and patient conversation.",
        boundaries: "Does not rush intimacy or answer intrusive questions merely because she was asked.",
        goal: "Observe the player carefully and share only what feels earned.",
        initiative: 1,
        baseline_energy: 48,
        baseline_engagement: 38,
        baseline_confidence: 52,
        baseline_competition: 42,
    },
];
pub fn profile(id: NpcId) -> &'static SpeechProfile {
    &PROFILES[match id {
        NpcId::Ananya => 0,
        NpcId::Freya => 1,
        NpcId::Yuna => 2,
    }]
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MoodState {
    pub mood: Mood,
    pub energy: u8,
    pub engagement: u8,
    pub irritation: u8,
    pub confidence: u8,
    pub competition: u8,
    #[serde(skip)]
    recovery_millis: u32,
}
impl MoodState {
    pub fn baseline(id: NpcId) -> Self {
        let p = profile(id);
        let mut state = Self {
            mood: Mood::Relaxed,
            energy: p.baseline_energy,
            engagement: p.baseline_engagement,
            irritation: 10,
            confidence: p.baseline_confidence,
            competition: p.baseline_competition,
            recovery_millis: 0,
        };
        state.relabel();
        state
    }
    fn adjust(
        &mut self,
        energy: i16,
        engagement: i16,
        irritation: i16,
        confidence: i16,
        competition: i16,
    ) {
        fn add(value: u8, delta: i16) -> u8 {
            (i16::from(value) + delta).clamp(0, 100) as u8
        }
        self.energy = add(self.energy, energy);
        self.engagement = add(self.engagement, engagement);
        self.irritation = add(self.irritation, irritation);
        self.confidence = add(self.confidence, confidence);
        self.competition = add(self.competition, competition);
        self.relabel();
    }
    fn relabel(&mut self) {
        self.mood =
            if self.irritation >= 58 || (self.mood == Mood::Irritated && self.irritation >= 42) {
                Mood::Irritated
            } else if self.energy <= 27 {
                Mood::Tired
            } else if self.confidence <= 27 {
                Mood::Disappointed
            } else if self.energy >= 75 && self.engagement >= 72 {
                Mood::Excited
            } else if self.engagement >= 65 && self.irritation < 30 {
                Mood::Cheerful
            } else if self.competition >= 65 {
                Mood::Focused
            } else {
                Mood::Relaxed
            };
    }
    pub fn hear_player(&mut self, intent: HumanIntent) {
        match intent {
            HumanIntent::Criticism => self.adjust(0, -3, 18, -3, 3),
            HumanIntent::Apology => self.adjust(0, 3, -11, 0, -2),
            HumanIntent::Compliment | HumanIntent::Joke => self.adjust(2, 6, -5, 3, 0),
            HumanIntent::Disclosure => self.adjust(0, 5, -2, 0, 0),
            HumanIntent::Invitation | HumanIntent::Flirtation => self.adjust(1, 3, 0, 1, 0),
            HumanIntent::Poker | HumanIntent::Teasing => self.adjust(0, 2, 0, 0, 3),
            _ => self.adjust(0, 1, -1, 0, 0),
        }
    }
    pub fn settle(&mut self, won: bool, shown_loss: bool, pot: u32) {
        if pot < 150 {
            return;
        }
        if won {
            self.adjust(4, 6, -8, if pot >= 500 { 12 } else { 6 }, 5);
        } else if shown_loss {
            self.adjust(
                -4,
                -2,
                if pot >= 500 { 13 } else { 6 },
                if pot >= 500 { -9 } else { -4 },
                5,
            );
        }
    }
    pub fn advance(&mut self, seconds: f32, id: NpcId) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        self.recovery_millis = self
            .recovery_millis
            .saturating_add((seconds.min(60.0) * 1000.0).round() as u32);
        let ticks = (self.recovery_millis / 30_000).min(20);
        self.recovery_millis %= 30_000;
        if ticks == 0 {
            return;
        }
        let base = Self::baseline(id);
        fn toward(value: &mut u8, target: u8, ticks: u32) {
            if *value < target {
                *value = (*value as u32 + ticks).min(target as u32) as u8;
            } else {
                *value = (*value as i32 - ticks as i32).max(i32::from(target)) as u8;
            }
        }
        toward(&mut self.energy, base.energy, ticks);
        toward(&mut self.engagement, base.engagement, ticks);
        toward(&mut self.irritation, base.irritation, ticks);
        toward(&mut self.confidence, base.confidence, ticks);
        toward(&mut self.competition, base.competition, ticks);
        self.relabel();
    }
}

#[derive(Clone, Debug)]
pub enum PublicSocialEvent {
    HandStarted,
    Settled {
        pot: u32,
        awards: Vec<u32>,
        shown: Vec<bool>,
        net: Vec<i64>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_and_mood_are_stable_and_recover() {
        assert_eq!(PROFILES.map(|p| p.id), NpcId::ALL);
        assert!(profile(NpcId::Freya).initiative > profile(NpcId::Yuna).initiative);
        let mut mood = MoodState::baseline(NpcId::Freya);
        for _ in 0..3 {
            mood.hear_player(HumanIntent::Criticism);
        }
        assert_eq!(mood.mood, Mood::Irritated);
        mood.hear_player(HumanIntent::Apology);
        assert!(mood.irritation >= 10);
        for _ in 0..150 {
            mood.advance(30.0, NpcId::Freya);
        }
        assert_eq!(mood, MoodState::baseline(NpcId::Freya));
    }
    #[test]
    fn wins_and_revealed_losses_change_mood_gradually() {
        let mut win = MoodState::baseline(NpcId::Ananya);
        let base = win.clone();
        win.settle(true, false, 600);
        assert!(win.confidence > base.confidence && win.confidence - base.confidence <= 12);
        let mut loss = base.clone();
        loss.settle(false, true, 600);
        assert!(loss.confidence < base.confidence && loss.irritation > base.irritation);
        let mut folded = base.clone();
        folded.settle(false, false, 600);
        assert_eq!(folded, base);
    }
    #[test]
    fn intents_are_bookkeeping_not_phrase_responses() {
        assert_ne!(
            classify_human("You're not terrible at poker."),
            HumanIntent::Criticism
        );
        assert_ne!(
            classify_human("That's not clever."),
            HumanIntent::Compliment
        );
        assert_eq!(
            classify_human("Would you like to grab coffee?"),
            HumanIntent::Invitation
        );
        assert_eq!(classify_human("I'm sorry."), HumanIntent::Apology);
        assert_eq!(
            classify_human("You're annoying today."),
            HumanIntent::Criticism
        );
        assert_eq!(
            classify_human("Remember yesterday?"),
            HumanIntent::PastReference
        );
    }
}
