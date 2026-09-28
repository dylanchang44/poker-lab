//! A runtime choice between the original two-seat and new four-seat tables.
use poker_lab::poker::{
    Action, FourPlayerMatch, LegalActions, Observation, Phase, PokerMatch, Seat, events::GameEvent,
};

pub enum MatchEngine {
    HeadsUp(Box<PokerMatch>),
    Four(Box<FourPlayerMatch>),
}
macro_rules! dispatch {
    ($self:ident, $engine:ident, $expression:expr) => {
        match $self {
            Self::HeadsUp($engine) => $expression,
            Self::Four($engine) => $expression,
        }
    };
}
impl MatchEngine {
    pub fn phase(&self) -> Phase {
        dispatch!(self, e, e.phase())
    }
    pub fn actor(&self) -> Option<Seat> {
        dispatch!(self, e, e.actor())
    }
    #[cfg(test)]
    pub fn stacks(&self) -> Vec<poker_lab::poker::Chips> {
        dispatch!(self, e, e.stacks().to_vec())
    }
    pub fn history(&self) -> &[GameEvent] {
        dispatch!(self, e, e.history())
    }
    pub fn observe(&self, seat: Seat) -> Observation {
        dispatch!(self, e, e.observe(seat))
    }
    pub fn legal_actions(&self, seat: Seat) -> LegalActions {
        dispatch!(self, e, e.legal_actions(seat))
    }
    pub fn act(&mut self, seat: Seat, action: Action) -> Result<(), &'static str> {
        dispatch!(self, e, e.act(seat, action))
    }
    pub fn start_next_hand(&mut self) -> Result<(), &'static str> {
        dispatch!(self, e, e.start_next_hand())
    }
}
