mod game;
mod ui;

use bevy::prelude::*;

fn main() {
    let memory_path = poker_lab::memory::database_path()
        .map_err(|error| eprintln!("Memory configuration: {error}; using temporary storage."))
        .ok();
    let explicit_config = std::env::var_os("POKER_LAB_CONFIG").map(std::path::PathBuf::from);
    let conversation_config = poker_lab::conversation::ConversationConfig::for_application(
        explicit_config.as_deref(),
        std::path::Path::new("config/conversation.json"),
    )
    .unwrap_or_else(|_| {
        eprintln!("Conversation configuration invalid; using visibly labelled scripted mock dialogue. Check POKER_LAB_CONFIG or config/conversation.json.");
        poker_lab::conversation::ConversationConfig::default()
    });
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let mode = if let Some(index) = args.iter().position(|arg| arg == "--heads-up") {
        args.remove(index);
        game::TableMode::HeadsUp
    } else {
        game::TableMode::Four
    };
    let seed = match args.as_slice() {
        [] => None,
        [flag, value] if flag == "--seed" => Some(value.parse::<u64>().unwrap_or_else(|_| {
            eprintln!("--seed must be an unsigned 64-bit integer");
            std::process::exit(2);
        })),
        _ => {
            eprintln!("Usage: poker-lab [--seed NUMBER] [--heads-up]");
            std::process::exit(2);
        }
    };
    App::new()
        .insert_resource(ui::memory::MemorySettings(memory_path))
        .insert_resource(ui::conversation::ConversationSettings(conversation_config))
        .insert_resource(game::MatchSeed(seed))
        .insert_resource(mode)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Poker Lab".into(),
                resolution: (1280, 960).into(),
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: 1000.0,
                    min_height: 820.0,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }))
        .init_state::<game::AppState>()
        .add_plugins(ui::UiPlugin)
        .run();
}
