//! Public-event-only presentation model. No Bevy, strategy RNG or poker engine access.
use crate::poker::{Action, Seat};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CharacterExpression {
    #[default]
    Neutral,
    Thinking,
    Confident,
    Happy,
    Surprised,
    Disappointed,
    Eliminated,
}
impl CharacterExpression {
    pub const ALL: [Self; 7] = [
        Self::Neutral,
        Self::Thinking,
        Self::Confident,
        Self::Happy,
        Self::Surprised,
        Self::Disappointed,
        Self::Eliminated,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn label(self) -> &'static str {
        [
            "Neutral",
            "Thinking",
            "Confident",
            "Happy",
            "Surprised",
            "Disappointed",
            "Eliminated",
        ][self.index()]
    }
}

pub struct CharacterDefinition {
    pub seat: Seat,
    pub origin: &'static str,
    pub age: u8,
    pub portrait_path: &'static str,
    pub intensity: f32,
    pub raise_line: &'static str,
    pub win_line: &'static str,
    pub lose_line: &'static str,
}
pub const CAST: [CharacterDefinition; 3] = [
    CharacterDefinition {
        seat: Seat::Npc,
        origin: "India",
        age: 32,
        portrait_path: "characters/ananya/expressions.png",
        intensity: 0.35,
        raise_line: "Interesting decision.",
        win_line: "A little patience goes a long way.",
        lose_line: "Well played. A well-timed move.",
    },
    CharacterDefinition {
        seat: Seat::Jax,
        origin: "Sweden",
        age: 24,
        portrait_path: "characters/freya/expressions.png",
        intensity: 1.0,
        raise_line: "Now we're talking!",
        win_line: "That's my kind of hand!",
        lose_line: "All right, you got me this time.",
    },
    CharacterDefinition {
        seat: Seat::Nova,
        origin: "Japan",
        age: 28,
        portrait_path: "characters/yuna/expressions.png",
        intensity: 0.22,
        raise_line: "Let's see where this goes.",
        win_line: "That worked out nicely.",
        lose_line: "Well played.",
    },
];
pub fn definition(seat: Seat) -> Option<&'static CharacterDefinition> {
    CAST.iter().find(|c| c.seat == seat)
}

/// A deliberately narrow event type: no hole cards, deck, seed or hand equity.
#[derive(Clone, Debug, PartialEq)]
pub enum PresentationEvent {
    HandStarted {
        number: u64,
        stacks: Vec<u32>,
    },
    Acted {
        seat: Seat,
        action: Action,
    },
    Settled {
        pot: u32,
        awards: Vec<u32>,
        shown: Vec<bool>,
        stacks: Vec<u32>,
    },
}

/// A future dialogue producer supplies this same value. Epochs reject late replies.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueLine {
    pub session: u64,
    pub hand: u64,
    pub speaker: Seat,
    pub text: String,
    pub expression: Option<CharacterExpression>,
    pub duration: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharacterState {
    pub expression: CharacterExpression,
    pub active: bool,
    pub folded: bool,
    pub eliminated: bool,
    pub reaction_left: f32,
    pub emphasis: f32,
    pub action_text: String,
    pub action_left: f32,
}
impl CharacterState {
    fn react(&mut self, expression: CharacterExpression, seconds: f32) {
        if self.eliminated {
            return;
        }
        self.expression = expression;
        self.reaction_left = seconds;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PresentationState {
    pub session: u64,
    pub hand: u64,
    pub seats: [CharacterState; 4],
    pub dialogue: Option<DialogueLine>,
    pub dialogue_left: f32,
}
impl PresentationState {
    pub fn reset(&mut self, session: u64) {
        *self = Self {
            session,
            ..Self::default()
        };
    }
    pub fn set_actor(&mut self, actor: Option<Seat>) {
        for (i, state) in self.seats.iter_mut().enumerate() {
            state.active = actor.is_some_and(|s| s.index() == i);
            if state.eliminated {
                state.expression = CharacterExpression::Eliminated;
            } else if state.reaction_left <= 0.0 {
                state.expression = if state.active {
                    CharacterExpression::Thinking
                } else {
                    CharacterExpression::Neutral
                };
            }
        }
    }
    pub fn tick(&mut self, seconds: f32) {
        let seconds = seconds.max(0.0);
        for s in &mut self.seats {
            s.emphasis = (s.emphasis - seconds * 1.5).max(0.0);
            s.action_left = (s.action_left - seconds).max(0.0);
            s.reaction_left = (s.reaction_left - seconds).max(0.0);
            if s.eliminated {
                s.expression = CharacterExpression::Eliminated;
            } else if s.reaction_left == 0.0 {
                s.expression = if s.active {
                    CharacterExpression::Thinking
                } else {
                    CharacterExpression::Neutral
                };
            }
        }
        self.dialogue_left = (self.dialogue_left - seconds).max(0.0);
        if self.dialogue_left == 0.0 {
            self.dialogue = None;
        }
    }
    pub fn say(&mut self, line: DialogueLine) -> bool {
        if line.session != self.session
            || line.hand != self.hand
            || definition(line.speaker).is_none()
            || line.text.trim().is_empty()
            || !line.duration.is_finite()
            || line.duration <= 0.0
        {
            return false;
        }
        let duration = line.duration.min(15.0);
        if let Some(expression) = line.expression {
            self.seats[line.speaker.index()].react(expression, duration);
        }
        self.dialogue_left = duration;
        self.dialogue = Some(line);
        true
    }
    fn sample_line(&mut self, seat: Seat, text: &'static str) {
        self.say(DialogueLine {
            session: self.session,
            hand: self.hand,
            speaker: seat,
            text: text.into(),
            expression: None,
            duration: 3.4,
        });
    }
    pub fn apply(&mut self, event: &PresentationEvent) {
        match event {
            PresentationEvent::HandStarted { number, stacks } => {
                self.hand = *number;
                self.dialogue = None;
                self.dialogue_left = 0.0;
                self.seats = std::array::from_fn(|i| {
                    let eliminated = stacks.get(i).is_none_or(|s| *s == 0);
                    CharacterState {
                        eliminated,
                        expression: if eliminated {
                            CharacterExpression::Eliminated
                        } else {
                            CharacterExpression::Neutral
                        },
                        ..Default::default()
                    }
                });
            }
            PresentationEvent::Acted { seat, action } => {
                let Some(c) = definition(*seat) else {
                    return;
                };
                let state = &mut self.seats[seat.index()];
                state.emphasis = 1.0;
                state.action_text = action.to_string();
                state.action_left = 2.5;
                state.folded = *action == Action::Fold;
                let raise = matches!(
                    action,
                    Action::BetTo(_) | Action::RaiseTo(_) | Action::AllIn
                );
                state.react(
                    if raise {
                        CharacterExpression::Confident
                    } else {
                        CharacterExpression::Neutral
                    },
                    if raise { 1.2 + c.intensity } else { 0.35 },
                );
                // One short line at a time, not a queue of stale chatter.
                if raise && self.dialogue.is_none() {
                    self.sample_line(*seat, c.raise_line);
                }
            }
            PresentationEvent::Settled {
                pot,
                awards,
                shown,
                stacks,
            } => {
                let recipients = awards.iter().filter(|a| **a > 0).count();
                for c in &CAST {
                    let i = c.seat.index();
                    let state = &mut self.seats[i];
                    let Some(stack) = stacks.get(i) else {
                        continue;
                    };
                    let award = awards[i];
                    if award > 0 {
                        let expression = if recipients > 1 {
                            CharacterExpression::Surprised
                        } else if *pot >= 100 {
                            CharacterExpression::Happy
                        } else {
                            CharacterExpression::Confident
                        };
                        state.react(expression, 2.4 + c.intensity);
                        state.action_text = format!("+{award} chips");
                        state.action_left = 4.0;
                        state.emphasis = 1.0;
                    } else if shown[i] {
                        state.react(CharacterExpression::Disappointed, 2.4 + c.intensity);
                    }
                    if *stack == 0 {
                        state.eliminated = true;
                        state.active = false;
                        state.expression = CharacterExpression::Eliminated;
                        state.reaction_left = 0.0;
                        state.action_text = "Out of chips".into();
                        state.action_left = 4.0;
                    }
                }
                let speaker = CAST
                    .iter()
                    .filter(|c| c.seat.index() < awards.len())
                    .max_by_key(|c| awards[c.seat.index()]);
                if let Some(c) = speaker.filter(|c| awards[c.seat.index()] > 0) {
                    self.sample_line(c.seat, c.win_line);
                } else if let Some(c) = CAST
                    .iter()
                    .find(|c| shown.get(c.seat.index()) == Some(&true))
                {
                    self.sample_line(c.seat, c.lose_line);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
