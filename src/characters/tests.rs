use super::*;
use crate::npc::profiles::{PROFILES, profile};

fn state() -> PresentationState {
    let mut state = PresentationState::default();
    state.reset(7);
    state.apply(&PresentationEvent::HandStarted {
        number: 1,
        stacks: vec![1000; 4],
    });
    state
}

#[test]
fn identity_migration_keeps_seats_ids_and_every_strategy_parameter() {
    let expected = [
        (
            Seat::Npc,
            "mira",
            "Ananya",
            [0.68, 0.74, 0.035, 0.30, 60.0, 0.65, 0.90],
        ),
        (
            Seat::Jax,
            "jax",
            "Freya",
            [0.93, 0.20, 0.16, 0.80, 85.0, 0.80, 0.50],
        ),
        (
            Seat::Nova,
            "nova",
            "Yuna",
            [0.18, 0.82, 0.01, 0.18, 45.0, 0.20, 0.65],
        ),
    ];
    for (seat, id, name, values) in expected {
        let p = profile(seat);
        assert_eq!((p.id, p.name), (id, name));
        assert_eq!(p, PROFILES[seat.index()]);
        let s = p.personality;
        assert_eq!(
            [
                s.aggression,
                s.selectivity,
                s.bluff_frequency,
                s.risk_tolerance,
                f64::from(s.bet_size_percent),
                s.continuation,
                s.positional_awareness
            ],
            values
        );
        let c = definition(seat).unwrap();
        assert!(c.portrait_path.contains(&name.to_lowercase()));
        assert_eq!(c.seat, seat);
        assert!(c.age >= 24);
    }
    assert!(definition(Seat::Human).is_none());
}

#[test]
fn public_action_reacts_then_returns_to_current_turn_baseline() {
    let mut s = state();
    s.set_actor(Some(Seat::Jax));
    assert_eq!(s.seats[2].expression, CharacterExpression::Thinking);
    s.apply(&PresentationEvent::Acted {
        seat: Seat::Jax,
        action: Action::RaiseTo(80),
    });
    s.set_actor(Some(Seat::Nova));
    assert_eq!(s.seats[2].expression, CharacterExpression::Confident);
    assert_eq!(s.seats[2].action_text, "Raise to 80");
    assert_eq!(s.seats[3].expression, CharacterExpression::Thinking);
    s.tick(5.0);
    assert_eq!(s.seats[2].expression, CharacterExpression::Neutral);
    assert!(s.dialogue.is_none());
    s.apply(&PresentationEvent::Acted {
        seat: Seat::Nova,
        action: Action::Fold,
    });
    assert!(s.seats[3].folded);
}

#[test]
fn showdown_outcomes_and_elimination_override_short_reactions() {
    let mut s = state();
    s.apply(&PresentationEvent::Settled {
        pot: 500,
        awards: vec![0, 500, 0, 0],
        shown: vec![false, true, true, false],
        stacks: vec![1000, 1500, 0, 1500],
    });
    assert_eq!(s.seats[1].expression, CharacterExpression::Happy);
    assert_eq!(s.seats[2].expression, CharacterExpression::Eliminated);
    assert_eq!(s.seats[3].expression, CharacterExpression::Neutral); // folded loser reveals no reaction
    s.tick(20.0);
    s.set_actor(None);
    assert_eq!(s.seats[2].expression, CharacterExpression::Eliminated);
    s.apply(&PresentationEvent::HandStarted {
        number: 2,
        stacks: vec![1000, 1500, 0, 1500],
    });
    assert!(s.seats[2].eliminated);
    s.apply(&PresentationEvent::Settled {
        pot: 100,
        awards: vec![0, 50, 0, 50],
        shown: vec![false, true, false, true],
        stacks: vec![1000, 1500, 0, 1500],
    });
    assert_eq!(s.seats[1].expression, CharacterExpression::Surprised);
    assert_eq!(s.seats[3].expression, CharacterExpression::Surprised);
    s.apply(&PresentationEvent::Settled {
        pot: 100,
        awards: vec![100, 0, 0, 0],
        shown: vec![true, true, false, false],
        stacks: vec![1500, 1000, 0, 1500],
    });
    assert_eq!(s.seats[1].expression, CharacterExpression::Disappointed);
    s.tick(5.0);
    assert_eq!(s.seats[1].expression, CharacterExpression::Neutral);
}

#[test]
fn dialogue_speaker_and_epoch_guard_survive_restart_and_next_hand() {
    let mut s = state();
    let line = DialogueLine {
        session: 7,
        hand: 1,
        speaker: Seat::Nova,
        text: "Well played.".into(),
        expression: Some(CharacterExpression::Happy),
        duration: 2.0,
    };
    assert!(s.say(line.clone()));
    assert_eq!(profile(s.dialogue.as_ref().unwrap().speaker).name, "Yuna");
    s.tick(3.0);
    assert!(s.dialogue.is_none());
    assert_eq!(s.seats[3].expression, CharacterExpression::Neutral);
    s.apply(&PresentationEvent::HandStarted {
        number: 2,
        stacks: vec![1000; 4],
    });
    assert!(!s.say(line.clone()));
    s.reset(8);
    assert!(!s.say(line));
    assert_eq!(
        s,
        PresentationState {
            session: 8,
            ..Default::default()
        }
    );
}
