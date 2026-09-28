//! Offline evaluation: identical engine/strategy boundary, no Bevy or delays.
use super::{PersonalityStrategy, Strategy, profiles::PROFILES};
use crate::poker::{Action, FourPlayerMatch, Phase, events::GameEvent};

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
