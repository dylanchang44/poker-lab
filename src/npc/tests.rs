use super::*;
use crate::poker::{Action, FourPlayerMatch, Seat, cards::Card};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn view() -> crate::poker::Observation {
    let mut game = FourPlayerMatch::new(7);
    game.start_next_hand().unwrap();
    game.observe(Seat::Nova)
}
fn cards(s: &str) -> Vec<Card> {
    s.split_whitespace().map(|c| c.parse().unwrap()).collect()
}

#[test]
fn equity_is_reproducible_and_accounts_for_ties_and_visible_cards() {
    let mut v = view();
    v.board = cards("As Ks Qs Js Ts");
    v.hole_cards = Some(cards("2d 3c").try_into().unwrap());
    assert_eq!(
        equity::estimate(&v, 64, &mut ChaCha8Rng::seed_from_u64(42)),
        0.25
    );
    v.board.clear();
    let a = equity::estimate(&v, 128, &mut ChaCha8Rng::seed_from_u64(42));
    assert_eq!(
        a,
        equity::estimate(&v, 128, &mut ChaCha8Rng::seed_from_u64(42))
    );
    v.hole_cards = Some(cards("As Ah").try_into().unwrap());
    assert!(equity::estimate(&v, 256, &mut ChaCha8Rng::seed_from_u64(42)) > a + 0.2);
}

#[test]
fn draws_and_starting_strength_have_sensible_order() {
    let d = equity::draws(cards("Ah Kh").try_into().unwrap(), &cards("Qh Jh 2c"));
    assert!(d.flush && d.straight);
    let d = equity::draws(cards("2c 7d").try_into().unwrap(), &cards("Ah Ks 9s"));
    assert!(!d.flush && !d.straight);
    assert!(
        equity::starting_strength(cards("As Ah").try_into().unwrap())
            > equity::starting_strength(cards("As Ks").try_into().unwrap())
    );
    assert!(
        equity::starting_strength(cards("8s 9s").try_into().unwrap())
            > equity::starting_strength(cards("7d 2s").try_into().unwrap())
    );
}

#[test]
fn same_information_different_profiles_have_distinct_coherent_tendencies() {
    let mut counts = [(0, 0); 3];
    for seed in 0..180 {
        let mut g = FourPlayerMatch::new(seed);
        g.start_next_hand().unwrap();
        let v = g.observe(Seat::Nova);
        for (i, profile) in [profiles::MIRA, profiles::JAX, profiles::NOVA]
            .iter()
            .enumerate()
        {
            let a = PersonalityStrategy::new(profile.personality, seed + 333, 32)
                .decide(&v)
                .unwrap();
            assert!(v.legal.accepts(a));
            if a != Action::Fold {
                counts[i].0 += 1;
            }
            if matches!(a, Action::RaiseTo(_) | Action::AllIn) {
                counts[i].1 += 1;
            }
        }
    }
    assert!(counts[1].0 > counts[0].0 + 15, "{counts:?}");
    assert!(
        counts[1].1 > counts[0].1 && counts[0].1 > counts[2].1,
        "{counts:?}"
    );
    let mut v = view();
    v.hole_cards = Some(cards("7d 2s").try_into().unwrap());
    for p in profiles::PROFILES {
        assert_eq!(
            PersonalityStrategy::new(p.personality, 42, 32).decide(&v),
            Some(Action::Fold)
        );
    }
}

#[test]
fn seeded_batches_repeat_and_four_player_personality_matches_finish() {
    let a = simulation::simulate(2, 42, 16, 2000);
    let b = simulation::simulate(2, 42, 16, 2000);
    assert_eq!(a, b);
    assert_eq!(a.completed, 2);
    assert_eq!(a.truncated, 0);
    assert_eq!(a.rows.iter().map(|r| r.net_chips).sum::<i64>(), 0);
    assert_eq!(a.rows.iter().map(|r| r.matches_won).sum::<u64>(), 2);
}
