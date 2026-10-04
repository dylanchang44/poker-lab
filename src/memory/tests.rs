use super::*;
use crate::poker::{FourPlayerMatch, events::GameEvent};

struct Temporary(std::path::PathBuf);
impl Temporary {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("poker-memory-test-{:016x}", rand::random::<u64>()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("test.db")
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn quote(text: &str, audience: Audience) -> ObservedEvent {
    ObservedEvent {
        witnesses: NpcId::ALL.to_vec(),
        fact: Fact::Conversation {
            speaker: "human".into(),
            audience,
            text: text.into(),
        },
    }
}
fn setup() -> Repository {
    let mut r = Repository::open(None).unwrap();
    r.begin("session", &NpcId::ALL, 100).unwrap();
    r
}

#[test]
fn initialization_migration_and_future_schema() {
    let temp = Temporary::new();
    let r = Repository::open(Some(&temp.db())).unwrap();
    assert_eq!(r.schema_version().unwrap(), repository::SCHEMA_VERSION);
    drop(r);
    let sql = rusqlite::Connection::open(temp.db()).unwrap();
    sql.execute_batch("ALTER TABLE memories DROP COLUMN audience; ALTER TABLE sessions DROP COLUMN statistics; PRAGMA user_version=2;").unwrap();
    drop(sql);
    let r = Repository::open(Some(&temp.db())).unwrap();
    assert_eq!(r.schema_version().unwrap(), 3);
    assert_eq!(r.snapshots().unwrap().len(), 3);
    drop(r);
    let sql = rusqlite::Connection::open(temp.db()).unwrap();
    sql.execute_batch("PRAGMA user_version=999;").unwrap();
    assert!(Repository::open(Some(&temp.db())).is_err());
}

#[test]
fn restart_restores_memories_relationships_and_summaries() {
    let temp = Temporary::new();
    let mut r = Repository::open(Some(&temp.db())).unwrap();
    r.begin("session", &NpcId::ALL, 1).unwrap();
    r.record(
        "session",
        "chat",
        &quote("I enjoy strategy games", Audience::Private(NpcId::Yuna)),
        2,
    )
    .unwrap();
    r.end("session", 3).unwrap();
    let before = r.snapshots().unwrap();
    drop(r);
    let r = Repository::open(Some(&temp.db())).unwrap();
    assert_eq!(
        serde_json::to_string(&before).unwrap(),
        serde_json::to_string(&r.snapshots().unwrap()).unwrap()
    );
    assert!(
        r.summary("session")
            .unwrap()
            .contains("1 notable conversation")
    );
}

#[test]
fn private_quotes_cannot_reach_other_npcs_or_public_requests() {
    let mut r = setup();
    r.record(
        "session",
        "private",
        &quote(
            "I hate losing with pocket queens",
            Audience::Private(NpcId::Yuna),
        ),
        101,
    )
    .unwrap();
    for npc in [NpcId::Ananya, NpcId::Freya] {
        assert!(
            !r.memories(npc)
                .unwrap()
                .iter()
                .any(|m| m.summary.contains("queens"))
        );
    }
    assert!(
        !r.retrieve(NpcId::Yuna, "queens", Audience::Public, 200)
            .unwrap()
            .memories
            .iter()
            .any(|m| m.contains("queens"))
    );
    assert!(
        r.retrieve(NpcId::Yuna, "queens", Audience::Private(NpcId::Yuna), 201)
            .unwrap()
            .memories
            .iter()
            .any(|m| m.contains("queens"))
    );
}

#[test]
fn public_quotes_propagate_only_to_witnesses() {
    let mut r = setup();
    let mut event = quote("I like chess", Audience::Public);
    event.witnesses = vec![NpcId::Ananya, NpcId::Freya];
    r.record("session", "public", &event, 102).unwrap();
    for npc in NpcId::ALL {
        assert_eq!(
            r.memories(npc)
                .unwrap()
                .iter()
                .any(|m| m.summary.contains("chess")),
            npc != NpcId::Yuna
        );
    }
}

#[test]
fn replay_and_duplicate_quotes_do_not_farm_relationships() {
    let mut r = setup();
    let event = quote("I enjoy strategy games", Audience::Public);
    r.record("session", "a", &event, 101).unwrap();
    let before = r.snapshots().unwrap();
    r.begin("session", &NpcId::ALL, 102).unwrap();
    r.record("session", "a", &event, 102).unwrap();
    r.record(
        "session",
        "b",
        &quote("I enjoy   strategy games", Audience::Public),
        103,
    )
    .unwrap();
    let after = r.snapshots().unwrap();
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.relationship, b.relationship);
        assert_eq!(b.sessions, 1);
        let preference = b
            .memories
            .iter()
            .find(|m| m.kind == MemoryType::PlayerPreference)
            .unwrap();
        assert_eq!(preference.occurrences, 2);
    }
}

#[test]
fn relationships_are_gradual_bounded_and_character_specific() {
    let mut r = setup();
    r.record(
        "session",
        "a",
        &quote("I enjoy probability and strategy", Audience::Public),
        101,
    )
    .unwrap();
    let profiles = r.snapshots().unwrap();
    assert_eq!(profiles[0].relationship.respect, 3);
    assert_eq!(profiles[1].relationship.respect, 0);
    assert_eq!(profiles[2].relationship.warmth, 2);
    assert_eq!(profiles[0].relationship.familiarity, 6);
    let mut relation = Relationship::default();
    for _ in 0..2000 {
        relation.apply(&Relationship {
            familiarity: 2,
            warmth: -1,
            ..Default::default()
        });
    }
    assert_eq!(relation.familiarity, 1000);
    assert_eq!(relation.warmth, 0);
}

#[test]
fn importance_and_preferences_are_conservative() {
    assert!(derive(NpcId::Ananya, &quote("hello", Audience::Public)).is_none());
    assert_eq!(
        derive(NpcId::Ananya, &quote("I enjoy chess", Audience::Public))
            .unwrap()
            .kind,
        MemoryType::PlayerPreference
    );
    let event = ObservedEvent {
        witnesses: NpcId::ALL.to_vec(),
        fact: Fact::HandFinished {
            hand: 1,
            pot: 600,
            awards: vec![600, 0, 0, 0],
            public_showdown: false,
            shown_river_aggression: false,
        },
    };
    let memory = derive(NpcId::Freya, &event).unwrap();
    assert_eq!(memory.importance, 85);
    assert!(!memory.summary.contains("bluff"));
}

#[test]
fn relevance_recall_cooldown_and_retention_are_bounded() {
    let mut r = setup();
    for i in 0..120 {
        r.record(
            "session",
            &format!("chat{i}"),
            &quote(&format!("I enjoy game number {i}"), Audience::Public),
            101 + i,
        )
        .unwrap();
    }
    r.record(
        "session",
        "chess",
        &quote("I enjoy chess strategy", Audience::Public),
        300,
    )
    .unwrap();
    assert_eq!(
        r.memories(NpcId::Ananya).unwrap().len(),
        repository::MEMORY_LIMIT
    );
    let context = r
        .retrieve(NpcId::Ananya, "chess strategy", Audience::Public, 400)
        .unwrap();
    assert_eq!(context.memories.len(), 3);
    assert!(context.memories[0].contains("chess"));
    let next = r
        .retrieve(NpcId::Ananya, "chess strategy", Audience::Public, 401)
        .unwrap();
    assert!(!next.memories.iter().any(|m| context.memories.contains(m)));
    assert!(
        r.memories(NpcId::Ananya)
            .unwrap()
            .iter()
            .any(|m| m.recall_count == 1)
    );
}

#[test]
fn hidden_deals_burns_and_shuffle_seeds_never_enter_facts() {
    fn facts(seed: u64) -> String {
        let mut game = FourPlayerMatch::new(seed);
        game.start_next_hand().unwrap();
        let mut observer = PokerObserver::default();
        let facts: Vec<_> = game
            .history()
            .iter()
            .flat_map(|e| observer.observe(e))
            .collect();
        serde_json::to_string(&facts).unwrap()
    }
    assert_eq!(facts(1), facts(99999));
    let mut observer = PokerObserver::default();
    assert!(
        observer
            .observe(&GameEvent::CardBurned {
                card: "As".parse().unwrap()
            })
            .is_empty()
    );
    assert!(
        observer
            .observe(&GameEvent::CardsDealt {
                seat: crate::poker::Seat::Human,
                cards: ["As".parse().unwrap(), "Ah".parse().unwrap()]
            })
            .is_empty()
    );
}

#[test]
fn public_showdown_observation_distinguishes_all_in_calls_and_raises() {
    use crate::poker::{Action, Phase, Seat, state::Outcome};
    for (total, expected) in [(100, false), (200, true)] {
        let mut observer = PokerObserver::default();
        observer.observe(&GameEvent::HandStarted {
            number: 1,
            seed: 999,
            dealer: Seat::Human,
            stacks: vec![1000; 4],
            blinds: [5, 10],
        });
        observer.observe(&GameEvent::CommunityCardsDealt {
            phase: Phase::River,
            cards: "2s 4h 7d 9c Js"
                .split_whitespace()
                .map(|s| s.parse().unwrap())
                .collect(),
        });
        observer.observe(&GameEvent::PlayerActed {
            seat: Seat::Npc,
            phase: Phase::River,
            action: Action::BetTo(100),
            paid: 100,
            street_total: 100,
            pot: 300,
            stacks: vec![900; 4],
        });
        observer.observe(&GameEvent::PlayerActed {
            seat: Seat::Human,
            phase: Phase::River,
            action: Action::AllIn,
            paid: total,
            street_total: total,
            pot: 300 + total,
            stacks: vec![0, 900, 900, 900],
        });
        observer.observe(&GameEvent::ShowdownStarted {
            cards: vec![
                Some(["As".parse().unwrap(), "Kh".parse().unwrap()]),
                None,
                None,
                None,
            ],
        });
        let facts = observer.observe(&GameEvent::HandCompleted {
            outcome: Outcome {
                winner: Some(Seat::Human),
                pot: 500,
                awards: vec![500, 0, 0, 0],
                hands: None,
                folded: None,
                pots: vec![],
            },
            stacks: vec![500, 900, 900, 900],
        });
        assert!(
            matches!(facts[0].fact, Fact::HandFinished { shown_river_aggression, .. } if shown_river_aggression == expected)
        );
    }
}

#[test]
fn resets_are_scoped_and_preserve_poker_history() {
    let mut r = setup();
    r.record(
        "session",
        "a",
        &quote("I enjoy chess", Audience::Public),
        101,
    )
    .unwrap();
    r.reset(Some(NpcId::Ananya), true).unwrap();
    assert_eq!(
        r.snapshots().unwrap()[0].relationship,
        Relationship::default()
    );
    assert!(!r.memories(NpcId::Ananya).unwrap().is_empty());
    r.reset(Some(NpcId::Ananya), false).unwrap();
    assert!(r.memories(NpcId::Ananya).unwrap().is_empty());
    assert!(!r.memories(NpcId::Yuna).unwrap().is_empty());
    r.reset(None, false).unwrap();
    assert!(
        r.snapshots()
            .unwrap()
            .iter()
            .all(|s| s.sessions == 0 && s.memories.is_empty())
    );
    assert!(r.summary("session").is_ok());
}

#[test]
fn worker_saves_without_llm_and_degrades_on_invalid_storage() {
    let temp = Temporary::new();
    let service = MemoryService::start(Some(temp.db()));
    service.begin("session".into(), NpcId::ALL.to_vec());
    service.record(
        "session".into(),
        "chat".into(),
        quote("I enjoy chess", Audience::Public),
    );
    service.end("session".into());
    assert!(service.flush());
    assert!(service.snapshot().persistent);
    let r = Repository::open(Some(&temp.db())).unwrap();
    assert!(
        r.memories(NpcId::Yuna)
            .unwrap()
            .iter()
            .any(|m| m.summary.contains("chess"))
    );
    let fallback = MemoryService::start(Some(temp.db().join("invalid.db")));
    fallback.begin("offline".into(), NpcId::ALL.to_vec());
    assert!(fallback.flush());
    assert!(!fallback.snapshot().persistent);
    assert!(fallback.snapshot().error.is_some());
    assert_eq!(fallback.snapshot().characters[0].sessions, 1);
}

#[test]
fn mock_dialogue_retrieves_only_the_speakers_memories() {
    use crate::conversation::*;
    let service = MemoryService::start(None);
    service.begin("session".into(), NpcId::ALL.to_vec());
    service.record(
        "session".into(),
        "private".into(),
        quote(
            "I enjoy secret chess puzzles",
            Audience::Private(NpcId::Yuna),
        ),
    );
    assert!(service.flush());
    let mut game = FourPlayerMatch::new(4);
    game.start_next_hand().unwrap();
    let mut request = TurnRequest {
        session: 1,
        hand: 1,
        speaker: Speaker::Yuna,
        kind: InteractionType::Reply,
        context: public_context(&game.observe(crate::poker::Seat::Human), &[]),
        history: vec![],
        player_text: Some("Remember secret chess puzzles?".into()),
        may_interject: false,
        audience: Audience::Private(NpcId::Yuna),
        social: SocialContext::default(),
        mood: crate::social::MoodState::baseline(NpcId::Yuna),
    };
    let provider = RememberingProvider {
        inner: Arc::new(MockProvider),
        memory: service,
    };
    let reply = provider.respond(&request).unwrap();
    assert!(reply.contains("secret chess puzzles"));
    assert!(parse_response(&reply, Speaker::Yuna).is_ok());
    request.speaker = Speaker::Freya;
    request.audience = Audience::Private(NpcId::Freya);
    assert!(
        !provider
            .respond(&request)
            .unwrap()
            .contains("secret chess puzzles")
    );
}

#[test]
fn transactions_rollback_when_database_is_locked_and_retry_is_idempotent() {
    let temp = Temporary::new();
    let mut r = Repository::open(Some(&temp.db())).unwrap();
    r.begin("session", &NpcId::ALL, 1).unwrap();
    let lock = rusqlite::Connection::open(temp.db()).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let event = quote("I enjoy chess", Audience::Public);
    assert!(r.record("session", "a", &event, 2).is_err());
    assert_eq!(r.snapshots().unwrap()[0].relationship.familiarity, 5);
    lock.execute_batch("ROLLBACK;").unwrap();
    r.record("session", "a", &event, 2).unwrap();
    r.record("session", "a", &event, 2).unwrap();
    assert_eq!(r.snapshots().unwrap()[0].relationship.familiarity, 6);
}

#[test]
fn source_retention_preserves_bounded_audit_and_session_totals() {
    let temp = Temporary::new();
    let mut r = Repository::open(Some(&temp.db())).unwrap();
    r.begin("session", &NpcId::ALL, 1).unwrap();
    for i in 0..530 {
        r.record(
            "session",
            &format!("hand{i}"),
            &ObservedEvent {
                witnesses: NpcId::ALL.to_vec(),
                fact: Fact::HandStarted {
                    hand: i,
                    participants: NpcId::ALL.to_vec(),
                },
            },
            2 + i as i64,
        )
        .unwrap();
    }
    r.end("session", 600).unwrap();
    assert!(r.summary("session").unwrap().starts_with("530 hands"));
    let sql = rusqlite::Connection::open(temp.db()).unwrap();
    let count: i64 = sql
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert!((512..=524).contains(&count));
    let broken:i64 = sql.query_row("SELECT COUNT(*) FROM memories m LEFT JOIN events e ON m.source_event_id=e.id WHERE e.id IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(broken, 0);
}

#[test]
fn relationship_and_relevant_memory_enrich_the_same_character_request() {
    use crate::conversation::*;
    struct Capture(std::sync::Mutex<Option<TurnRequest>>);
    impl DialogueProvider for Capture {
        fn respond(&self, request: &TurnRequest) -> Result<String, String> {
            *self.0.lock().unwrap() = Some(request.clone());
            MockProvider.respond(request)
        }
    }
    let service = MemoryService::start(None);
    service.begin("context".into(), NpcId::ALL.to_vec());
    service.record(
        "context".into(),
        "preference".into(),
        quote("I enjoy strategy games", Audience::Private(NpcId::Ananya)),
    );
    assert!(service.flush());
    let capture = Arc::new(Capture(std::sync::Mutex::new(None)));
    let provider = RememberingProvider {
        inner: capture.clone(),
        memory: service,
    };
    let mut game = FourPlayerMatch::new(42);
    game.start_next_hand().unwrap();
    let request = TurnRequest {
        session: 1,
        hand: 1,
        speaker: Speaker::Ananya,
        kind: InteractionType::Reply,
        context: public_context(&game.observe(crate::poker::Seat::Human), &[]),
        history: vec![],
        player_text: Some("Strategy games?".into()),
        may_interject: false,
        audience: Audience::Private(NpcId::Ananya),
        social: SocialContext::default(),
        mood: crate::social::MoodState::baseline(NpcId::Ananya),
    };
    provider.respond(&request).unwrap();
    let captured = capture.0.lock().unwrap();
    let context = captured.as_ref().unwrap();
    assert_eq!(context.social.relationship.as_ref().unwrap().respect, 3);
    assert!(
        context
            .social
            .memories
            .iter()
            .any(|m| m.contains("strategy games"))
    );
    assert_eq!(context.mood, request.mood);
    assert!(request.social.relationship.is_none());
}

#[test]
fn repeated_criticism_changes_warmth_gradually_but_model_quotes_do_not() {
    let mut r = setup();
    for i in 0..5 {
        r.record(
            "session",
            &format!("nice{i}"),
            &quote(&format!("I enjoy strategy game {i}"), Audience::Public),
            101 + i,
        )
        .unwrap();
    }
    let before = r.snapshots().unwrap()[0].relationship.warmth;
    for i in 0..2 {
        r.record(
            "session",
            &format!("criticism{i}"),
            &quote("You're annoying today.", Audience::Private(NpcId::Ananya)),
            120 + i,
        )
        .unwrap();
    }
    let after = r.snapshots().unwrap();
    assert_eq!(after[0].relationship.warmth, before - 4);
    assert_eq!(after[1].relationship.warmth, 5);
}
