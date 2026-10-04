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
    let config = match std::env::var("POKER_LAB_CONFIG") {
        Ok(path) => ConversationConfig::load(Some(std::path::Path::new(&path)))?,
        Err(_) => ConversationConfig::default(),
    };
    if config.provider == ProviderKind::Remote {
        return Err("This development probe uses local or mock providers only.".into());
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
        "Remember what happened yesterday?",
    ];
    for (npc, target) in NpcId::ALL
        .into_iter()
        .zip([Target::Ananya, Target::Freya, Target::Yuna])
    {
        println!("\n{}: {}", npc.name(), profile(npc).disposition);
        for (i, message) in messages.iter().enumerate() {
            manager.human_message(message, target, context.clone(), i as f32 * 20.0);
            let start = Instant::now();
            let mut line = None;
            while start.elapsed() < config.timeout() + Duration::from_millis(600) {
                if let Some(result) = manager.tick(1, context.hand) {
                    line = Some(result);
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            println!(
                "You: {message}\n{}: {} [{}]",
                npc.name(),
                line.map(|l| l.text).unwrap_or_else(|| "(silence)".into()),
                manager.label()
            );
        }
    }
    Ok(())
}
