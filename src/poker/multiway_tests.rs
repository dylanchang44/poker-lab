use super::*;

fn act(g: &mut FourPlayerMatch, a: Action) {
    g.act(g.actor().unwrap(), a).unwrap();
}
fn game(stacks: [Chips; 4]) -> FourPlayerMatch {
    let mut g = FourPlayerMatch::with_config(
        42,
        Config {
            starting_stacks: stacks,
            ..Config::default()
        },
    )
    .unwrap();
    g.start_next_hand().unwrap();
    g
}
fn check_down(g: &mut FourPlayerMatch) {
    for _ in 0..100 {
        let Some(seat) = g.actor() else { return };
        let action = if g.legal_actions(seat).check {
            Action::Check
        } else {
            Action::Call
        };
        g.act(seat, action).unwrap();
    }
    panic!("hand stuck");
}
fn cards(s: &str) -> Vec<Card> {
    s.split_whitespace().map(|c| c.parse().unwrap()).collect()
}

#[test]
fn folds_skip_turns_but_do_not_end_a_multiway_hand() {
    let mut g = game([1000; 4]);
    act(&mut g, Action::Fold);
    assert_eq!(g.actor(), Some(Seat::Human));
    assert_eq!(g.phase(), Phase::PreFlop);
    act(&mut g, Action::Call);
    act(&mut g, Action::Call);
    act(&mut g, Action::Check);
    act(&mut g, Action::Check);
    act(&mut g, Action::Check);
    assert_eq!(g.actor(), Some(Seat::Human)); // skips folded Nova
    check_down(&mut g);
    let view = g.observe(Seat::Human);
    assert!(view.revealed_cards.unwrap()[3].is_none());
    assert_eq!(g.stacks().iter().sum::<u32>(), 4000);
}

#[test]
fn rotation_dead_blind_and_heads_up_skip_busted_seats() {
    let mut g = game([1000; 4]);
    check_down(&mut g);
    g.start_next_hand().unwrap();
    assert_eq!(
        (g.dealer, g.small_blind, g.big_blind),
        (Seat::Npc, Some(Seat::Jax), Seat::Nova)
    );
    // Isolate blind movement using a completed-hand fixture (all chips conserved).
    let mut g = game([1000; 4]);
    check_down(&mut g);
    g.stacks[0] += g.stacks[2];
    g.stacks[2] = 0;
    g.start_next_hand().unwrap();
    assert_eq!(
        (g.dealer, g.small_blind, g.big_blind),
        (Seat::Npc, None, Seat::Nova)
    );
    assert!(!g.dealt[2]);
    assert_eq!(g.actor(), Some(Seat::Human));
    check_down(&mut g);
    g.start_next_hand().unwrap();
    assert_eq!(
        (g.dealer, g.small_blind, g.big_blind),
        (Seat::Jax, Some(Seat::Nova), Seat::Human)
    );
    check_down(&mut g);
    g.stacks[3] += g.stacks[1];
    g.stacks[1] = 0;
    g.start_next_hand().unwrap();
    assert_eq!(g.big_blind, Seat::Nova); // previous BB human cannot post again
    assert_eq!(g.dealer, Seat::Human);
    assert_eq!(g.small_blind, Some(Seat::Human));
    assert_eq!(g.actor(), Some(Seat::Human));
    act(&mut g, Action::Call);
    act(&mut g, Action::Check);
    assert_eq!(g.actor(), Some(Seat::Nova));
}

#[test]
fn short_blind_keeps_full_bring_in_while_players_can_bet() {
    let mut g = game([1000, 1000, 3, 1000]);
    assert_eq!(g.legal_actions(Seat::Nova).call, Some(10));
    check_down(&mut g);
    assert_eq!(g.outcome.as_ref().unwrap().pot, 33);
}

#[test]
fn cumulative_short_raises_reopen_only_players_facing_a_full_raise() {
    let mut g = game([140, 1000, 1000, 1000]);
    act(&mut g, Action::RaiseTo(100)); // Nova, last full increment 90
    act(&mut g, Action::AllIn); // Human 140, short +40
    assert!(g.legal_actions(Seat::Npc).wager.is_some()); // has not yet acted
    act(&mut g, Action::Call);
    act(&mut g, Action::Call);
    assert_eq!(g.actor(), Some(Seat::Nova));
    assert!(g.legal_actions(Seat::Nova).wager.is_none());
    act(&mut g, Action::Call);
    assert_eq!(g.phase(), Phase::Flop); // three can still bet

    let mut g = game([140, 190, 1000, 1000]);
    act(&mut g, Action::RaiseTo(100));
    act(&mut g, Action::AllIn);
    act(&mut g, Action::AllIn);
    act(&mut g, Action::Call);
    assert_eq!(g.actor(), Some(Seat::Nova));
    assert_eq!(g.legal_actions(Seat::Nova).wager.unwrap().min_to, 280);
    act(&mut g, Action::RaiseTo(280));
    act(&mut g, Action::Call);
    assert_eq!(g.phase(), Phase::Flop);
    check_down(&mut g);
    assert_eq!(g.stacks().iter().sum::<u32>(), 2330);
}

#[test]
fn multiple_all_ins_refund_only_unmatched_top_and_make_side_pots() {
    let mut g = game([100, 200, 300, 1000]);
    act(&mut g, Action::AllIn);
    act(&mut g, Action::Call);
    act(&mut g, Action::Call);
    act(&mut g, Action::Call);
    let result = g.outcome.as_ref().unwrap();
    assert_eq!(result.pot, 900);
    assert_eq!(
        result.pots.iter().map(|p| p.pot.amount).collect::<Vec<_>>(),
        [400, 300, 200]
    );
    assert_eq!(
        result.pots[1].pot.eligible,
        [Seat::Npc, Seat::Jax, Seat::Nova]
    );
    assert_eq!(result.pots[2].pot.eligible, [Seat::Jax, Seat::Nova]);
    assert!(g.history().contains(&GameEvent::UncalledBetReturned {
        seat: Seat::Nova,
        amount: 700
    }));
    assert_eq!(g.stacks().iter().sum::<u32>(), 1600);
}

#[test]
fn different_winners_receive_main_and_side_pots() {
    let mut g = game([100, 200, 300, 1000]);
    // Rig only the settlement fixture, not any production shuffle path.
    g.board = cards("2s 3h 7d 8c 9s");
    g.holes = Some([
        cards("As Ah").try_into().unwrap(),
        cards("Ks Kh").try_into().unwrap(),
        cards("Qs Qh").try_into().unwrap(),
        cards("Js Jh").try_into().unwrap(),
    ]);
    act(&mut g, Action::AllIn);
    act(&mut g, Action::Call);
    act(&mut g, Action::Call);
    act(&mut g, Action::Call);
    assert_eq!(g.outcome.as_ref().unwrap().awards, [400, 300, 200, 0]);
    assert_eq!(g.stacks(), [400, 300, 200, 700]);
}

#[test]
fn folded_contributions_and_odd_chips_are_distributed_per_pot() {
    let mut g = game([3, 5, 5, 2]);
    g.phase = Phase::River;
    g.stacks = [0; 4];
    g.contributions = [3, 5, 5, 2];
    g.street_bets = g.contributions;
    g.folded = [false, false, false, true];
    g.dealt = [true; 4];
    g.board = cards("As Ks Qs Js Ts");
    g.holes = Some([
        cards("2d 3c").try_into().unwrap(),
        cards("4d 5c").try_into().unwrap(),
        cards("6d 7c").try_into().unwrap(),
        cards("8d 9c").try_into().unwrap(),
    ]);
    g.showdown();
    let result = g.outcome.as_ref().unwrap();
    assert_eq!(
        result.pots.iter().map(|p| p.pot.amount).collect::<Vec<_>>(),
        [11, 4]
    );
    assert_eq!(result.awards, [3, 6, 6, 0]);
    assert_eq!(g.stacks().iter().sum::<u32>(), 15);
    assert_eq!(
        pots::split(5, &[Seat::Human, Seat::Nova], Seat::Jax, 4),
        [2, 0, 0, 3]
    );
}

#[test]
fn illegal_actions_and_hidden_hands_cannot_cross_observation_boundary() {
    let mut g = game([1000; 4]);
    let before = format!("{g:?}");
    assert!(g.act(Seat::Npc, Action::AllIn).is_err());
    assert!(g.act(Seat::Nova, Action::RaiseTo(9999)).is_err());
    assert_eq!(format!("{g:?}"), before);
    let view = g.observe(Seat::Npc);
    for i in [0, 2, 3] {
        g.holes.as_mut().unwrap()[i] = cards("As Ah").try_into().unwrap();
    }
    g.deck = Deck::shuffled(999);
    g.seed = 999;
    assert_eq!(view, g.observe(Seat::Npc));
    assert!(view.revealed_cards.is_none());
}

#[test]
fn seeded_random_four_player_matches_finish_and_replay() {
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    for seed in 0..100 {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut g = FourPlayerMatch::new(seed);
        for hand in 0..2000 {
            if g.phase() == Phase::MatchComplete {
                break;
            }
            assert!(hand < 1999);
            g.start_next_hand().unwrap();
            for turn in 0..1000 {
                let Some(seat) = g.actor() else {
                    break;
                };
                assert!(turn < 999);
                let l = g.legal_actions(seat);
                let mut options = vec![Action::Fold];
                if l.check {
                    options.push(Action::Check);
                }
                if l.call.is_some() {
                    options.push(Action::Call);
                }
                if l.all_in {
                    options.push(Action::AllIn);
                }
                if let Some(r) = l.wager {
                    let to = rng.random_range(r.min_to..=r.max_to);
                    options.push(if r.is_raise {
                        Action::RaiseTo(to)
                    } else {
                        Action::BetTo(to)
                    });
                }
                g.act(seat, options[rng.random_range(0..options.len())])
                    .unwrap();
                assert_eq!(g.stacks().iter().sum::<u32>() + g.pot(), 4000);
            }
        }
        assert_eq!(g.phase(), Phase::MatchComplete);
        assert!(g.stacks().contains(&4000));
        let mut replay = FourPlayerMatch::new(seed);
        for event in g.history() {
            match event {
                GameEvent::HandStarted { .. } => replay.start_next_hand().unwrap(),
                GameEvent::PlayerActed { seat, action, .. } => replay.act(*seat, *action).unwrap(),
                _ => (),
            }
        }
        assert_eq!(g.history(), replay.history());
    }
}

#[test]
fn four_seat_deal_and_order() {
    let mut g = FourPlayerMatch::new(42);
    g.start_next_hand().unwrap();
    assert_eq!(g.stacks(), [1000, 995, 990, 1000]);
    assert_eq!(g.actor(), Some(Seat::Nova));
    assert_eq!(
        g.holes
            .unwrap()
            .into_iter()
            .flatten()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    for seat in [Seat::Nova, Seat::Human, Seat::Npc] {
        g.act(seat, Action::Call).unwrap();
    }
    assert_eq!(g.phase(), Phase::PreFlop);
    g.act(Seat::Jax, Action::Check).unwrap();
    assert_eq!(g.actor(), Some(Seat::Npc));
    assert_eq!(g.phase(), Phase::Flop);
}
