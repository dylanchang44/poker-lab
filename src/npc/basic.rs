use super::Strategy;
use crate::poker::{Action, Observation, Phase, hand::evaluate};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub struct BasicNpc {
    rng: ChaCha8Rng,
}
impl BasicNpc {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl Strategy for BasicNpc {
    fn decide(&mut self, view: &Observation) -> Option<Action> {
        if view.actor != Some(view.seat) {
            return None;
        }
        let hole = view.hole_cards?;
        let legal = view.legal;
        let strength = if view.phase == Phase::PreFlop {
            let high = hole[0].rank.max(hole[1].rank) as u32;
            if hole[0].rank == hole[1].rank {
                50 + high * 3
            } else {
                high * 3 + u32::from(hole[0].suit == hole[1].suit) * 12
            }
        } else {
            let mut cards = view.board.clone();
            cards.extend(hole);
            match evaluate(&cards).ok()?.category_index() {
                1 => 20,
                2 => 45,
                3 => 65,
                4 => 75,
                _ => 95,
            }
        };
        let roll = self.rng.random_range(0..100);
        if legal.all_in
            && strength >= 80
            && view.stacks[view.seat.index()] < view.pot.saturating_mul(2)
            && roll < 30
        {
            return Some(Action::AllIn);
        }
        if let Some(range) = legal.wager
            && ((strength >= 55 && roll < 35) || roll < 5)
        {
            let to = view.street_bets[view.seat.index()]
                .saturating_add(view.to_call)
                .saturating_add(view.pot / 2)
                .clamp(range.min_to, range.max_to);
            return Some(if range.is_raise {
                Action::RaiseTo(to)
            } else {
                Action::BetTo(to)
            });
        }
        if legal.check {
            return Some(Action::Check);
        }
        if let Some(call) = legal.call
            && (strength >= 65
                || u64::from(call) * 4 <= u64::from(view.pot)
                || (strength >= 40 && u64::from(call) * 2 <= u64::from(view.pot))
                || roll < 15)
        {
            return Some(Action::Call);
        }
        Some(Action::Fold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poker::{PokerMatch, Seat};

    #[test]
    fn opponents_play_complete_matches_using_only_legal_actions() {
        for seed in 0..20 {
            let mut game = PokerMatch::new(seed);
            let mut bots = [BasicNpc::new(seed + 100), BasicNpc::new(seed + 200)];
            let mut hands = 0;
            while game.phase() != Phase::MatchComplete && hands < 10000 {
                game.start_next_hand().unwrap();
                hands += 1;
                while let Some(seat) = game.actor() {
                    let observation = game.observe(seat);
                    let action = bots[seat.index()].decide(&observation).unwrap();
                    assert!(observation.legal.accepts(action));
                    game.act(seat, action).unwrap();
                    assert_eq!(game.stacks().iter().sum::<u32>() + game.pot(), 2000);
                }
            }
            assert_eq!(game.phase(), Phase::MatchComplete);
            assert!(bots[0].decide(&game.observe(Seat::Human)).is_none());
        }
    }
}
