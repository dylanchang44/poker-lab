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
        assert!(!request.contains("hole_cards") && !request.contains("CardBurned"));
        let result = serde_json::json!({"choices":[{"message":{"content":"{\"speaker\":\"freya\",\"dialogue\":\"A fine hand.\",\"expression\":\"happy\"}"}}]}).to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", result.len(), result).unwrap();
    });
    let config = ConversationConfig {
        provider: ProviderKind::Local,
        base_url: format!("http://{address}/v1"),
        ..Default::default()
    };
    let request = TurnRequest {
        session: 1,
        hand: 1,
        speaker: Speaker::Freya,
        kind: InteractionType::Reply,
        context: context(8),
        history: vec![],
        player_text: Some("Hello".into()),
        may_interject: false,
    };
    let raw = configured_provider(&config).respond(&request).unwrap();
    assert_eq!(
        parse_response(&raw, Speaker::Freya).unwrap().0,
        "A fine hand."
    );
    server.join().unwrap();
}
