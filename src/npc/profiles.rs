use crate::poker::Seat;
use serde::{Deserialize, Serialize};

/// Shared identity for poker, social state and persistence; stable Stage 2 keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NpcId {
    #[serde(rename = "mira")]
    Ananya,
    #[serde(rename = "jax")]
    Freya,
    #[serde(rename = "nova")]
    Yuna,
}
impl NpcId {
    pub const ALL: [Self; 3] = [Self::Ananya, Self::Freya, Self::Yuna];
    pub fn seat(self) -> Seat {
        match self {
            Self::Ananya => Seat::Npc,
            Self::Freya => Seat::Jax,
            Self::Yuna => Seat::Nova,
        }
    }
    pub fn from_seat(seat: Seat) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.seat() == seat)
    }
    pub fn id(self) -> &'static str {
        profile(self.seat()).id
    }
    pub fn name(self) -> &'static str {
        profile(self.seat()).name
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|n| n.id() == value || n.name().eq_ignore_ascii_case(value))
    }
}

/// Policy modifiers, not independent coin-flip probabilities for every action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Personality {
    pub aggression: f64,
    pub selectivity: f64,
    pub bluff_frequency: f64,
    pub risk_tolerance: f64,
    pub bet_size_percent: u32,
    pub continuation: f64,
    pub positional_awareness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub id: &'static str,
    pub name: &'static str,
    pub archetype: &'static str,
    pub accent: [f32; 3],
    pub personality: Personality,
}

pub const ANANYA: Profile = Profile {
    id: "mira",
    name: "Ananya",
    archetype: "The Analyst",
    accent: [0.86, 0.69, 0.36],
    personality: Personality {
        aggression: 0.68,
        selectivity: 0.74,
        bluff_frequency: 0.035,
        risk_tolerance: 0.30,
        bet_size_percent: 60,
        continuation: 0.65,
        positional_awareness: 0.90,
    },
};
pub const FREYA: Profile = Profile {
    id: "jax",
    name: "Freya",
    archetype: "The Gambler",
    accent: [0.98, 0.54, 0.36],
    personality: Personality {
        aggression: 0.93,
        selectivity: 0.20,
        bluff_frequency: 0.16,
        risk_tolerance: 0.80,
        bet_size_percent: 85,
        continuation: 0.80,
        positional_awareness: 0.50,
    },
};
pub const YUNA: Profile = Profile {
    id: "nova",
    name: "Yuna",
    archetype: "The Observer",
    accent: [0.69, 0.60, 0.84],
    personality: Personality {
        aggression: 0.18,
        selectivity: 0.82,
        bluff_frequency: 0.01,
        risk_tolerance: 0.18,
        bet_size_percent: 45,
        continuation: 0.20,
        positional_awareness: 0.65,
    },
};
/// A neutral fourth strategy stands in for the human during batch simulations.
pub const BASELINE: Profile = Profile {
    id: "baseline",
    name: "Baseline",
    archetype: "Simulation reference",
    accent: [0.9, 0.72, 0.39],
    personality: Personality {
        aggression: 0.50,
        selectivity: 0.50,
        bluff_frequency: 0.06,
        risk_tolerance: 0.50,
        bet_size_percent: 60,
        continuation: 0.50,
        positional_awareness: 0.65,
    },
};
// Stable IDs and aliases preserve Stage 2 strategy/seat compatibility.
pub use ANANYA as MIRA;
pub use FREYA as JAX;
pub use YUNA as NOVA;
pub const PROFILES: [Profile; 4] = [BASELINE, ANANYA, FREYA, YUNA];

pub fn profile(seat: Seat) -> Profile {
    PROFILES[seat.index()]
}
pub fn display_name(seat: Seat) -> &'static str {
    if seat == Seat::Human {
        "You"
    } else {
        profile(seat).name
    }
}
