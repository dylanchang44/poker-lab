use crate::poker::Seat;

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

pub const MIRA: Profile = Profile {
    id: "mira",
    name: "Mira",
    archetype: "The Analyst",
    accent: [0.43, 0.65, 0.94],
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
pub const JAX: Profile = Profile {
    id: "jax",
    name: "Jax",
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
pub const NOVA: Profile = Profile {
    id: "nova",
    name: "Nova",
    archetype: "The Observer",
    accent: [0.52, 0.83, 0.68],
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
pub const PROFILES: [Profile; 4] = [BASELINE, MIRA, JAX, NOVA];

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
