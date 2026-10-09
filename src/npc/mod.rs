//! Strategies receive only the same filtered observation a seated player sees.
use crate::poker::{Action, Observation};
mod basic;
pub use basic::BasicNpc;
pub mod equity;
mod personality;
pub mod profiles;
pub use personality::PersonalityStrategy;
pub mod adaptation;
pub mod opponent;
#[cfg(test)]
mod opponent_tests;
pub mod simulation;
#[cfg(test)]
mod tests;

pub trait Strategy {
    /// None means there is no decision to make (not this player's turn).
    fn decide(&mut self, observation: &Observation) -> Option<Action>;
}
