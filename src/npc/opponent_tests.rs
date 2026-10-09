use super::{
    PersonalityStrategy, Strategy,
    adaptation::Adaptation,
    opponent::*,
    profiles::{NpcId, profile},
};
use crate::poker::{Action, Outcome, Phase, Seat, events::GameEvent};

fn start(o: &mut Observer) {
    o.observe(&GameEvent::HandStarted {
        number: 1,
        seed: 999,
        dealer: Seat::Human,
        stacks: vec![1000; 4],
        blinds: [5, 10],
    });
}
#[allow(clippy::too_many_arguments)]
fn act(
    o: &mut Observer,
    seat: Seat,
    phase: Phase,
    action: Action,
    call: u32,
    raise: bool,
    paid: u32,
    total: u32,
    pot: u32,
) {
    o.observe(&GameEvent::DecisionOffered {
        seat,
        phase,
        pot,
        to_call: call,
        street_bet: total - paid,
        can_raise: raise,
    });
    o.observe(&GameEvent::PlayerActed {
        seat,
        phase,
        action,
        paid,
        street_total: total,
        pot: pot + paid,
        stacks: vec![900; 4],
    });
}
fn street(o: &mut Observer, phase: Phase) {
    o.observe(&GameEvent::CommunityCardsDealt {
        phase,
        cards: vec![],
    });
}
fn finish(o: &mut Observer, won: bool) -> HandSample {
    o.observe(&GameEvent::HandCompleted {
        outcome: Outcome {
            winner: None,
            pot: 100,
            awards: vec![if won { 50 } else { 0 }, 50, 0, 0],
            hands: None,
            folded: None,
            pots: vec![],
        },
        stacks: vec![1000; 4],
    })
    .unwrap()
    .1
}
fn rich_model(m: Metric, yes: u64, n: u64) -> OpponentModel {
    let mut model = OpponentModel::default();
    for h in 0..n {
        let mut s = HandSample::default();
        s.counts[m as usize] = Count {
            yes: u64::from(h < yes),
            opportunities: 1,
        };
        model.record(s);
    }
    model
}
#[test]
fn vpip_pfr_limp_are_once_per_decision_hand_and_exclude_blinds() {
    let mut o = Observer::default();
    start(&mut o);
    o.observe(&GameEvent::BlindPosted {
        seat: Seat::Human,
        amount: 10,
    });
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::Check,
        0,
        true,
        0,
        10,
        15,
    );
    let s = finish(&mut o, false);
    assert_eq!(
        s.get(Metric::Vpip),
        Count {
            yes: 0,
            opportunities: 1
        }
    );
    assert_eq!(s.get(Metric::Pfr).yes, 0);
    start(&mut o);
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::Call,
        10,
        true,
        10,
        10,
        15,
    );
    act(
        &mut o,
        Seat::Npc,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        25,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(70),
        20,
        true,
        60,
        70,
        55,
    );
    let s = finish(&mut o, false);
    for m in [Metric::Vpip, Metric::Pfr, Metric::Limp] {
        assert_eq!(
            s.get(m),
            Count {
                yes: 1,
                opportunities: 1
            }
        );
    }
    start(&mut o);
    let s = finish(&mut o, false);
    assert_eq!(s.get(Metric::Vpip).opportunities, 0); // blind all-in: no voluntary decision
}
#[test]
fn three_bets_count_legal_opportunities_and_short_all_in_calls_are_not_raises() {
    let mut o = Observer::default();
    start(&mut o);
    act(
        &mut o,
        Seat::Npc,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        15,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::AllIn,
        30,
        false,
        20,
        20,
        45,
    );
    let s = finish(&mut o, false);
    assert_eq!(s.get(Metric::ThreeBet).opportunities, 0);
    assert_eq!(s.get(Metric::Pfr).yes, 0);
    start(&mut o);
    act(
        &mut o,
        Seat::Npc,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        15,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(80),
        30,
        true,
        80,
        80,
        45,
    );
    let s = finish(&mut o, false);
    assert_eq!(
        s.get(Metric::ThreeBet),
        Count {
            yes: 1,
            opportunities: 1
        }
    );
    start(&mut o);
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        15,
    );
    act(
        &mut o,
        Seat::Npc,
        Phase::PreFlop,
        Action::RaiseTo(80),
        30,
        true,
        80,
        80,
        45,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::Fold,
        50,
        true,
        0,
        30,
        125,
    );
    let s = finish(&mut o, false);
    assert_eq!(
        s.get(Metric::FoldThreeBet),
        Count {
            yes: 1,
            opportunities: 1
        }
    );
    assert_eq!(s.get(Metric::ThreeBet).opportunities, 0);
}
#[test]
fn cbet_donk_and_intervening_raise_opportunities() {
    let mut o = Observer::default();
    start(&mut o);
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        15,
    );
    street(&mut o, Phase::Flop);
    act(
        &mut o,
        Seat::Npc,
        Phase::Flop,
        Action::Check,
        0,
        true,
        0,
        0,
        90,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::Flop,
        Action::BetTo(50),
        0,
        true,
        50,
        50,
        90,
    );
    assert_eq!(
        finish(&mut o, false).get(Metric::Cbet),
        Count {
            yes: 1,
            opportunities: 1
        }
    );
    start(&mut o);
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        15,
    );
    street(&mut o, Phase::Flop);
    act(
        &mut o,
        Seat::Npc,
        Phase::Flop,
        Action::BetTo(30),
        0,
        true,
        30,
        30,
        90,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::Flop,
        Action::Call,
        30,
        true,
        30,
        30,
        120,
    );
    assert_eq!(finish(&mut o, false).get(Metric::Cbet).opportunities, 0);
    for intervenes in [false, true] {
        start(&mut o);
        act(
            &mut o,
            Seat::Npc,
            Phase::PreFlop,
            Action::RaiseTo(30),
            10,
            true,
            30,
            30,
            15,
        );
        street(&mut o, Phase::Flop);
        act(
            &mut o,
            Seat::Npc,
            Phase::Flop,
            Action::BetTo(50),
            0,
            true,
            50,
            50,
            90,
        );
        if intervenes {
            act(
                &mut o,
                Seat::Jax,
                Phase::Flop,
                Action::RaiseTo(150),
                50,
                true,
                150,
                150,
                140,
            );
        }
        act(
            &mut o,
            Seat::Human,
            Phase::Flop,
            Action::Fold,
            if intervenes { 150 } else { 50 },
            true,
            0,
            0,
            140,
        );
        assert_eq!(
            finish(&mut o, false).get(Metric::FoldCbet).opportunities,
            u64::from(!intervenes)
        );
    }
}
#[test]
fn river_sizes_showdown_and_eligibility() {
    let mut o = Observer::default();
    start(&mut o);
    street(&mut o, Phase::Flop);
    street(&mut o, Phase::River);
    act(
        &mut o,
        Seat::Npc,
        Phase::River,
        Action::BetTo(75),
        0,
        true,
        75,
        75,
        100,
    );
    act(
        &mut o,
        Seat::Human,
        Phase::River,
        Action::Fold,
        75,
        true,
        0,
        0,
        175,
    );
    let s = finish(&mut o, false);
    assert_eq!(
        s.get(Metric::FoldLargeRiver),
        Count {
            yes: 1,
            opportunities: 1
        }
    );
    assert_eq!(
        s.get(Metric::RiverAggression),
        Count {
            yes: 0,
            opportunities: 1
        }
    );
    assert_eq!(
        s.get(Metric::Showdown),
        Count {
            yes: 0,
            opportunities: 1
        }
    );
    start(&mut o);
    street(&mut o, Phase::Flop);
    street(&mut o, Phase::River);
    act(
        &mut o,
        Seat::Human,
        Phase::River,
        Action::BetTo(50),
        0,
        true,
        50,
        50,
        100,
    );
    let card = "As".parse().unwrap();
    o.observe(&GameEvent::ShowdownStarted {
        cards: vec![Some([card, card]), None, None, None],
    });
    let s = finish(&mut o, true);
    assert_eq!(
        s.get(Metric::BetSize),
        Count {
            yes: 5000,
            opportunities: 1
        }
    );
    assert_eq!(s.get(Metric::Showdown).yes, 1);
    assert_eq!(s.get(Metric::WonShowdown).yes, 1);
    start(&mut o);
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(40),
        10,
        true,
        40,
        40,
        20,
    );
    assert_eq!(
        finish(&mut o, false).get(Metric::RaiseSize),
        Count {
            yes: 10_000,
            opportunities: 1
        }
    );
}
#[test]
fn hidden_events_never_change_statistics() {
    let mut a = Observer::default();
    let mut b = Observer::default();
    start(&mut a);
    start(&mut b);
    for s in ["As", "2c"] {
        let card = s.parse().unwrap();
        a.observe(&GameEvent::CardsDealt {
            seat: Seat::Human,
            cards: [card, card],
        });
        a.observe(&GameEvent::CardBurned { card });
    }
    assert_eq!(finish(&mut a, false), finish(&mut b, false));
}
#[test]
fn smoothing_confidence_recent_weighting_and_archetype() {
    let empty = OpponentModel::default();
    assert_eq!(empty.archetype(), "Unknown");
    assert!((empty.estimate(Metric::Pfr) - 0.20).abs() < 1e-12);
    let two = rich_model(Metric::RiverAggression, 2, 2);
    assert!(two.estimate(Metric::RiverAggression) < 0.5);
    assert!(two.total.get(Metric::RiverAggression).confidence() < 0.1);
    let mut model = rich_model(Metric::Pfr, 200, 200);
    let before = model.estimate(Metric::Pfr);
    for _ in 0..5 {
        let mut s = HandSample::default();
        s.counts[Metric::Pfr as usize] = Count {
            yes: 0,
            opportunities: 1,
        };
        model.record(s);
    }
    assert!(model.estimate(Metric::Pfr) > before - 0.10);
    for _ in 0..95 {
        let mut s = HandSample::default();
        s.counts[Metric::Pfr as usize] = Count {
            yes: 0,
            opportunities: 1,
        };
        model.record(s);
    }
    assert!(model.estimate(Metric::Pfr) < before - 0.3);
    assert_eq!(model.total.get(Metric::Pfr).yes, 200);
    assert_eq!(model.recent.len(), 50);
    let mut loose = rich_model(Metric::Vpip, 100, 100);
    loose.total.counts[Metric::Pfr as usize] = Count {
        yes: 60,
        opportunities: 100,
    };
    assert_eq!(loose.archetype(), "Loose aggressive");
}
#[test]
fn character_speed_bounds_and_hand_snapshot_stability() {
    let small = rich_model(Metric::FoldLargeRiver, 16, 16);
    assert!(
        !Adaptation::for_player(NpcId::Freya, &small)
            .active
            .is_empty()
    );
    assert!(
        Adaptation::for_player(NpcId::Ananya, &small)
            .active
            .is_empty()
    );
    assert!(
        Adaptation::for_player(NpcId::Yuna, &small)
            .active
            .is_empty()
    );
    let large = rich_model(Metric::FoldLargeRiver, 200, 200);
    let reads = NpcId::ALL.map(|id| Adaptation::for_player(id, &large));
    assert!(
        reads[1].river_bluff > reads[0].river_bluff && reads[0].river_bluff > reads[2].river_bluff
    );
    for (id, a) in NpcId::ALL.into_iter().zip(reads) {
        let base = profile(id.seat()).personality;
        let eff = a.effective(base, Phase::River);
        assert!((eff.bluff_frequency - base.bluff_frequency).abs() <= 0.08);
        assert!(eff.aggression <= 0.98);
        assert!(a.active.len() <= 3);
    }
    let calling = rich_model(Metric::FoldRiver, 0, 200);
    for id in NpcId::ALL {
        let a = Adaptation::for_player(id, &calling);
        assert!(a.river_bluff < 0.0 && a.value > 0.0);
    }
    let mut strategy = PersonalityStrategy::new(profile(Seat::Npc).personality, 1, 16);
    strategy.adaptation = Adaptation::for_player(NpcId::Ananya, &large);
    let frozen = strategy.adaptation.clone();
    let _changed = calling;
    assert_eq!(strategy.adaptation, frozen);
}
#[test]
fn persistence_migration_idempotence_and_resets_are_separate() {
    use crate::memory::{Repository, models::NpcId as Id};
    let dir = std::env::temp_dir().join(format!("poker-opponent-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("test.db");
    {
        let mut r = Repository::open(Some(&path)).unwrap();
        r.begin("session", &Id::ALL, 1).unwrap();
        let sample = rich_model(Metric::Vpip, 1, 1).recent[0].clone();
        r.record_opponent("hand1", &sample, 2).unwrap();
        r.record_opponent("hand1", &sample, 2).unwrap();
        assert_eq!(r.opponent_model().unwrap().hands, 1);
        r.reset(None, false).unwrap();
        assert_eq!(r.opponent_model().unwrap().hands, 1);
    }
    {
        let mut r = Repository::open(Some(&path)).unwrap();
        assert_eq!(r.opponent_model().unwrap().hands, 1);
        r.begin("another", &Id::ALL, 3).unwrap();
        let social = r.snapshots().unwrap();
        r.reset_opponent().unwrap();
        assert_eq!(r.opponent_model().unwrap().hands, 0);
        assert_eq!(
            format!("{:?}", r.snapshots().unwrap()),
            format!("{social:?}")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn seeded_adaptive_simulations_finish_and_change_real_decisions() {
    use super::simulation::{HumanStyle, simulate_adaptive};
    let a = simulate_adaptive(120, 17, 16, HumanStyle::CallingStation, None);
    let b = simulate_adaptive(120, 17, 16, HumanStyle::CallingStation, None);
    assert_eq!(a, b);
    assert_eq!(a.model.hands, 120);
    assert!(a.changed_decisions.iter().sum::<u64>() > 0);
    assert!(a.first_adaptation[1].is_some());
}
#[test]
fn absence_of_human_turns_off_effective_modifiers() {
    let mut g = crate::poker::FourPlayerMatch::new(14);
    g.start_next_hand().unwrap();
    while g.actor() != Some(Seat::Npc) {
        let s = g.actor().unwrap();
        g.act(s, Action::Fold).unwrap();
    }
    let mut v = g.observe(Seat::Npc);
    v.in_hand[0] = false;
    let mut a = PersonalityStrategy::new(profile(Seat::Npc).personality, 1, 16);
    let mut b = a.clone();
    a.adaptation = Adaptation::for_player(NpcId::Ananya, &rich_model(Metric::FoldRiver, 0, 200));
    assert_eq!(a.decide(&v), b.decide(&v));
}

#[test]
fn turn_aggression_and_declined_cbet_use_only_legal_opportunities() {
    let mut o = Observer::default();
    start(&mut o);
    act(
        &mut o,
        Seat::Human,
        Phase::PreFlop,
        Action::RaiseTo(30),
        10,
        true,
        30,
        30,
        15,
    );
    street(&mut o, Phase::Flop);
    act(
        &mut o,
        Seat::Human,
        Phase::Flop,
        Action::Check,
        0,
        true,
        0,
        0,
        60,
    );
    street(&mut o, Phase::Turn);
    act(
        &mut o,
        Seat::Human,
        Phase::Turn,
        Action::BetTo(30),
        0,
        true,
        30,
        30,
        60,
    );
    act(
        &mut o,
        Seat::Npc,
        Phase::Turn,
        Action::AllIn,
        30,
        true,
        40,
        40,
        90,
    );
    // A short all-in has not reopened human raising rights.
    act(
        &mut o,
        Seat::Human,
        Phase::Turn,
        Action::Call,
        10,
        false,
        10,
        40,
        130,
    );
    let s = finish(&mut o, true);
    assert_eq!(
        s.get(Metric::Cbet),
        Count {
            yes: 0,
            opportunities: 1
        }
    );
    assert_eq!(
        s.get(Metric::TurnAggression),
        Count {
            yes: 1,
            opportunities: 1
        }
    );
}

#[test]
fn strategic_hints_require_evidence_and_contain_no_private_or_numeric_facts() {
    assert!(rich_model(Metric::Pfr, 10, 10).hints().is_empty());
    let model = rich_model(Metric::Pfr, 100, 100);
    assert_eq!(model.hints(), vec!["The player has often raised preflop."]);
    assert!(!model.hints().join(" ").chars().any(|c| c.is_ascii_digit()));
    let encoded = serde_json::to_string(&model).unwrap();
    for hidden in ["cards", "seed", "deck", "warmth", "dialogue"] {
        assert!(!encoded.contains(hidden));
    }
}

#[test]
fn worker_restores_learned_state_without_an_llm() {
    use crate::memory::MemoryService;
    let dir = std::env::temp_dir().join(format!(
        "poker-learning-worker-{:016x}",
        rand::random::<u64>()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("test.db");
    let sample = rich_model(Metric::Vpip, 1, 1).recent[0].clone();
    let first = MemoryService::start(Some(path.clone()));
    first.record_opponent("completed-hand".into(), sample.clone());
    assert!(first.flush());
    let second = MemoryService::start(Some(path));
    assert!(second.flush());
    assert_eq!(second.snapshot().opponent.unwrap().recent[0], sample);
    drop(first);
    drop(second);
    std::fs::remove_dir_all(dir).unwrap();
}
