use super::{cards::Card, hand::HandValue};

pub type Chips = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seat {
    Human,
    /// Seat 1; the legacy Stage 1 name is kept for source compatibility.
    /// Display identities are configured in npc::profiles, not poker rules.
    Npc,
    Jax,
    Nova,
}
impl Seat {
    pub const ALL: [Self; 4] = [Self::Human, Self::Npc, Self::Jax, Self::Nova];
    pub fn index(self) -> usize {
        match self {
            Self::Human => 0,
            Self::Npc => 1,
            Self::Jax => 2,
            Self::Nova => 3,
        }
    }
    pub fn other(self) -> Self {
        match self {
            Self::Human => Self::Npc,
            Self::Npc => Self::Human,
            _ => panic!("other() is a heads-up helper; use the table's seat ring"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WaitingForHand,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    HandComplete,
    MatchComplete,
}
impl Phase {
    pub fn is_betting(self) -> bool {
        matches!(self, Self::PreFlop | Self::Flop | Self::Turn | Self::River)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::WaitingForHand => "Ready",
            Self::PreFlop => "Preflop",
            Self::Flop => "Flop",
            Self::Turn => "Turn",
            Self::River => "River",
            Self::Showdown => "Showdown",
            Self::HandComplete => "Hand complete",
            Self::MatchComplete => "Match complete",
        }
    }
}

/// Bet/raise amounts are the TOTAL contribution on this street, not extra chips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Fold,
    Check,
    Call,
    BetTo(Chips),
    RaiseTo(Chips),
    AllIn,
}
impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fold => write!(f, "Fold"),
            Self::Check => write!(f, "Check"),
            Self::Call => write!(f, "Call"),
            Self::BetTo(n) => write!(f, "Bet to {n}"),
            Self::RaiseTo(n) => write!(f, "Raise to {n}"),
            Self::AllIn => write!(f, "All-in"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WagerRange {
    pub min_to: Chips,
    pub max_to: Chips,
    pub is_raise: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LegalActions {
    pub fold: bool,
    pub check: bool,
    /// Chips actually paid, capped to the player's remaining stack.
    pub call: Option<Chips>,
    /// Full bet/raise range. A shorter all-in is offered separately.
    pub wager: Option<WagerRange>,
    pub all_in: bool,
}
impl LegalActions {
    pub fn accepts(self, action: Action) -> bool {
        match action {
            Action::Fold => self.fold,
            Action::Check => self.check,
            Action::Call => self.call.is_some(),
            Action::AllIn => self.all_in,
            Action::BetTo(to) | Action::RaiseTo(to) => self.wager.is_some_and(|r| {
                (r.min_to..=r.max_to).contains(&to)
                    && r.is_raise == matches!(action, Action::RaiseTo(_))
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub winner: Option<Seat>,
    pub pot: Chips,
    pub awards: Vec<Chips>,
    pub hands: Option<Vec<Option<HandValue>>>,
    pub folded: Option<Seat>,
    pub pots: Vec<PotResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pot {
    pub amount: Chips,
    pub eligible: Vec<Seat>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PotResult {
    pub pot: Pot,
    pub winners: Vec<Seat>,
    pub awards: Vec<Chips>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicAction {
    pub seat: Seat,
    pub phase: Phase,
    pub action: Action,
    pub paid: Chips,
    pub street_total: Chips,
}

/// An owned, filtered snapshot. No deck, shuffle seed, private event log, or
/// opponent's unrevealed cards can cross the decision interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub seat: Seat,
    pub hole_cards: Option<[Card; 2]>,
    pub revealed_cards: Option<Vec<Option<[Card; 2]>>>,
    pub board: Vec<Card>,
    pub stacks: Vec<Chips>,
    pub street_bets: Vec<Chips>,
    pub contributions: Vec<Chips>,
    pub folded: Vec<bool>,
    pub eliminated: Vec<bool>,
    pub in_hand: Vec<bool>,
    pub pots: Vec<Pot>,
    pub small_blind: Option<Seat>,
    pub big_blind: Seat,
    pub history: Vec<PublicAction>,
    pub pot: Chips,
    pub dealer: Seat,
    pub blinds: [Chips; 2],
    pub phase: Phase,
    pub actor: Option<Seat>,
    pub hand_number: u64,
    pub to_call: Chips,
    pub legal: LegalActions,
    pub last_actions: Vec<Option<Action>>,
    pub outcome: Option<Outcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config<const N: usize = 2> {
    pub starting_stacks: [Chips; N],
    pub small_blind: Chips,
    pub big_blind: Chips,
}
impl<const N: usize> Default for Config<N> {
    fn default() -> Self {
        Self {
            starting_stacks: [1000; N],
            small_blind: 5,
            big_blind: 10,
        }
    }
}
