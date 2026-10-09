//! Offline evaluation: identical engine/strategy boundary, no Bevy or delays.
use super::{PersonalityStrategy, Strategy, profiles::PROFILES};
use super::{
    adaptation::Adaptation,
    opponent::{Observer, OpponentModel},
    profiles::NpcId,
};
use crate::poker::{Action, FourPlayerMatch, Phase, events::GameEvent};
use rand::{Rng, SeedableRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanStyle {
    TightPassive,
    LooseAggressive,
    CallingStation,
    Overfolder,
}
impl HumanStyle {
    pub const ALL: [Self; 4] = [
        Self::TightPassive,
        Self::LooseAggressive,
        Self::CallingStation,
        Self::Overfolder,
    ];
    fn act(self, v: &crate::poker::Observation, rng: &mut rand_chacha::ChaCha8Rng) -> Action {
        let legal = v.legal;
        let start = super::equity::starting_strength(v.hole_cards.unwrap());
        let raise = match self {
            Self::TightPassive => start > 78.0 && rng.random_bool(0.35),
            Self::LooseAggressive => rng.random_bool(0.65),
            Self::CallingStation => false,
            Self::Overfolder => {
                v.phase == Phase::PreFlop && v.to_call <= v.blinds[1] && rng.random_bool(0.35)
            }
        };
        if raise && let Some(r) = legal.wager {
            let to =
                (v.street_bets[0] + v.to_call + v.pot.max(v.blinds[1])).clamp(r.min_to, r.max_to);
            return if r.is_raise {
                Action::RaiseTo(to)
            } else {
                Action::BetTo(to)
            };
        }
        if legal.check {
            return Action::Check;
        }
        if legal.call.is_some() {
            let fold = match self {
                Self::CallingStation => false,
                Self::LooseAggressive => rng.random_bool(0.15),
                Self::TightPassive => start < 58.0,
                Self::Overfolder => v.to_call > v.blinds[1] && rng.random_bool(0.85),
            };
            if !fold {
                return Action::Call;
            }
        }
        Action::Fold
    }
}
#[derive(Debug, Default, PartialEq)]
pub struct AdaptiveBatch {
    pub model: OpponentModel,
    pub rows: [Stats; 4],
    pub first_adaptation: [Option<u32>; 3],
    pub changed_decisions: [u64; 3],
    pub bluff_actions: [u64; 3],
    pub wager_percent_sum: [u64; 3],
    pub wagers: [u64; 3],
    pub final_reads: [Adaptation; 3],
}
/// Same engine, statistics collector and personality strategy as the original
/// batch runner. Rebuy the entire table when anyone busts so all three NPCs
/// receive comparable observation opportunities throughout the batch.
pub fn simulate_adaptive(
    hands: u32,
    seed: u64,
    samples: u32,
    style: HumanStyle,
    switch_to: Option<HumanStyle>,
) -> AdaptiveBatch {
    let mut result = AdaptiveBatch::default();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let mut bots: [PersonalityStrategy; 4] = std::array::from_fn(|i| {
        PersonalityStrategy::new(
            PROFILES[i].personality,
            seed.wrapping_add(i as u64 + 1),
            samples,
        )
    });
    let mut game = FourPlayerMatch::new(seed);
    let mut observer = Observer::default();
    for h in 0..hands {
        if game.phase() == Phase::MatchComplete || game.stacks().contains(&0) {
            game = FourPlayerMatch::new(seed.wrapping_add(u64::from(h) * 100_003));
        }
        let cursor = game.history().len();
        for (i, npc) in NpcId::ALL.into_iter().enumerate() {
            let read = Adaptation::for_player(npc, &result.model);
            if !read.active.is_empty() && result.first_adaptation[i].is_none() {
                result.first_adaptation[i] = Some(h);
            }
            bots[i + 1].adaptation = read;
        }
        game.start_next_hand().unwrap();
        let current = if h >= hands / 2 {
            switch_to.unwrap_or(style)
        } else {
            style
        };
        let mut turns = 0;
        while let Some(seat) = game.actor() {
            turns += 1;
            assert!(turns < 1000, "betting failed to terminate");
            let v = game.observe(seat);
            let action = if seat.index() == 0 {
                current.act(&v, &mut rng)
            } else {
                let i = seat.index();
                let mut base = bots[i].clone();
                base.adaptation = Adaptation::default();
                let before = base.decide(&v).unwrap();
                let action = bots[i].decide(&v).unwrap();
                result.changed_decisions[i - 1] += u64::from(before != action);
                result.bluff_actions[i - 1] += u64::from(bots[i].last_bluff);
                if let Action::BetTo(to) | Action::RaiseTo(to) = action {
                    result.wagers[i - 1] += 1;
                    result.wager_percent_sum[i - 1] +=
                        u64::from(to.saturating_sub(v.street_bets[i] + v.to_call)) * 100
                            / u64::from(v.pot + v.to_call).max(1);
                }
                action
            };
            game.act(seat, action)
                .expect("all proposed actions validated");
            assert_eq!(game.stacks().iter().sum::<u32>() + game.pot(), 4000);
        }
        let events = &game.history()[cursor..];
        for event in events {
            if let Some((_, sample)) = observer.observe(event) {
                result.model.record(sample);
            }
        }
        collect(&mut result.rows, &[0, 1, 2, 3], events);
    }
    result.final_reads = NpcId::ALL.map(|npc| Adaptation::for_player(npc, &result.model));
    result
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub hands: u64,
    pub vpip: u64,
    pub pfr: u64,
    pub actions: u64,
    pub aggressive: u64,
    pub folds: u64,
    pub showdowns: u64,
    pub committed: u64,
    pub hands_won: u64,
    pub net_chips: i64,
    pub matches_won: u64,
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    pub completed: u32,
    pub truncated: u32,
    pub hands: u64,
    pub rows: [Stats; 4],
}

pub fn simulate(matches: u32, seed: u64, samples: u32, hand_limit: u32) -> Batch {
    let mut result = Batch::default();
    for match_no in 0..matches {
        let match_seed = seed.wrapping_add(u64::from(match_no) * 100_003);
        let identity: [usize; 4] = std::array::from_fn(|seat| (seat + match_no as usize) % 4);
        let mut bots: [PersonalityStrategy; 4] = std::array::from_fn(|i| {
            PersonalityStrategy::new(
                PROFILES[identity[i]].personality,
                match_seed.wrapping_add(i as u64 + 1),
                samples,
            )
        });
        let mut game = FourPlayerMatch::new(match_seed);
        for _ in 0..hand_limit {
            if game.phase() == Phase::MatchComplete {
                break;
            }
            let cursor = game.history().len();
            game.start_next_hand().unwrap();
            result.hands += 1;
            while let Some(seat) = game.actor() {
                let view = game.observe(seat);
                let action = bots[seat.index()]
                    .decide(&view)
                    .expect("valid decision turn");
                game.act(seat, action)
                    .expect("strategy respects legal actions");
                assert_eq!(game.stacks().iter().sum::<u32>() + game.pot(), 4000);
            }
            collect(&mut result.rows, &identity, &game.history()[cursor..]);
        }
        if game.phase() == Phase::MatchComplete {
            result.completed += 1;
        } else {
            result.truncated += 1;
        }
    }
    result
}

fn collect(rows: &mut [Stats; 4], identity: &[usize; 4], events: &[GameEvent]) {
    let mut vpip = [false; 4];
    let mut pfr = [false; 4];
    let mut committed = [0u64; 4];
    let mut high = 0;
    let mut initial = [0; 4];
    for event in events {
        match event {
            GameEvent::HandStarted { stacks, blinds, .. } => {
                high = blinds[1];
                initial.copy_from_slice(stacks);
                for (i, stack) in stacks.iter().enumerate() {
                    if *stack > 0 {
                        rows[identity[i]].hands += 1;
                    }
                }
            }
            GameEvent::BlindPosted { seat, amount } => {
                committed[seat.index()] += u64::from(*amount)
            }
            GameEvent::PlayerActed {
                seat,
                phase,
                action,
                paid,
                street_total,
                ..
            } => {
                let i = seat.index();
                let row = &mut rows[identity[i]];
                row.actions += 1;
                committed[i] += u64::from(*paid);
                if *action == Action::Fold {
                    row.folds += 1;
                }
                let raise = *street_total > high
                    && matches!(
                        action,
                        Action::AllIn | Action::BetTo(_) | Action::RaiseTo(_)
                    );
                if raise {
                    row.aggressive += 1;
                }
                if *phase == Phase::PreFlop {
                    vpip[i] |= *paid > 0;
                    pfr[i] |= raise;
                }
                high = high.max(*street_total);
            }
            GameEvent::CommunityCardsDealt { .. } => high = 0,
            GameEvent::UncalledBetReturned { seat, amount } => {
                committed[seat.index()] -= u64::from(*amount)
            }
            GameEvent::ShowdownStarted { cards } => {
                for (i, hand) in cards.iter().enumerate() {
                    if hand.is_some() {
                        rows[identity[i]].showdowns += 1;
                    }
                }
            }
            GameEvent::HandCompleted { outcome, stacks } => {
                for i in 0..4 {
                    let row = &mut rows[identity[i]];
                    row.vpip += u64::from(vpip[i]);
                    row.pfr += u64::from(pfr[i]);
                    row.committed += committed[i];
                    row.hands_won += u64::from(outcome.awards[i] > 0);
                    row.net_chips += i64::from(stacks[i]) - i64::from(initial[i]);
                }
            }
            GameEvent::MatchCompleted { winner } => rows[identity[winner.index()]].matches_won += 1,
            _ => (),
        }
    }
}
