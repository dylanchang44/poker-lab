//! Read-only human replay projection. No rules, random dealing or private log storage.
use crate::poker::{Action, Chips, Outcome, Phase, Seat, cards::Card, events::GameEvent};
use std::collections::VecDeque;

pub const RECENT_HANDS: usize = 20;
const MAX_FRAMES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Moment {
    Start,
    Blind {
        seat: Seat,
        amount: Chips,
    },
    YourCards,
    Action {
        seat: Seat,
        action: Action,
        paid: Chips,
    },
    Board,
    Refund {
        seat: Seat,
        amount: Chips,
    },
    Reveal,
    Award {
        seat: Seat,
        amount: Chips,
    },
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub phase: Phase,
    pub moment: Moment,
    pub stacks: Vec<Chips>,
    pub street_bets: Vec<Chips>,
    pub pot: Chips,
    pub board: Vec<Card>,
    pub your_cards: Option<[Card; 2]>,
    pub shown: Vec<Option<[Card; 2]>>,
    pub folded: Vec<bool>,
    pub outcome: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewedHand {
    pub number: u64,
    pub dealer: Seat,
    pub blinds: [Chips; 2],
    pub frames: Vec<Frame>,
}

/// Only completed, sanitized hands are accessible. The pending hand holds no
/// opponent deals, seeds or burns, and never exposes its frames to presentation.
#[derive(Default)]
pub struct ReviewHistory {
    hands: VecDeque<ReviewedHand>,
    pending: Option<ReviewedHand>,
}
impl ReviewHistory {
    pub fn hands(&self) -> &VecDeque<ReviewedHand> {
        &self.hands
    }

    /// Called once per authoritative event by the trusted game host, not by UI.
    pub fn observe(&mut self, event: &GameEvent) {
        if let GameEvent::HandStarted {
            number,
            dealer,
            stacks,
            blinds,
            ..
        } = event
        {
            self.pending = Some(ReviewedHand {
                number: *number,
                dealer: *dealer,
                blinds: *blinds,
                frames: vec![Frame {
                    phase: Phase::PreFlop,
                    moment: Moment::Start,
                    stacks: stacks.clone(),
                    street_bets: vec![0; stacks.len()],
                    pot: 0,
                    board: Vec::new(),
                    your_cards: None,
                    shown: vec![None; stacks.len()],
                    folded: vec![false; stacks.len()],
                    outcome: None,
                }],
            });
            return;
        }
        let Some(hand) = &mut self.pending else {
            return;
        };
        // Reject an exceptionally long review rather than silently omit actions.
        if hand.frames.len() >= MAX_FRAMES {
            self.pending = None;
            return;
        }
        let mut frame = hand.frames.last().unwrap().clone();
        match event {
            GameEvent::BlindPosted { seat, amount } => {
                frame.stacks[seat.index()] -= amount;
                frame.street_bets[seat.index()] += amount;
                frame.pot += amount;
                frame.moment = Moment::Blind {
                    seat: *seat,
                    amount: *amount,
                };
            }
            GameEvent::CardsDealt {
                seat: Seat::Human,
                cards,
            } => {
                frame.your_cards = Some(*cards);
                frame.moment = Moment::YourCards;
            }
            GameEvent::PlayerActed {
                seat,
                phase,
                action,
                paid,
                street_total,
                pot,
                stacks,
            } => {
                frame.phase = *phase;
                frame.stacks.clone_from(stacks);
                frame.pot = *pot;
                frame.street_bets[seat.index()] = *street_total;
                frame.folded[seat.index()] |= *action == Action::Fold;
                frame.moment = Moment::Action {
                    seat: *seat,
                    action: *action,
                    paid: *paid,
                };
            }
            GameEvent::CommunityCardsDealt { phase, cards } => {
                frame.phase = *phase;
                frame.board.extend(cards);
                frame.street_bets.fill(0);
                frame.moment = Moment::Board;
            }
            GameEvent::UncalledBetReturned { seat, amount } => {
                frame.stacks[seat.index()] += amount;
                frame.street_bets[seat.index()] -= amount;
                frame.pot -= amount;
                frame.moment = Moment::Refund {
                    seat: *seat,
                    amount: *amount,
                };
            }
            GameEvent::ShowdownStarted { cards } => {
                // The engine's reveal event already excludes folded/unshown hands.
                // All-in reveals can precede the flop: keep the actual street.
                frame.shown.clone_from(cards);
                frame.moment = Moment::Reveal;
            }
            GameEvent::PotAwarded { seat, amount } => {
                frame.stacks[seat.index()] += amount;
                frame.pot -= amount;
                frame.street_bets.fill(0);
                frame.moment = Moment::Award {
                    seat: *seat,
                    amount: *amount,
                };
            }
            GameEvent::HandCompleted { outcome, stacks } => {
                frame.phase = Phase::HandComplete;
                frame.stacks.clone_from(stacks);
                frame.pot = 0;
                frame.street_bets.fill(0);
                frame.outcome = Some(outcome.clone());
                frame.moment = Moment::Complete;
            }
            // In particular: never copy opponent CardsDealt, CardBurned or seed.
            _ => return,
        }
        hand.frames.push(frame);
        if matches!(event, GameEvent::HandCompleted { .. }) {
            self.hands.push_back(self.pending.take().unwrap());
            while self.hands.len() > RECENT_HANDS {
                self.hands.pop_front();
            }
        }
    }
}

/// A selection is just indices into safe history; navigation cannot act on a match.
#[derive(Default, Clone, Copy)]
pub struct Selection {
    pub hand: usize,
    pub step: usize,
}
impl Selection {
    pub fn latest(history: &ReviewHistory) -> Self {
        Self {
            hand: history.hands.len().saturating_sub(1),
            step: 0,
        }
    }
    pub fn frame<'a>(&self, history: &'a ReviewHistory) -> Option<&'a Frame> {
        history.hands.get(self.hand)?.frames.get(self.step)
    }
    pub fn move_hand(&mut self, history: &ReviewHistory, forward: bool) {
        self.hand = if forward {
            (self.hand + 1).min(history.hands.len().saturating_sub(1))
        } else {
            self.hand.saturating_sub(1)
        };
        self.step = 0;
    }
    pub fn move_step(&mut self, history: &ReviewHistory, forward: bool) {
        let last = history
            .hands
            .get(self.hand)
            .map_or(0, |h| h.frames.len().saturating_sub(1));
        self.step = if forward {
            (self.step + 1).min(last)
        } else {
            self.step.saturating_sub(1)
        };
    }
}

#[cfg(test)]
mod tests;
