use super::*;
use crate::poker::{
    Config, FourPlayerMatch, PokerMatch,
    cards::{Rank, Suit},
};

fn project(events: &[GameEvent]) -> ReviewHistory {
    let mut result = ReviewHistory::default();
    for event in events {
        result.observe(event);
    }
    result
}
fn conserved(history: &ReviewHistory, total: Chips) {
    for hand in history.hands() {
        for frame in &hand.frames {
            assert_eq!(frame.stacks.iter().sum::<u32>() + frame.pot, total);
        }
    }
}
fn finish(game: &mut FourPlayerMatch) {
    while let Some(seat) = game.actor() {
        let legal = game.legal_actions(seat);
        game.act(
            seat,
            if legal.check {
                Action::Check
            } else {
                Action::Call
            },
        )
        .unwrap();
    }
}

#[test]
fn complete_only_initial_blinds_actions_and_board_timing() {
    let mut game = FourPlayerMatch::new(42);
    game.start_next_hand().unwrap();
    assert!(project(game.history()).hands().is_empty());
    finish(&mut game);
    let before = game.history().to_vec();
    let history = project(&before);
    let frames = &history.hands()[0].frames;
    assert_eq!(frames[0].stacks, vec![1000; 4]);
    assert_eq!(frames[0].pot, 0);
    assert_eq!(frames[1].pot, 5);
    assert_eq!(frames[2].pot, 15);
    let actions: Vec<_> = frames
        .iter()
        .filter_map(|f| match f.moment {
            Moment::Action { seat, action, paid } => Some((seat, action, paid)),
            _ => None,
        })
        .collect();
    let source: Vec<_> = before
        .iter()
        .filter_map(|e| match e {
            GameEvent::PlayerActed {
                seat, action, paid, ..
            } => Some((*seat, *action, *paid)),
            _ => None,
        })
        .collect();
    assert_eq!(actions, source);
    let mut board = 0;
    for frame in frames {
        if frame.moment == Moment::Board {
            board = if board == 0 { 3 } else { board + 1 };
        }
        assert_eq!(frame.board.len(), board);
    }
    assert_eq!(board, 5);
    conserved(&history, 4000);
    let mut selection = Selection::latest(&history);
    selection.move_step(&history, false);
    assert_eq!(selection.step, 0);
    for _ in 0..100 {
        selection.move_step(&history, true);
        selection.move_hand(&history, true);
    }
    assert_eq!(selection.hand, 0);
    for _ in 0..100 {
        selection.move_step(&history, true);
    }
    assert_eq!(selection.frame(&history).unwrap().moment, Moment::Complete);
    assert_eq!(game.history(), before); // projection/navigation have no mutable engine handle
}

#[test]
fn refunds_awards_side_pots_and_tied_shares_follow_authoritative_events() {
    let mut saw_split = false;
    for seed in 0..300 {
        let mut game = FourPlayerMatch::with_config(
            seed,
            Config {
                starting_stacks: [50, 100, 200, 300],
                ..Default::default()
            },
        )
        .unwrap();
        game.start_next_hand().unwrap();
        while let Some(seat) = game.actor() {
            game.act(seat, Action::AllIn).unwrap();
        }
        let history = project(game.history());
        conserved(&history, 650);
        let hand = &history.hands()[0];
        let revealed = hand
            .frames
            .iter()
            .find(|f| f.moment == Moment::Reveal)
            .unwrap();
        assert!(
            revealed.board.is_empty(),
            "all-in hands reveal before the runout"
        );
        let last = hand.frames.last().unwrap();
        assert_eq!(last.stacks, game.stacks());
        assert_eq!(last.outcome, game.observe(Seat::Human).outcome);
        assert!(hand.frames.iter().any(|f| f.moment
            == Moment::Refund {
                seat: Seat::Nova,
                amount: 100
            }));
        assert_eq!(last.outcome.as_ref().unwrap().pots.len(), 3);
        for pot in &last.outcome.as_ref().unwrap().pots {
            assert_eq!(pot.awards.iter().sum::<u32>(), pot.pot.amount);
            saw_split |= pot.winners.len() > 1;
        }
        if saw_split {
            break;
        }
    }
    assert!(saw_split, "fixture must exercise a real split pot");
}

#[test]
fn opponent_deals_burns_and_seeds_cannot_enter_replay() {
    let mut game = FourPlayerMatch::new(42);
    game.start_next_hand().unwrap();
    let folded = game.actor().unwrap(); // Yuna folds; others go to showdown
    assert_ne!(folded, Seat::Human);
    game.act(folded, Action::Fold).unwrap();
    finish(&mut game);
    let original = project(game.history());
    let secret = Card {
        rank: Rank::Ace,
        suit: Suit::Spades,
    };
    let changed: Vec<_> = game
        .history()
        .iter()
        .cloned()
        .map(|mut e| {
            match &mut e {
                GameEvent::HandStarted { seed, .. } => *seed = u64::MAX,
                GameEvent::CardsDealt { seat, cards } if *seat != Seat::Human => {
                    *cards = [secret; 2]
                }
                GameEvent::CardBurned { card } => *card = secret,
                _ => {}
            }
            e
        })
        .collect();
    assert_eq!(original.hands(), project(&changed).hands());
    let mut revealed = false;
    for frame in &original.hands()[0].frames {
        revealed |= frame.moment == Moment::Reveal;
        assert_eq!(frame.shown[folded.index()], None);
        if !revealed {
            assert!(frame.shown.iter().all(Option::is_none));
        }
    }
    assert!(revealed);
    let mut fold_game = PokerMatch::new(2);
    fold_game.start_next_hand().unwrap();
    fold_game.act(Seat::Human, Action::Fold).unwrap();
    assert!(
        project(fold_game.history()).hands()[0]
            .frames
            .iter()
            .all(|f| f.shown.iter().all(Option::is_none))
    );
}

#[test]
fn odd_chip_awards_are_copied_without_resplitting() {
    for seed in 0..2000 {
        let mut game = crate::poker::Table::<3>::with_config(
            seed,
            Config {
                starting_stacks: [20; 3],
                small_blind: 1,
                big_blind: 1,
            },
        )
        .unwrap();
        game.start_next_hand().unwrap();
        game.act(Seat::Human, Action::Call).unwrap();
        game.act(Seat::Npc, Action::Fold).unwrap();
        while let Some(seat) = game.actor() {
            game.act(seat, Action::Check).unwrap();
        }
        let outcome = game.observe(Seat::Human).outcome.unwrap();
        if outcome.pots[0].winners.len() == 2 {
            assert_eq!(outcome.pot, 3);
            let replay = project(game.history());
            conserved(&replay, 60);
            assert_eq!(
                replay.hands()[0].frames.last().unwrap().outcome,
                Some(outcome.clone())
            );
            let mut awards = outcome.awards;
            awards.sort();
            assert_eq!(awards, vec![0, 1, 2]);
            return;
        }
    }
    panic!("seed fixtures must produce a tied three-chip pot");
}

#[test]
fn history_is_bounded_and_duplicate_completion_does_not_add_a_hand() {
    let mut history = ReviewHistory::default();
    for seed in 0..25 {
        let mut game = PokerMatch::new(seed);
        game.start_next_hand().unwrap();
        game.act(Seat::Human, Action::Fold).unwrap();
        for e in game.history() {
            history.observe(e);
        }
        history.observe(game.history().last().unwrap());
    }
    assert_eq!(history.hands().len(), RECENT_HANDS);
    let mut selection = Selection::latest(&history);
    assert_eq!(selection.hand, 19);
    for _ in 0..30 {
        selection.move_hand(&history, false);
    }
    assert_eq!(selection.hand, 0);
    let empty = ReviewHistory::default();
    selection.move_step(&empty, true);
    selection.move_hand(&empty, false);
    assert!(selection.frame(&empty).is_none());
}
