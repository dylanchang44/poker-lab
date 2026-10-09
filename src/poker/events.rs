use super::{cards::Card, state::*};

/// Privileged in-memory history for future replay. Live UI/NPCs use Observation,
/// never this stream: CardsDealt and CardBurned contain secret information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    HandStarted {
        number: u64,
        seed: u64,
        dealer: Seat,
        stacks: Vec<Chips>,
        blinds: [Chips; 2],
    },
    BlindPosted {
        seat: Seat,
        amount: Chips,
    },
    CardsDealt {
        seat: Seat,
        cards: [Card; 2],
    },
    /// Public opportunity immediately before a validated action. No cards/equity.
    DecisionOffered {
        seat: Seat,
        phase: Phase,
        pot: Chips,
        to_call: Chips,
        street_bet: Chips,
        can_raise: bool,
    },
    PlayerActed {
        seat: Seat,
        phase: Phase,
        action: Action,
        paid: Chips,
        street_total: Chips,
        pot: Chips,
        stacks: Vec<Chips>,
    },
    BettingRoundCompleted {
        phase: Phase,
    },
    CardBurned {
        card: Card,
    },
    CommunityCardsDealt {
        phase: Phase,
        cards: Vec<Card>,
    },
    UncalledBetReturned {
        seat: Seat,
        amount: Chips,
    },
    ShowdownStarted {
        cards: Vec<Option<[Card; 2]>>,
    },
    PotAwarded {
        seat: Seat,
        amount: Chips,
    },
    HandCompleted {
        outcome: Outcome,
        stacks: Vec<Chips>,
    },
    MatchCompleted {
        winner: Seat,
    },
    PlayerEliminated {
        seat: Seat,
    },
}
