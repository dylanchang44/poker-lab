use super::*;
use crate::poker::PokerMatch;
use std::{sync::Arc, time::Duration};

fn context(seed: u64) -> PublicContext {
    let mut game = crate::poker::FourPlayerMatch::new(seed);
    game.start_next_hand().unwrap();
    public_context(&game.observe(Seat::Human), &[])
}
fn manager() -> ConversationManager {
    let config = ConversationConfig::default();
    let mut manager = ConversationManager::new(config, Arc::new(MockProvider));
    manager.reset(11);
    manager
}

#[test]
fn private_history_is_excluded_from_other_npcs_and_public_speech() {
    let mut manager = manager();
    let context = context(44);
    assert!(manager.human_message("I enjoy secret chess puzzles", Target::Yuna, context, 0.0));
    assert!(
        manager
            .visible_history(Speaker::Yuna, Audience::Private(NpcId::Yuna))
            .iter()
            .any(|m| m.text.contains("secret"))
    );
    for speaker in [Speaker::Ananya, Speaker::Freya, Speaker::Yuna] {
        assert!(
            manager
                .visible_history(speaker, Audience::Public)
                .is_empty()
        );
    }
    assert!(
        manager
            .visible_history(Speaker::Freya, Audience::Private(NpcId::Freya))
            .is_empty()
    );
}
fn await_line(manager: &mut ConversationManager, session: u64, hand: u64) -> DialogueLine {
    for _ in 0..100 {
        if let Some(line) = manager.tick(session, hand) {
            return line;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("no dialogue line");
}

#[test]
fn configuration_defaults_and_validation() {
    let config = ConversationConfig::load(None).unwrap();
    assert_eq!(config.provider, ProviderKind::Mock);
    assert_eq!(config.history_limit, 16);
    let mut invalid = config.clone();
    invalid.provider = ProviderKind::Remote;
    assert!(invalid.validate().is_err());
    invalid.base_url = "https://example.test/v1".into();
    assert!(invalid.validate().is_ok());
    invalid.temperature = 5.0;
    assert!(invalid.validate().is_err());
    let mut bad_mock = config;
    bad_mock.history_limit = 0;
    assert!(bad_mock.validate().is_err());
}

#[test]
fn desktop_defaults_to_local_but_explicit_mock_still_wins() {
    let dir = std::env::temp_dir().join(format!("poker-config-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&dir).unwrap();
    let local = dir.join("conversation.json");
    let config = ConversationConfig::for_application(None, &local).unwrap();
    assert_eq!(config.provider, ProviderKind::Local);
    assert_eq!(config.model, "auto");
    assert_eq!(config.reasoning_effort, Some(ReasoningEffort::Off));
    assert!(config.structured_output);
    config.validate().unwrap();
    std::fs::write(&local, r#"{"provider":"local","model":"chosen"}"#).unwrap();
    assert_eq!(
        ConversationConfig::for_application(None, &local)
            .unwrap()
            .model,
        "chosen"
    );
    let explicit = dir.join("mock.json");
    std::fs::write(&explicit, r#"{"provider":"mock"}"#).unwrap();
    assert_eq!(
        ConversationConfig::for_application(Some(&explicit), &local)
            .unwrap()
            .provider,
        ProviderKind::Mock
    );
    std::fs::write(&explicit, "invalid").unwrap();
    assert!(ConversationConfig::for_application(Some(&explicit), &local).is_err());
    std::fs::remove_file(explicit).unwrap();
    std::fs::remove_file(local).unwrap();
    std::fs::remove_dir(dir).unwrap();
    let config: ConversationConfig = serde_json::from_str(
        r#"{"provider":"remote","base_url":"https://example.invalid/v1","model":"chosen"}"#,
    )
    .unwrap();
    config.validate().unwrap();
    assert_eq!(config.reasoning_effort, None);
    assert!(!config.structured_output);
    assert!(
        serde_json::from_str::<ConversationConfig>(r#"{"reasoning_effort":"invented"}"#).is_err()
    );
}

#[test]
fn discovery_selects_only_one_loaded_chat_model() {
    use serde_json::json;
    let llm = json!({"type":"llm","key":"disk-name","loaded_instances":[{"id":"loaded-name"}]});
    let embedding = json!({"type":"embedding","loaded_instances":[{"id":"not-a-chat-model"}]});
    let unloaded = json!({"type":"llm","key":"huge-model","loaded_instances":[]});
    assert_eq!(
        provider::loaded_model(&json!({"models":[llm,embedding,unloaded]})).unwrap(),
        "loaded-name"
    );
    assert!(
        provider::loaded_model(&json!({"models":[unloaded,embedding]}))
            .unwrap_err()
            .contains("no loaded")
    );
    assert!(
        provider::loaded_model(&json!({"models":[llm,llm]}))
            .unwrap_err()
            .contains("multiple loaded")
    );
    assert!(provider::loaded_model(&json!({"data":[]})).is_err());
}

#[test]
fn reasoning_only_and_truncated_outputs_are_not_dialogue() {
    use serde_json::json;
    let limited = json!({"choices":[{"finish_reason":"length","message":{"content":"", "reasoning_content":"PRIVATE_REASONING"}}]});
    let error = provider::dialogue_content(limited).unwrap_err();
    assert!(error.contains("token limit"));
    assert!(!error.contains("PRIVATE_REASONING"));
    assert!(provider::dialogue_content(json!({"choices":[{"message":{"content":"  "}}]})).is_err());
    assert_eq!(
        provider::dialogue_content(
            json!({"choices":[{"finish_reason":"stop","message":{"content":"a reply"}}]})
        )
        .unwrap(),
        "a reply"
    );
}

#[test]
fn provenance_and_connection_status_are_host_owned_and_reset() {
    let config = ConversationConfig::local_development();
    let mut manager = ConversationManager::new(config, Arc::new(MockProvider));
    assert_eq!(manager.status, ProviderStatus::Unverified);
    let request = manager.request(
        Speaker::Yuna,
        InteractionType::Reply,
        context(1),
        Some("Hello".into()),
    );
    let reply = r#"{"speaker":"yuna","dialogue":"Hello there."}"#.to_string();
    let line = manager.deliver(request.clone(), Ok(reply)).unwrap();
    assert_eq!(line.source, DialogueSource::Model);
    assert_eq!(manager.status, ProviderStatus::Ready);
    let line = manager
        .deliver(
            request.clone(),
            Err("network or protocol error SECRET".into()),
        )
        .unwrap();
    assert_eq!(line.source, DialogueSource::Fallback);
    assert_eq!(
        manager.history().back().unwrap().source,
        Some(DialogueSource::Fallback)
    );
    assert_eq!(manager.label(), "Scripted fallback: server offline");
    assert!(line.source.badge().contains("scripted fallback"));
    assert_eq!(
        parse_social_response(
            r#"{"speaker":"yuna","dialogue":"Hello","source":"model"}"#,
            Speaker::Yuna,
            240
        ),
        Err(ResponseError::InvalidMetadata)
    );
    manager.reset(2);
    assert_eq!(manager.status, ProviderStatus::Unverified);
    assert!(!manager.verified);
    let mut mock = ConversationManager::new(ConversationConfig::default(), Arc::new(MockProvider));
    let line = mock
        .deliver(request.clone(), MockProvider.respond(&request))
        .unwrap();
    assert_eq!(line.source, DialogueSource::Scripted);
    assert_eq!(mock.label(), "Scripted mock (no model)");
}

#[test]
fn response_schema_rejects_wrong_identity_expression_and_length() {
    let valid = r#"{"speaker":"freya","dialogue":"A neat move.","expression":"happy","interaction_type":"reply"}"#;
    assert_eq!(
        parse_response(valid, Speaker::Freya),
        Ok(("A neat move.".into(), Some(CharacterExpression::Happy)))
    );
    assert_eq!(
        parse_response(valid, Speaker::Yuna),
        Err(ResponseError::WrongSpeaker)
    );
    assert_eq!(
        parse_response("oops", Speaker::Freya),
        Err(ResponseError::Malformed)
    );
    assert_eq!(
        parse_response(
            r#"{"speaker":"freya","dialogue":" ","expression":"happy"}"#,
            Speaker::Freya
        ),
        Err(ResponseError::Empty)
    );
    assert_eq!(
        parse_response(
            r#"{"speaker":"freya","dialogue":"Hi","expression":"eliminated"}"#,
            Speaker::Freya
        ),
        Err(ResponseError::InvalidExpression)
    );
    assert_eq!(
        parse_response(r#"{"speaker":"unknown","dialogue":"Hi"}"#, Speaker::Freya),
        Err(ResponseError::WrongSpeaker)
    );
    let long = serde_json::json!({"speaker":"freya","dialogue":"x".repeat(181)}).to_string();
    assert_eq!(
        parse_response(&long, Speaker::Freya),
        Err(ResponseError::TooLong)
    );
}

#[test]
fn public_context_never_contains_private_cards_or_seed() {
    let mut a = PokerMatch::new(1);
    let mut b = PokerMatch::new(9999);
    a.start_next_hand().unwrap();
    b.start_next_hand().unwrap();
    let mut view = a.observe(Seat::Human);
    let public = public_context(&view, &[]);
    assert_eq!(public, public_context(&b.observe(Seat::Human), &[]));
    view.hole_cards = Some(["As".parse().unwrap(), "Ah".parse().unwrap()]);
    view.revealed_cards = Some(vec![
        Some(["Ks".parse().unwrap(), "Kh".parse().unwrap()]),
        None,
    ]);
    assert_eq!(public, public_context(&view, &[]));
    let json = serde_json::to_string(&public).unwrap();
    assert!(!json.contains("hole_cards") && !json.contains("seed") && !json.contains("As"));
}

#[test]
fn targeted_and_table_routing_and_mock_provider() {
    let mut manager = manager();
    let context = context(44);
    assert!(manager.human_message("Hello, Yuna", Target::Yuna, context.clone(), 0.0));
    let line = await_line(&mut manager, 11, context.hand);
    assert_eq!(line.speaker, Seat::Nova);
    assert_eq!(line.expression, Some(CharacterExpression::Thinking));
    assert_eq!(manager.history()[0].speaker, Speaker::Human);
    assert_eq!(manager.history()[1].speaker, Speaker::Yuna);
    assert!(manager.human_message(
        "What do you all think?",
        Target::Table,
        context.clone(),
        1.0
    ));
    let line = await_line(&mut manager, 11, context.hand);
    assert_ne!(line.speaker, Seat::Human);
    assert_eq!(manager.history().len(), 4);
}

#[test]
fn third_table_exchange_can_include_one_npc_interjection() {
    let mut manager = manager();
    let context = context(45);
    for n in 0..3 {
        assert!(manager.human_message(
            &format!("Question {n}"),
            Target::Table,
            context.clone(),
            n as f32
        ));
        await_line(&mut manager, 11, context.hand);
    }
    let fourth = await_line(&mut manager, 11, context.hand);
    assert!(matches!(fourth.speaker, Seat::Npc | Seat::Jax | Seat::Nova));
    assert_ne!(
        manager.history()[manager.history().len() - 1].speaker,
        Speaker::Human
    );
}

#[test]
fn bounded_history_and_invalid_human_input() {
    let mut config = ConversationConfig {
        history_limit: 4,
        ..Default::default()
    };
    let mut manager = ConversationManager::new(config.clone(), Arc::new(MockProvider));
    manager.reset(7);
    for i in 0..5 {
        assert!(manager.human_message(
            &format!("Message {i}"),
            Target::Ananya,
            context(3),
            i as f32
        ));
    }
    assert_eq!(manager.history().len(), 4);
    assert_eq!(manager.history().front().unwrap().text, "Message 1");
    assert!(!manager.human_message("", Target::Table, context(3), 0.0));
    assert!(!manager.human_message(&"x".repeat(241), Target::Table, context(3), 0.0));
    config.enabled = false;
    let mut disabled = ConversationManager::new(config, Arc::new(MockProvider));
    disabled.reset(9);
    assert!(!disabled.human_message("Hi", Target::Table, context(3), 0.0));
}

#[test]
fn cooldown_duplicate_event_and_frequency_suppression() {
    let mut manager = manager();
    let context = context(99);
    manager.offer_event(10, Speaker::Freya, 3, context.clone(), 0.0);
    let _ = await_line(&mut manager, 11, context.hand);
    manager.offer_event(10, Speaker::Freya, 3, context.clone(), 30.0);
    manager.offer_event(11, Speaker::Yuna, 3, context.clone(), 1.0);
    assert!(manager.queued.is_none());
    manager.offer_event(12, Speaker::Yuna, 3, context.clone(), 20.0);
    assert!(manager.queued.is_some());
    manager.queued = None;
    manager.config.initiative_frequency = 0.0;
    manager.offer_event(13, Speaker::Freya, 3, context, 40.0);
    assert!(manager.queued.is_none());
}

struct DelayedProvider;
impl DialogueProvider for DelayedProvider {
    fn respond(&self, request: &TurnRequest) -> Result<String, String> {
        std::thread::sleep(Duration::from_millis(1300));
        Ok(
            serde_json::json!({"speaker":request.speaker.id(),"dialogue":"Late arrival."})
                .to_string(),
        )
    }
}

#[test]
fn restart_and_hand_change_discard_old_worker_result() {
    let mut manager =
        ConversationManager::new(ConversationConfig::default(), Arc::new(DelayedProvider));
    manager.reset(5);
    let context = context(4);
    manager.human_message("Hello", Target::Freya, context.clone(), 0.0);
    assert!(manager.tick(5, context.hand).is_none());
    manager.reset(6);
    std::thread::sleep(Duration::from_millis(10));
    assert!(manager.tick(6, context.hand).is_none());
    assert!(manager.history().is_empty());
    manager.human_message("Again", Target::Freya, context.clone(), 1.0);
    manager.tick(6, context.hand);
    // A result for a prior hand is discarded even if the session survives.
    std::thread::sleep(Duration::from_millis(10));
    assert!(manager.tick(6, context.hand + 1).is_none());
}

#[test]
fn timed_out_provider_falls_back_without_blocking_caller() {
    let config = ConversationConfig {
        timeout_seconds: 1,
        ..Default::default()
    };
    let mut manager = ConversationManager::new(config, Arc::new(DelayedProvider));
    manager.reset(9);
    let context = context(6);
    manager.human_message("Hello", Target::Freya, context.clone(), 0.0);
    let start = Instant::now();
    manager.tick(9, context.hand);
    assert!(start.elapsed() < Duration::from_millis(100));
    manager.pending.as_mut().unwrap().begun -= Duration::from_secs(2);
    let line = manager.tick(9, context.hand).unwrap();
    assert_eq!(line.speaker, Seat::Jax);
    assert_eq!(manager.status, ProviderStatus::Fallback);
}

#[test]
fn heads_up_routing_only_addresses_the_present_opponent() {
    let mut game = PokerMatch::new(1);
    game.start_next_hand().unwrap();
    let context = public_context(&game.observe(Seat::Human), &[]);
    let mut manager = manager();
    assert!(!manager.human_message("Hi", Target::Freya, context.clone(), 0.0));
    assert!(manager.human_message("Hi", Target::Table, context.clone(), 0.0));
    assert_eq!(
        await_line(&mut manager, 11, context.hand).speaker,
        Seat::Npc
    );
    assert_eq!(
        parse_response(
            r#"{"speaker":"ananya","dialogue":"Hi","expression":42}"#,
            Speaker::Ananya
        ),
        Err(ResponseError::InvalidExpression)
    );
}

#[test]
fn restarted_session_does_not_start_a_second_worker() {
    let mut manager =
        ConversationManager::new(ConversationConfig::default(), Arc::new(DelayedProvider));
    manager.reset(1);
    manager.human_message("First", Target::Ananya, context(1), 0.0);
    manager.tick(1, 1);
    assert!(manager.worker_busy.load(Ordering::Acquire));
    manager.reset(2);
    manager.human_message("Second", Target::Ananya, context(1), 1.0);
    manager.tick(2, 1);
    assert!(manager.pending.is_none());
    assert!(manager.queued.is_some());
    assert_eq!(manager.history().len(), 1);
}

#[test]
fn completed_result_from_a_previous_hand_is_rejected() {
    let mut manager = manager();
    let request = manager.request(Speaker::Ananya, InteractionType::Reply, context(1), None);
    let (sender, receiver) = mpsc::channel();
    sender.send(MockProvider.respond(&request)).unwrap();
    manager.pending = Some(Pending {
        receiver: Mutex::new(receiver),
        request,
        begun: Instant::now(),
    });
    assert!(manager.tick(11, 2).is_none());
    assert!(manager.history().is_empty());
}

#[test]
fn missing_remote_key_uses_safe_error_without_network() {
    let config = ConversationConfig {
        provider: ProviderKind::Remote,
        base_url: "https://example.invalid/v1".into(),
        api_key_env: "POKER_LAB_STAGE4_TEST_KEY_DOES_NOT_EXIST".into(),
        ..Default::default()
    };
    let provider = configured_provider(&config);
    let request = TurnRequest {
        session: 1,
        hand: 1,
        speaker: Speaker::Freya,
        kind: InteractionType::Reply,
        context: context(8),
        history: vec![],
        player_text: Some("Hello".into()),
        may_interject: false,
        audience: Audience::Public,
        social: SocialContext::default(),
        mood: MoodState::baseline(NpcId::Freya),
    };
    let response = provider.respond(&request).unwrap_err();
    assert!(response.contains("not configured"));
    assert!(!response.contains("Bearer"));
}

#[test]
fn openai_compatible_adapter_posts_bounded_public_context() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = stream.read(&mut chunk).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(split) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..split]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap();
                if bytes.len() >= split + 4 + length {
                    break;
                }
            }
        }
        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(request.contains("Public table snapshot"));
        assert!(request.contains("Temporary character state"));
        assert!(request.contains("Stable personality"));
        assert!(request.contains("Familiarity and trust influence comfort"));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["messages"][1]["content"], "Hello");
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(body["response_format"]["type"], "json_schema");
        let schema = &body["response_format"]["json_schema"]["schema"];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["speaker"]["enum"],
            serde_json::json!(["freya"])
        );
        assert_eq!(schema["properties"]["dialogue"]["maxLength"], 240);
        assert!(!request.contains("SCRIPTED_FIXTURE_DO_NOT_SEND"));
        assert!(!request.to_lowercase().contains("authorization: bearer"));
        assert!(!request.contains("hole_cards") && !request.contains("CardBurned"));
        let result = serde_json::json!({"choices":[{"message":{"content":"{\"speaker\":\"freya\",\"dialogue\":\"A fine hand.\",\"expression\":\"happy\"}"}}]}).to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", result.len(), result).unwrap();
    });
    let config = ConversationConfig {
        provider: ProviderKind::Local,
        base_url: format!("http://{address}/v1"),
        model: "fixture-model".into(),
        ..ConversationConfig::local_development()
    };
    let request = TurnRequest {
        session: 1,
        hand: 1,
        speaker: Speaker::Freya,
        kind: InteractionType::Reply,
        context: context(8),
        history: vec![ChatMessage {
            speaker: Speaker::Freya,
            text: "SCRIPTED_FIXTURE_DO_NOT_SEND".into(),
            audience: Audience::Public,
            source: Some(DialogueSource::Fallback),
        }],
        player_text: Some("Hello".into()),
        may_interject: false,
        audience: Audience::Public,
        social: SocialContext::default(),
        mood: MoodState::baseline(NpcId::Freya),
    };
    let raw = configured_provider(&config).respond(&request).unwrap();
    assert_eq!(
        parse_response(&raw, Speaker::Freya).unwrap().0,
        "A fine hand."
    );
    server.join().unwrap();
}

#[test]
fn rich_response_validates_intents_declines_silence_and_metadata() {
    let raw = r#"{"speaker":"yuna","dialogue":"Maybe another time.","tone":"gentle","social_intent":"decline_invitation","relationship_signal":"warm","conversation_continuation":false}"#;
    let response = parse_social_response(raw, Speaker::Yuna, 180).unwrap();
    assert_eq!(response.intent, SocialIntent::DeclineInvitation);
    assert!(!response.continuation);
    assert_eq!(response.relationship_signal, RelationshipSignal::Warm);
    for field in [
        r#""tone":"obedient""#,
        r#""social_intent":"change_bet""#,
        r#""relationship_delta":100"#,
        r#""conversation_continuation":"yes""#,
    ] {
        let raw = format!("{{\"speaker\":\"yuna\",\"dialogue\":\"Hello\",{field}}}");
        assert_eq!(
            parse_social_response(&raw, Speaker::Yuna, 180),
            Err(ResponseError::InvalidMetadata)
        );
    }
    assert!(parse_social_response(r#"{"speaker":"yuna","dialogue":"","silent":true,"expression":null,"conversation_continuation":false}"#,Speaker::Yuna,180).unwrap().silent);
    let long = serde_json::json!({"speaker":"yuna","dialogue":"a".repeat(210)}).to_string();
    assert!(parse_social_response(&long, Speaker::Yuna, 240).is_ok());
    assert_eq!(
        parse_social_response(&long, Speaker::Yuna, 180),
        Err(ResponseError::TooLong)
    );
}

#[test]
fn mood_is_private_to_the_recipient_and_resets_without_personality_drift() {
    let mut manager = manager();
    let before = system_prompt(Speaker::Yuna);
    let anya = manager.mood(NpcId::Ananya).clone();
    for _ in 0..3 {
        manager.human_message("You're annoying today.", Target::Yuna, context(1), 0.0);
    }
    assert_eq!(
        manager.mood(NpcId::Yuna).mood,
        crate::social::Mood::Irritated
    );
    assert_eq!(*manager.mood(NpcId::Ananya), anya);
    assert_eq!(
        manager.queued.as_ref().unwrap().mood.mood,
        crate::social::Mood::Irritated
    );
    manager.reset(99);
    assert_eq!(*manager.mood(NpcId::Yuna), MoodState::baseline(NpcId::Yuna));
    assert_eq!(system_prompt(Speaker::Yuna), before);
    assert_ne!(system_prompt(Speaker::Yuna), system_prompt(Speaker::Freya));
}

#[test]
fn initiative_varies_by_character_and_irritation_suppresses_casual_comments() {
    let mut counts = [0; 3];
    for event in 1..=60 {
        for (i, npc) in NpcId::ALL.into_iter().enumerate() {
            let mut manager = manager();
            manager.last_event = event;
            counts[i] +=
                usize::from(manager.can_initiate(Speaker::from_seat(npc.seat()), 1, 100.0));
        }
    }
    assert!(counts[1] > counts[0] && counts[0] > counts[2]);
    let mut manager = manager();
    for _ in 0..4 {
        manager.human_message("You're annoying", Target::Freya, context(2), 0.0);
    }
    assert!(!manager.can_initiate(Speaker::Freya, 1, 100.0));
}

struct FixedSocialProvider(&'static str);
impl DialogueProvider for FixedSocialProvider {
    fn respond(&self, _: &TurnRequest) -> Result<String, String> {
        Ok(self.0.into())
    }
}
#[test]
fn silence_and_conversation_end_do_not_spawn_endless_replies() {
    let mut manager = ConversationManager::new(
        ConversationConfig::default(),
        Arc::new(FixedSocialProvider(
            r#"{"speaker":"freya","dialogue":"","silent":true}"#,
        )),
    );
    manager.reset(11);
    manager.human_message("Hm.", Target::Freya, context(1), 0.0);
    for _ in 0..30 {
        assert!(manager.tick(11, 1).is_none());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(manager.history.len(), 1);
    assert!(manager.queued.is_none() && manager.pending.is_none());
    let mut manager = ConversationManager::new(
        ConversationConfig::default(),
        Arc::new(FixedSocialProvider(
            r#"{"speaker":"freya","dialogue":"I'll leave it there.","social_intent":"end_conversation","conversation_continuation":false}"#,
        )),
    );
    manager.reset(11);
    manager.human_count = 2;
    manager.human_message("What do you think?", Target::Table, context(1), 0.0);
    await_line(&mut manager, 11, 1);
    assert!(manager.queued.is_none());
}

#[test]
fn generated_relationship_hints_cannot_change_authoritative_mood_or_relationships() {
    let mut manager = manager();
    let request = manager.request(
        Speaker::Freya,
        InteractionType::Reply,
        context(5),
        Some("Hello".into()),
    );
    let before = manager.moods.clone();
    let line=manager.deliver(request,Ok(r#"{"speaker":"freya","dialogue":"Maybe. What did you have in mind?","social_intent":"ask_question","relationship_signal":"warm"}"#.into())).unwrap();
    assert_eq!(line.speaker, Seat::Jax);
    assert_eq!(manager.moods, before);
    let fact = crate::memory::ObservedEvent {
        witnesses: vec![NpcId::Freya],
        fact: crate::memory::Fact::Conversation {
            speaker: "freya".into(),
            audience: Audience::Public,
            text: "I think your strategy discussion was interesting.".into(),
        },
    };
    assert_eq!(
        crate::memory::models::derive(NpcId::Freya, &fact)
            .unwrap()
            .delta,
        crate::memory::Relationship::default()
    );
}

#[test]
fn local_outage_uses_fallback_without_credentials() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let config = ConversationConfig {
        provider: ProviderKind::Local,
        base_url: format!("http://{address}/v1"),
        timeout_seconds: 1,
        ..Default::default()
    };
    let provider = configured_provider(&config);
    let mut manager = ConversationManager::new(config, provider);
    manager.reset(11);
    manager.human_message("Good morning.", Target::Yuna, context(1), 0.0);
    let line = await_line(&mut manager, 11, 1);
    assert!(!line.text.is_empty());
    assert_eq!(manager.status, ProviderStatus::Fallback);
}

#[test]
fn side_pot_awards_do_not_hide_a_net_loss_from_social_state() {
    let mut manager = manager();
    let before = manager.mood(NpcId::Freya).clone();
    manager.observe_social(&crate::social::PublicSocialEvent::Settled {
        pot: 1200,
        awards: vec![1000, 0, 200, 0],
        shown: vec![true, false, true, false],
        net: vec![700, -50, -600, -50],
    });
    assert!(manager.mood(NpcId::Freya).confidence < before.confidence);
    assert!(manager.mood(NpcId::Freya).irritation > before.irritation);
}

#[test]
fn public_npc_exchange_has_one_interjection_then_ends() {
    let mut manager = manager();
    manager.offer_event(12, Speaker::Freya, 3, context(4), 0.0);
    assert_eq!(await_line(&mut manager, 11, 1).speaker, Seat::Jax);
    assert_eq!(await_line(&mut manager, 11, 1).speaker, Seat::Npc);
    assert!(manager.queued.is_none());
    assert_eq!(manager.history.len(), 2);
}
