use super::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;

fn game(stacks: [Chips; 2]) -> PokerMatch {
    let mut game = PokerMatch::with_config(
        42,
        Config {
            starting_stacks: stacks,
            ..Config::default()
        },
    )
    .unwrap();
    game.start_next_hand().unwrap();
    game
}
fn act(game: &mut PokerMatch, action: Action) {
    game.act(game.actor().unwrap(), action).unwrap();
}
fn cards(s: &str) -> Vec<Card> {
    s.split_whitespace().map(|s| s.parse().unwrap()).collect()
}
fn assert_conserved(game: &PokerMatch, total: Chips) {
    assert_eq!(game.stacks().iter().sum::<Chips>() + game.pot(), total);
}

#[test]
fn blinds_action_order_and_rotation() {
    let mut g = game([1000; 2]);
    assert_eq!(g.stacks(), [995, 990]);
    assert_eq!(g.pot(), 15);
    assert_eq!(g.actor(), Some(Seat::Human));
    act(&mut g, Action::Call);
    assert_eq!(g.phase(), Phase::PreFlop); // BB still has its option.
    assert_eq!(g.actor(), Some(Seat::Npc));
    assert_eq!(g.legal_actions(Seat::Npc).wager.unwrap().min_to, 20);
    act(&mut g, Action::Check);
    for phase in [Phase::Flop, Phase::Turn, Phase::River] {
        assert_eq!(g.phase(), phase);
        assert_eq!(g.actor(), Some(Seat::Npc));
        act(&mut g, Action::Check);
        assert_eq!(g.actor(), Some(Seat::Human));
        act(&mut g, Action::Check);
    }
    assert_eq!(g.phase(), Phase::HandComplete);
    g.start_next_hand().unwrap();
    assert_eq!(g.dealer, Seat::Npc);
    assert_eq!(g.actor(), Some(Seat::Npc));
    assert_eq!(g.street_bets, [10, 5]);
}

#[test]
fn illegal_actions_are_atomic_and_bet_raise_are_distinct() {
    let mut g = game([1000; 2]);
    let before = format!("{g:?}");
    for (seat, action) in [
        (Seat::Npc, Action::Call),
        (Seat::Human, Action::Check),
        (Seat::Human, Action::BetTo(20)),
        (Seat::Human, Action::RaiseTo(19)),
        (Seat::Human, Action::RaiseTo(1001)),
        (Seat::Human, Action::RaiseTo(u32::MAX)),
    ] {
        assert!(g.act(seat, action).is_err());
        assert_eq!(format!("{g:?}"), before);
    }
    assert!(g.start_next_hand().is_err());
    assert_eq!(format!("{g:?}"), before);
    act(&mut g, Action::Call);
    act(&mut g, Action::Check);
    assert!(g.act(Seat::Npc, Action::Call).is_err());
    assert!(g.act(Seat::Npc, Action::RaiseTo(10)).is_err());
    assert!(g.act(Seat::Npc, Action::BetTo(9)).is_err());
    act(&mut g, Action::BetTo(10));
    assert_eq!(g.legal_actions(Seat::Human).wager.unwrap().min_to, 20);
}

#[test]
fn full_raises_reset_minimum_and_reopen_action() {
    let mut g = game([1000; 2]);
    act(&mut g, Action::RaiseTo(40));
    assert_eq!(g.last_full_raise, 30);
    assert_eq!(g.legal_actions(Seat::Npc).wager.unwrap().min_to, 70);
    act(&mut g, Action::RaiseTo(100));
    assert_eq!(g.legal_actions(Seat::Human).wager.unwrap().min_to, 160);
    act(&mut g, Action::RaiseTo(160));
    act(&mut g, Action::Call);
    assert_eq!(g.phase(), Phase::Flop);
    assert_eq!(g.pot(), 320);
    assert_eq!(g.legal_actions(Seat::Npc).wager.unwrap().min_to, 10);
    act(&mut g, Action::Check);
    act(&mut g, Action::BetTo(10));
    assert!(g.legal_actions(Seat::Npc).wager.is_some());
}

#[test]
fn short_all_in_does_not_reopen_and_cannot_be_a_non_all_in_raise() {
    let mut g = game([1000, 150]);
    act(&mut g, Action::RaiseTo(100));
    assert!(g.legal_actions(Seat::Npc).wager.is_none());
    assert!(g.act(Seat::Npc, Action::RaiseTo(149)).is_err());
    assert!(g.legal_actions(Seat::Npc).all_in);
    act(&mut g, Action::AllIn);
    assert_eq!(g.last_full_raise, 90);
    assert!(!g.raise_right[0]);
    assert!(g.legal_actions(Seat::Human).wager.is_none());
    assert!(!g.legal_actions(Seat::Human).all_in);
    assert_eq!(g.legal_actions(Seat::Human).call, Some(50));
    act(&mut g, Action::Call);
    assert_eq!(g.board.len(), 5);
    assert_conserved(&g, 1150);
}

#[test]
fn short_postflop_opening_all_in_runs_out_after_call() {
    let mut g = game([1000, 15]);
    act(&mut g, Action::Call);
    act(&mut g, Action::Check);
    assert!(g.legal_actions(Seat::Npc).wager.is_none());
    act(&mut g, Action::AllIn);
    assert_eq!(g.legal_actions(Seat::Human).call, Some(5));
    act(&mut g, Action::Call);
    assert_eq!(g.outcome.as_ref().unwrap().pot, 30);
    assert!(g.revealed);
    assert_conserved(&g, 1015);
}

#[test]
fn uncalled_all_in_is_refunded_before_showdown() {
    let mut g = game([1000, 100]);
    act(&mut g, Action::AllIn);
    assert_eq!(g.legal_actions(Seat::Npc).call, Some(90));
    act(&mut g, Action::Call);
    assert!(g.history().contains(&GameEvent::UncalledBetReturned {
        seat: Seat::Human,
        amount: 900
    }));
    assert_eq!(g.outcome.as_ref().unwrap().pot, 200);
    assert_conserved(&g, 1100);
    assert_eq!(g.pot(), 0);
}

#[test]
fn folds_award_only_contested_pot_and_hide_cards() {
    let mut g = game([1000; 2]);
    act(&mut g, Action::RaiseTo(100));
    act(&mut g, Action::Fold);
    assert_eq!(g.stacks(), [1010, 990]);
    assert_eq!(g.outcome.as_ref().unwrap().pot, 20);
    assert_eq!(g.outcome.as_ref().unwrap().awards, [20, 0]);
    assert!(g.history().contains(&GameEvent::UncalledBetReturned {
        seat: Seat::Human,
        amount: 90
    }));
    assert_eq!(g.observe(Seat::Human).revealed_cards, None);
    assert!(g.act(Seat::Human, Action::Check).is_err());
    assert_conserved(&g, 2000);
}

#[test]
fn short_blinds_do_not_force_a_bet_against_an_all_in_player() {
    for stacks in [[3, 1000], [1000, 3], [5, 10], [1, 1]] {
        let g = game(stacks);
        assert!(!g.phase().is_betting());
        assert_eq!(g.board.len(), 5);
        assert_eq!(
            g.outcome.as_ref().unwrap().pot,
            2 * stacks[0].min(stacks[1]).min(10)
        );
        assert_conserved(&g, stacks.iter().sum());
    }
    let mut g = game([1000, 7]);
    assert_eq!(g.actor(), Some(Seat::Human));
    assert_eq!(g.legal_actions(Seat::Human).call, Some(2));
    assert!(g.legal_actions(Seat::Human).wager.is_none());
    act(&mut g, Action::Call);
    assert_eq!(g.outcome.as_ref().unwrap().pot, 14);
}

fn river_fixture(board: &str, human: &str, npc: &str) -> PokerMatch {
    let mut g = game([1000; 2]);
    act(&mut g, Action::Call);
    act(&mut g, Action::Check);
    for _ in 0..4 {
        act(&mut g, Action::Check);
    }
    assert_eq!(g.phase(), Phase::River);
    g.board = cards(board);
    g.holes = Some([
        cards(human).try_into().unwrap(),
        cards(npc).try_into().unwrap(),
    ]);
    g
}

#[test]
fn tied_board_splits_pot_and_kicker_wins_showdown() {
    let mut g = river_fixture("As Ks Qs Js Ts", "2d 3c", "8d 9c");
    act(&mut g, Action::Check);
    act(&mut g, Action::Check);
    assert_eq!(g.stacks(), [1000; 2]);
    assert_eq!(g.outcome.as_ref().unwrap().awards, [10, 10]);
    assert_eq!(g.outcome.as_ref().unwrap().winner, None);
    assert!(g.observe(Seat::Human).revealed_cards.is_some());
    let mut g = river_fixture("As Ad 8h 7c 2d", "Ks 3c", "Qs 4c");
    act(&mut g, Action::Check);
    act(&mut g, Action::Check);
    assert_eq!(g.stacks(), [1010, 990]);
    assert_eq!(g.outcome.as_ref().unwrap().winner, Some(Seat::Human));
}

#[test]
fn dealing_order_burns_and_history_are_reproducible() {
    let mut g = game([1000; 2]);
    let mut deck = Deck::shuffled(42);
    let a = deck.deal().unwrap();
    let b = deck.deal().unwrap();
    let c = deck.deal().unwrap();
    let d = deck.deal().unwrap();
    assert_eq!(g.holes.unwrap(), [[b, d], [a, c]]);
    act(&mut g, Action::Call);
    act(&mut g, Action::Check);
    let burn = deck.deal().unwrap();
    assert!(g.history().contains(&GameEvent::CardBurned { card: burn }));
    assert_eq!(
        g.board,
        (0..3).map(|_| deck.deal().unwrap()).collect::<Vec<_>>()
    );
    for _ in 0..6 {
        act(&mut g, Action::Check);
    }
    let mut all_cards = Vec::new();
    for event in g.history() {
        match event {
            GameEvent::CardsDealt { cards, .. } => all_cards.extend(cards),
            GameEvent::CardBurned { card } => all_cards.push(*card),
            GameEvent::CommunityCardsDealt { cards, .. } => all_cards.extend(cards),
            _ => (),
        }
    }
    assert_eq!(all_cards.len(), 12);
    assert_eq!(all_cards.iter().collect::<HashSet<_>>().len(), 12);
    let mut replay = PokerMatch::new(42);
    for event in g.history() {
        match event {
            GameEvent::HandStarted { .. } => replay.start_next_hand().unwrap(),
            GameEvent::PlayerActed { seat, action, .. } => replay.act(*seat, *action).unwrap(),
            _ => (),
        }
    }
    assert_eq!(g.history(), replay.history());
}

#[test]
fn observation_cannot_leak_opponent_cards_or_seed() {
    let mut g = game([1000; 2]);
    let before = g.observe(Seat::Npc);
    assert_eq!(before.hole_cards, Some(g.holes.unwrap()[1]));
    assert!(before.revealed_cards.is_none());
    g.holes.as_mut().unwrap()[0] = cards("As Ah").try_into().unwrap();
    g.seed = 9999;
    assert_eq!(before, g.observe(Seat::Npc));
}

#[test]
fn seeded_random_matches_conserve_chips_and_terminate_hands() {
    let mut rng = ChaCha8Rng::seed_from_u64(888);
    for seed in 0..100 {
        let mut g = PokerMatch::new(seed);
        for _ in 0..100 {
            if g.phase() == Phase::MatchComplete {
                break;
            }
            g.start_next_hand().unwrap();
            for turn in 0..1000 {
                assert_conserved(&g, 2000);
                let Some(seat) = g.actor() else {
                    break;
                };
                assert!(turn < 999, "hand did not terminate");
                let legal = g.legal_actions(seat);
                let mut options = vec![Action::Fold];
                if legal.check {
                    options.push(Action::Check);
                }
                if legal.call.is_some() {
                    options.push(Action::Call);
                }
                if legal.all_in {
                    options.push(Action::AllIn);
                }
                if let Some(r) = legal.wager {
                    let to = rng.random_range(r.min_to..=r.max_to);
                    options.push(if r.is_raise {
                        Action::RaiseTo(to)
                    } else {
                        Action::BetTo(to)
                    });
                }
                g.act(seat, options[rng.random_range(0..options.len())])
                    .unwrap();
            }
            assert!(!g.phase().is_betting());
            assert_conserved(&g, 2000);
        }
    }
}

#[test]
fn match_completion_blocks_next_hand_and_new_match_resets() {
    let mut g = PokerMatch::new(77);
    for _ in 0..100 {
        if g.phase() == Phase::MatchComplete {
            break;
        }
        g.start_next_hand().unwrap();
        act(&mut g, Action::AllIn);
        act(&mut g, Action::Call);
    }
    assert_eq!(g.phase(), Phase::MatchComplete);
    assert!(g.start_next_hand().is_err());
    assert!(g.stacks().contains(&2000));
    assert_eq!(PokerMatch::new(77).stacks(), [1000; 2]);
}
