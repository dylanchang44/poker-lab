use super::{Strategy, equity, profiles::Personality};
use crate::poker::{Action, Observation, Phase};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Clone)]
pub struct PersonalityStrategy {
    pub personality: Personality,
    pub samples: u32,
    rng: ChaCha8Rng,
}
impl PersonalityStrategy {
    pub fn new(personality: Personality, seed: u64, samples: u32) -> Self {
        Self {
            personality,
            samples: samples.clamp(16, 4096),
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}
impl Strategy for PersonalityStrategy {
    fn decide(&mut self, v: &Observation) -> Option<Action> {
        if v.actor != Some(v.seat) {
            return None;
        }
        let hole = v.hole_cards?;
        let p = self.personality;
        let i = v.seat.index();
        let legal = v.legal;
        let opponents = v.in_hand.iter().filter(|x| **x).count().saturating_sub(1);
        // Position is relative to remaining opponents, not empty/folded seats.
        let ring: Vec<_> = (1..=v.stacks.len())
            .map(|n| (v.dealer.index() + n) % v.stacks.len())
            .filter(|j| v.in_hand[*j])
            .collect();
        let position = ring.iter().position(|j| *j == i).unwrap_or(0) as f64
            / ring.len().saturating_sub(1).max(1) as f64;
        let selection = 26.0 + p.selectivity * 34.0
            - position * p.positional_awareness * 10.0
            - ((3 - opponents.min(3)) as f64) * 6.0;
        let starting = equity::starting_strength(hole);
        // Preflop discipline cannot be bypassed by a lucky equity sample.
        if v.phase == Phase::PreFlop && starting < selection && !legal.check {
            return Some(Action::Fold);
        }
        let equity = equity::estimate(v, self.samples, &mut self.rng);
        let call = legal.call.unwrap_or(0);
        // Exclude chips above our eligible contribution cap from pot odds.
        let cap = v.contributions[i] + call;
        let contestable: u64 = v
            .contributions
            .iter()
            .map(|c| u64::from((*c).min(cap)))
            .sum::<u64>()
            + u64::from(call);
        let odds = if call == 0 {
            0.0
        } else {
            f64::from(call) / contestable.max(1) as f64
        };
        let margin = 0.10 - p.risk_tolerance * 0.10;
        let draw = equity::draws(hole, &v.board);
        let has_draw = draw.flush || draw.straight;
        let share = 1.0 / (opponents + 1) as f64;
        let value = equity > share + 0.20 - p.aggression * 0.10
            && (v.phase != Phase::PreFlop || starting > selection + 5.0);
        let last_raiser = v.history.iter().rev().find(|h| {
            h.phase == Phase::PreFlop && matches!(h.action, Action::RaiseTo(_) | Action::BetTo(_))
        });
        let dry = v.board.len() >= 3
            && !v
                .board
                .iter()
                .any(|c| v.board.iter().filter(|b| b.suit == c.suit).count() >= 3);
        let cbet = v.phase == Phase::Flop
            && last_raiser.is_some_and(|h| h.seat == v.seat)
            && dry
            && opponents <= 2
            && equity > share * 0.75
            && self.rng.random::<f64>() < p.continuation;
        let bluff = call == 0
            && opponents <= 2
            && (has_draw
                || (dry
                    && hole
                        .iter()
                        .any(|c| c.rank == crate::poker::cards::Rank::Ace)))
            && self.rng.random::<f64>() < p.bluff_frequency;
        if let Some(range) = legal.wager {
            let aggressive =
                (value && self.rng.random::<f64>() < 0.30 + p.aggression * 0.65) || cbet || bluff;
            let effective = v
                .stacks
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i && v.in_hand[*j])
                .map(|(j, n)| n.saturating_add(v.street_bets[j]))
                .max()
                .unwrap_or(0);
            let base = v.street_bets[i] + v.to_call;
            let increment = (u64::from(v.pot.saturating_add(call)) * u64::from(p.bet_size_percent)
                / 100) as u32;
            let to = base
                .saturating_add(increment)
                .min(effective)
                .clamp(range.min_to, range.max_to);
            let paid = to - v.street_bets[i];
            // Speculation cannot turn a weak hand into a stack-sized shove.
            let safe_size = equity >= 0.70
                || u64::from(paid) * 100 <= u64::from(v.stacks[i]) * if value { 65 } else { 20 };
            if aggressive && safe_size {
                return Some(if range.is_raise {
                    Action::RaiseTo(to)
                } else {
                    Action::BetTo(to)
                });
            }
        }
        if legal.all_in
            && equity >= 0.72
            && starting >= selection
            && v.stacks[i] <= v.pot.saturating_mul(2)
        {
            return Some(Action::AllIn);
        }
        if legal.check {
            return Some(Action::Check);
        }
        if legal.call.is_some() && equity >= odds + margin {
            return Some(Action::Call);
        }
        Some(Action::Fold)
    }
}
