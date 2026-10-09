//! Audition Stage 6 scenarios without graphics or a real save file.
//! POKER_LAB_CONFIG=config/conversation.example.json cargo run --example social_probe
use poker_lab::{
    conversation::*,
    memory::{MemoryService, NpcId, RememberingProvider},
    poker::{FourPlayerMatch, Seat},
    social::profile,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn main() -> Result<(), String> {
    let require_model = std::env::args().any(|a| a == "--require-model");
    let config = match std::env::var("POKER_LAB_CONFIG") {
        Ok(path) => ConversationConfig::load(Some(std::path::Path::new(&path)))?,
        Err(_) if require_model => ConversationConfig::for_application(
            None,
            std::path::Path::new("config/conversation.json"),
        )?,
        Err(_) => ConversationConfig::default(),
    };
    if config.provider == ProviderKind::Remote {
        return Err("This development probe uses local or mock providers only.".into());
    }
    if require_model && config.provider != ProviderKind::Local {
        return Err("--require-model requires local inference, not scripted mock output.".into());
    }
    println!(
        "Provider: {:?}. Mock/fallback output checks plumbing; judge natural dialogue only with a loaded local model.",
        config.provider
    );
    let memory = MemoryService::start(None);
    let provider = Arc::new(RememberingProvider {
        inner: configured_provider(&config),
        memory,
    });
    let mut manager = ConversationManager::new(config.clone(), provider);
    manager.reset(1);
    let mut game = FourPlayerMatch::new(42);
    game.start_next_hand().map_err(str::to_owned)?;
    let context = public_context(&game.observe(Seat::Human), &[]);
    let messages = [
        "Good morning.",
        "How are you today?",
        "Would you like to go out with me?",
        "You look beautiful today.",
        "You're terrible at poker.",
        "I'm sorry.",
        "Do you like music?",
        "I've been listening to jazz lately.",
        "What kind of music did I just mention?",
        "I'm exhausted from work; can we talk about something other than poker?",
        "Remember what happened yesterday?",
        "I'm feeling tired today.",
        "Remember what we talked about yesterday?",
    ];
    let mut generated = 0;
    let mut failures = 0;
    for (npc, target) in NpcId::ALL
        .into_iter()
        .zip([Target::Ananya, Target::Freya, Target::Yuna])
    {
        println!("\n{}: {}", npc.name(), profile(npc).disposition);
        for (i, message) in messages.iter().enumerate() {
            if !manager.human_message(message, target, context.clone(), i as f32 * 20.0) {
                return Err("probe message was not accepted".into());
            }
            let start = Instant::now();
            let mut line = None;
            while start.elapsed() < config.timeout() + Duration::from_millis(600) {
                if let Some(result) = manager.tick(1, context.hand) {
                    line = Some(result);
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if line
                .as_ref()
                .is_some_and(|l| l.source == poker_lab::characters::DialogueSource::Model)
            {
                generated += 1;
            } else {
                failures += 1;
            }
            println!(
                "You: {message}\n{}: {} [{}]",
                npc.name(),
                line.map(|l| format!("{}{}", l.text, l.source.badge()))
                    .unwrap_or_else(|| "(silence)".into()),
                manager.label()
            );
            println!(
                "  {:.2}s; diagnostic: {:?}",
                start.elapsed().as_secs_f64(),
                manager.diagnostics().last_issue
            );
        }
    }
    println!(
        "\nModel-generated replies: {generated}; scripted, silent or missing replies: {failures}."
    );
    println!("Host provenance totals: {:?}", manager.diagnostics());
    if require_model && failures > 0 {
        return Err("Local conversation acceptance probe had non-model or missing replies. Review output; do not count fallbacks as successful inference.".into());
    }
    Ok(())
}
