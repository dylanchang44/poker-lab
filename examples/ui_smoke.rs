//! Optional graphical check: cargo run --example ui_smoke
//! Drives Bevy's pointer hit testing (not OS input), saves /tmp/poker-lab-*.png,
//! then exits. Requires a working desktop and GPU.
#[path = "../src/game/mod.rs"]
mod game;
#[path = "../src/ui/mod.rs"]
mod ui;

use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyCode, KeyboardInput},
    },
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    ui::UiSystems,
};
use poker_lab::poker::{Phase, Seat};

#[derive(Resource, Default)]
struct Progress {
    frame: u32,
    next: u32,
    step: u32,
    expected_amount: String,
    chat_case: u8,
}

fn main() {
    let mut config = if std::env::args().any(|a| a == "--local") {
        poker_lab::conversation::ConversationConfig::for_application(
            std::env::var_os("POKER_LAB_CONFIG")
                .map(std::path::PathBuf::from)
                .as_deref(),
            std::path::Path::new("config/conversation.json"),
        )
        .expect("valid local conversation configuration")
    } else {
        poker_lab::conversation::ConversationConfig::default()
    };
    // Isolate the submitted human turn from unsolicited speech during the check.
    config.initiative_frequency = 0.0;
    if std::env::args().any(|a| a == "--long-chat") {
        config.max_dialogue_chars = 360;
    }
    App::new()
        .insert_resource(ui::conversation::ConversationSettings(config))
        // Explicit opt-in: ordinary smoke checks never touch the user's database.
        .insert_resource(ui::memory::MemorySettings(
            std::env::var_os("POKER_LAB_SMOKE_DB").map(std::path::PathBuf::from),
        ))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Poker Lab UI check".into(),
                resize_constraints: if std::env::args().any(|a| a == "--qhd" || a == "--hd") {
                    let qhd = std::env::args().any(|a| a == "--qhd");
                    bevy::window::WindowResizeConstraints {
                        min_width: if qhd { 2560.0 } else { 1920.0 },
                        min_height: if qhd { 1440.0 } else { 1080.0 },
                        ..default()
                    }
                } else {
                    default()
                },
                resolution: if std::env::args().any(|a| a == "--hd") {
                    (1920, 1080).into()
                } else if std::env::args().any(|a| a == "--qhd") {
                    (2560, 1440).into()
                } else if std::env::args().any(|a| a == "--small") {
                    (1000, 820).into()
                } else {
                    (1120, 860).into()
                },
                ..default()
            }),
            ..default()
        }))
        .insert_resource(game::MatchSeed(Some(42)))
        .init_resource::<Progress>()
        .init_state::<game::AppState>()
        .add_plugins(ui::UiPlugin)
        .add_systems(Startup, configure_chat_fixture)
        .add_systems(
            PreUpdate,
            drive.after(InputSystems).before(UiSystems::Focus),
        )
        .run();
}

fn capture(world: &mut World, name: &str) {
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("/tmp/poker-lab-{name}.png")));
}

fn click(world: &mut World, label: &str) {
    let parent = world
        .query::<(&Text, &ChildOf)>()
        .iter(world)
        .find(|(t, parent)| t.0 == label && world.get::<Button>(parent.parent()).is_some())
        .unwrap_or_else(|| panic!("missing button label {label}"))
        .1
        .parent();
    assert!(
        world.get::<Button>(parent).is_some(),
        "button {label} is disabled"
    );
    let position = world.get::<UiGlobalTransform>(parent).unwrap().translation;
    let size = world.get::<ComputedNode>(parent).unwrap().size();
    let mut window = world.query::<&mut Window>().single_mut(world).unwrap();
    assert!(
        position.x - size.x / 2.0 >= 0.0
            && position.x + size.x / 2.0 <= window.physical_width() as f32
    );
    assert!(
        position.y - size.y / 2.0 >= 0.0
            && position.y + size.y / 2.0 <= window.physical_height() as f32
    );
    // This input belongs to the test, not the OS pointer. Avoid asking Winit to
    // warp a Wayland cursor when it observes a changed Window component.
    window
        .bypass_change_detection()
        .set_physical_cursor_position(Some(position.as_dvec2()));
    world
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    println!("Click {label} at {position}");
}

fn type_chat(world: &mut World, text: &str) {
    let window = world
        .query_filtered::<Entity, With<Window>>()
        .single(world)
        .unwrap();
    world.write_message(KeyboardInput {
        key_code: KeyCode::KeyH,
        logical_key: Key::Character(text.into()),
        state: ButtonState::Pressed,
        text: Some(text.into()),
        repeat: false,
        window,
    });
    world.write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: Key::Enter,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
}

fn drive(world: &mut World) {
    world
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    let mut progress = world.resource_mut::<Progress>();
    progress.frame += 1;
    assert!(progress.frame < 6000, "graphical smoke check timed out");
    if progress.frame < 90 || progress.frame < progress.next {
        return;
    }
    progress.next = progress.frame + 90;
    let step = progress.step;
    match step {
        0 => {
            capture(world, "menu");
        }
        1 => {
            click(world, "Start Game");
        }
        2 => {
            assert_eq!(
                *world.resource::<State<game::AppState>>().get(),
                game::AppState::InGame
            );
            capture(world, "table");
            let window = world.query::<&Window>().single(world).unwrap();
            println!(
                "Layout: {}x{} logical, {}x{} physical",
                window.width(),
                window.height(),
                window.physical_width(),
                window.physical_height()
            );
            let images = world.resource::<Assets<Image>>();
            let portraits = world.resource::<ui::characters::PortraitAssets>();
            for handle in &portraits.sheets {
                assert!(images.contains(handle.id()), "portrait must load");
            }
            for name in ["Ananya", "Freya", "Yuna"] {
                assert!(
                    world
                        .query::<&Text>()
                        .iter(world)
                        .any(|t| t.0.starts_with(name))
                );
            }
        }
        3 => {
            click(world, "Everyone");
        }
        4 => {
            assert_eq!(
                world.resource::<ui::conversation::ConversationUi>().target,
                poker_lab::conversation::Target::Ananya
            );
            click(world, "Click to chat");
        }
        5 => {
            assert!(world.resource::<ui::conversation::ConversationUi>().editing);
            type_chat(world, "I enjoy strategy games");
        }
        6 => {
            let chat = world.resource::<ui::conversation::ConversationUi>();
            assert!(
                chat.manager
                    .history()
                    .iter()
                    .any(|m| m.text == "I enjoy strategy games")
            );
            if !chat
                .manager
                .history()
                .iter()
                .any(|m| m.speaker == poker_lab::conversation::Speaker::Ananya)
            {
                return; // A current bubble gets its full reading time before the reply.
            }
            if std::env::args().any(|a| a == "--local") {
                assert!(
                    chat.manager
                        .history()
                        .iter()
                        .any(|m| m.speaker == poker_lab::conversation::Speaker::Ananya
                            && m.source == Some(poker_lab::characters::DialogueSource::Model)),
                    "local GUI check received a scripted fallback, not a model reply"
                );
                println!("Verified local model reply through Bevy chat input/presentation.");
                assert_eq!(chat.manager.label(), "Local model connected");
                assert_eq!(chat.manager.diagnostics().fallbacks, 0);
            } else {
                assert!(
                    chat.manager
                        .history()
                        .iter()
                        .any(|m| m.speaker == poker_lab::conversation::Speaker::Ananya
                            && m.source == Some(poker_lab::characters::DialogueSource::Scripted))
                );
            }
            if std::env::args().any(|a| a == "--long-chat") {
                if !long_chat_cases(world) {
                    return;
                }
            } else {
                capture(world, "chat");
            }
        }
        7 => {
            let view = world
                .resource::<game::GameSession>()
                .engine
                .observe(Seat::Human);
            if view.actor != Some(Seat::Human) {
                return;
            }
            let range = view.legal.wager.expect("seeded opening offers raise");
            world.resource_mut::<Progress>().expected_amount =
                (view.street_bets[0] + view.to_call + view.pot + view.to_call)
                    .clamp(range.min_to, range.max_to)
                    .to_string();
            click(world, "Pot");
        }
        8 => {
            assert_eq!(
                world.resource::<game::GameSession>().bet_input,
                world.resource::<Progress>().expected_amount
            );
            click(world, "Raise");
        }
        9 => {
            let view = world
                .resource::<game::GameSession>()
                .engine
                .observe(Seat::Human);
            if view.phase == Phase::HandComplete {
                capture(world, "result");
            } else {
                if view.actor == Some(Seat::Human) {
                    let label = if let Some(call) = view.legal.call {
                        format!("Call {call}")
                    } else {
                        "Check".into()
                    };
                    click(world, &label);
                    if !view.board.is_empty() {
                        capture(world, "board");
                    }
                }
                return;
            }
        }
        10 => {
            click(world, "Next Hand");
        }
        11 => {
            let view = world
                .resource::<game::GameSession>()
                .engine
                .observe(Seat::Human);
            assert_eq!(view.hand_number, 2);
            assert_eq!(view.dealer, Seat::Npc);
            capture(world, "next-hand");
        }
        12 => {
            click(world, "Memories");
        }
        13 => {
            let service = &world.resource::<ui::memory::MemoryUi>().service;
            assert!(service.flush());
            let snapshot = service.snapshot();
            let anya = &snapshot.characters[0];
            assert!(
                anya.memories
                    .iter()
                    .any(|m| m.summary.contains("strategy games"))
            );
            assert!(snapshot.characters[1..].iter().all(|c| {
                !c.memories
                    .iter()
                    .any(|m| m.summary.contains("strategy games"))
            }));
            if std::env::args().any(|a| a == "--restored") {
                assert!(
                    snapshot.opponent.as_ref().unwrap().hands >= 2,
                    "opponent learning must survive application restart"
                );
                assert!(
                    anya.sessions >= 3,
                    "prior application sessions must be restored"
                );
                assert!(
                    anya.memories
                        .iter()
                        .any(|m| m.summary.contains("strategy games") && m.occurrences >= 2)
                );
            }
            capture(world, "memory");
            click(world, "Next character");
        }
        14 => {
            capture(world, "memory-freya");
            click(world, "Close memories");
        }
        15 => {
            click(world, "Poker reads");
        }
        16 => {
            let text = world
                .query_filtered::<&Text, With<ui::learning::LearningText>>()
                .single(world)
                .unwrap();
            assert!(text.0.contains("Effective preflop") && text.0.contains("completed hands"));
            capture(world, "strategy-read");
            click(world, "Metrics / strategy");
        }
        17 => {
            let text = world
                .query_filtered::<&Text, With<ui::learning::LearningText>>()
                .single(world)
                .unwrap();
            assert!(text.0.contains("VPIP") && text.0.contains("Fold to 3-bet"));
            assert!(world.resource::<ui::learning::LearningUi>().model.hands >= 1);
            capture(world, "strategy-metrics");
            click(world, "Close reads");
        }
        18 => {
            click(world, "New Match");
        }
        19 => {
            assert_eq!(
                world
                    .resource::<game::GameSession>()
                    .engine
                    .observe(Seat::Human)
                    .hand_number,
                1
            );
            click(world, "Back to Menu");
        }
        _ => {
            assert_eq!(
                *world.resource::<State<game::AppState>>().get(),
                game::AppState::MainMenu
            );
            println!(
                "Graphical smoke check passed: four seats, chat target/input/response, menu, pot sizing, raise, hand result, next hand, restart, menu return."
            );
            world.write_message(AppExit::Success);
        }
    }
    world.resource_mut::<Progress>().step += 1;
}

struct LongReply;
impl poker_lab::conversation::DialogueProvider for LongReply {
    fn respond(&self, r: &poker_lab::conversation::TurnRequest) -> Result<String, String> {
        if r.player_text.as_deref() == Some("force-fallback") {
            return Err("offline test".into());
        }
        let length = if r.player_text.as_deref() == Some("public wrap test") {
            180
        } else {
            360
        };
        let body:String="You're playing much more aggressively than when we started, although I'm still deciding whether that's confidence or impatience. ".repeat(4).chars().take(length-4).collect();
        Ok(serde_json::json!({"speaker":r.speaker.id(),"dialogue":format!("{body} END"),"expression":"neutral","conversation_continuation":false}).to_string())
    }
}
fn configure_chat_fixture(mut chat: ResMut<ui::conversation::ConversationUi>) {
    if std::env::args().any(|a| a == "--long-chat") {
        let config = poker_lab::conversation::ConversationConfig {
            max_dialogue_chars: 360,
            initiative_frequency: 0.0,
            ..default()
        };
        chat.manager = poker_lab::conversation::ConversationManager::new(
            config,
            std::sync::Arc::new(LongReply),
        );
    }
}
#[derive(Component)]
struct MeasureLatest;
fn long_chat_cases(world: &mut World) -> bool {
    use poker_lab::{
        characters::DialogueSource,
        conversation::{Speaker, Target},
    };
    let case = world.resource::<Progress>().chat_case;
    match case {
        0 => {
            let text = world
                .query_filtered::<&Text, With<ui::conversation::ChatHistory>>()
                .single(world)
                .unwrap()
                .0
                .clone();
            let last = world
                .resource::<ui::conversation::ConversationUi>()
                .manager
                .history()
                .back()
                .unwrap()
                .clone();
            assert_eq!(last.text.chars().count(), 360);
            assert!(text.contains(&last.text));
            assert!(text.contains("[private]"));
            assert!(text.contains("[scripted]"));
            let width = world
                .query_filtered::<&ComputedNode, With<ui::conversation::ChatViewport>>()
                .single(world)
                .unwrap();
            let width = width.size().x * width.inverse_scale_factor();
            world.spawn((
                MeasureLatest,
                Text::new(format!("Ananya [scripted] [private]:\n{}", last.text)),
                TextLayout::new_with_linebreak(bevy::text::LineBreak::WordOrCharacter),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-10000.0),
                    width: Val::Px(width),
                    ..default()
                },
            ));
        }
        1 => {
            let measured = world
                .query_filtered::<&ComputedNode, With<MeasureLatest>>()
                .single(world)
                .unwrap()
                .size();
            let (viewport,scroll)=world.query_filtered::<(&ComputedNode,&ScrollPosition),With<ui::conversation::ChatViewport>>().single(world).unwrap();
            assert!(
                measured.y <= viewport.size().y + 2.0,
                "maximum reply must fit completely in the newest viewport: {:?} vs {:?}",
                measured,
                viewport.size()
            );
            let bottom = (viewport.content_size().y - viewport.size().y).max(0.0)
                * viewport.inverse_scale_factor();
            assert!((scroll.y - bottom).abs() < 2.0);
            println!(
                "Chat scroll: viewport {:?}, content {:?}, scroll {:?}",
                viewport.size(),
                viewport.content_size(),
                scroll
            );
            let transcript = layout_rect::<ui::conversation::ChatHistory>(world);
            let viewport = layout_rect::<ui::conversation::ChatViewport>(world);
            let text_layout = world
                .query_filtered::<&bevy::text::TextLayoutInfo, With<ui::conversation::ChatHistory>>(
                )
                .single(world)
                .unwrap();
            assert!(
                text_layout.size.y * text_layout.scale_factor <= transcript.height() + 2.0,
                "transcript node must contain all rendered glyphs: glyphs {:?} at scale {}, node {:?}",
                text_layout.size,
                text_layout.scale_factor,
                transcript
            );
            assert!(
                transcript.max.y <= viewport.max.y + 2.0,
                "latest transcript line must actually be visible: {transcript:?} vs {viewport:?}"
            );
            assert_chat_geometry(world);
            capture(world, "chat-max-360");
            // Full input is rendered, including both endpoints, not an 18-character tail.
            let mut chat = world.resource_mut::<ui::conversation::ConversationUi>();
            chat.editing = true;
            chat.input = format!(
                "BEGIN{} END",
                " sentence".repeat(30).chars().take(231).collect::<String>()
            );
            assert_eq!(chat.input.chars().count(), 240);
        }
        2 => {
            let typed = world
                .resource::<ui::conversation::ConversationUi>()
                .input
                .clone();
            let (text, size) = world
                .query_filtered::<(&Text, &ComputedNode), With<ui::conversation::ChatInput>>()
                .single(world)
                .unwrap();
            assert!(
                text.0.starts_with("BEGIN") && text.0.contains(&typed) && text.0.ends_with("END│")
            );
            assert!(size.size().y < 64.0 / size.inverse_scale_factor());
            capture(world, "chat-input-240");
        }
        3 => {
            queue_chat_test(world, "public wrap test", Target::Table);
        }
        4 => {
            let chat = world.resource::<ui::conversation::ConversationUi>();
            let last = chat.manager.history().back().unwrap();
            if last.speaker == Speaker::Human {
                return false;
            }
            assert_eq!(last.text.chars().count(), 180);
            assert_eq!(last.audience, poker_lab::memory::Audience::Public);
            let text = world
                .query_filtered::<&Text, With<ui::conversation::ChatHistory>>()
                .single(world)
                .unwrap();
            assert!(
                text.0.contains("[private]")
                    && text.0.contains("public wrap test")
                    && text.0.matches(" END").count() >= 2
            );
            capture(world, "chat-multiple-wrapped");
        }
        5 => {
            queue_chat_test(world, "force-fallback", Target::Yuna);
        }
        6 => {
            let last = world
                .resource::<ui::conversation::ConversationUi>()
                .manager
                .history()
                .back()
                .unwrap();
            if last.speaker == Speaker::Human {
                return false;
            }
            assert_eq!(last.source, Some(DialogueSource::Fallback));
            let text = world
                .query_filtered::<&Text, With<ui::conversation::ChatHistory>>()
                .single(world)
                .unwrap();
            assert!(text.0.contains("[scripted fallback]"));
            capture(world, "chat-fallback");
        }
        7 => {
            click(world, "Older");
        }
        8 => {
            assert!(
                !world
                    .resource::<ui::conversation::ConversationUi>()
                    .follow_latest
            );
            click(world, "Newer");
        }
        9 => {
            click(world, "Latest");
        }
        10 => {
            let viewport = layout_rect::<ui::conversation::ChatViewport>(world);
            world
                .query::<&mut Window>()
                .single_mut(world)
                .unwrap()
                .bypass_change_detection()
                .set_physical_cursor_position(Some(viewport.center().as_dvec2()));
        }
        11 => {
            let window = world
                .query_filtered::<Entity, With<Window>>()
                .single(world)
                .unwrap();
            world.write_message(bevy::input::mouse::MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: 3.0,
                window,
            });
        }
        12 => {
            assert!(
                !world
                    .resource::<ui::conversation::ConversationUi>()
                    .follow_latest,
                "mouse wheel over the transcript must scroll history"
            );
            click(world, "Latest");
        }
        _ => {
            assert!(
                world
                    .resource::<ui::conversation::ConversationUi>()
                    .follow_latest
            );
            println!(
                "Long chat passed: full 360-character reply, 240-character input, public/private, fallback badge, wrapped history and scrolling."
            );
            return true;
        }
    }
    world.resource_mut::<Progress>().chat_case += 1;
    false
}
fn assert_chat_geometry(world: &mut World) {
    let window = world.query::<&Window>().single(world).unwrap();
    let screen = Vec2::new(
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let canvas = layout_rect::<ui::table::TableCanvas>(world);
    let felt = layout_rect::<ui::table::TableFelt>(world);
    let human = layout_rect::<ui::table::HumanSeat>(world);
    let bubble = layout_rect::<ui::characters::DialoguePanel>(world);
    let chat = layout_rect::<ui::conversation::ChatPanel>(world);
    let actions = layout_rect::<ui::table::ActionPanel>(world);
    // Nested percentage layouts round each edge to physical pixels.
    assert!((felt.center().x - canvas.center().x).abs() <= 2.0);
    assert!((human.center().x - canvas.center().x).abs() <= 2.0);
    assert!(felt.width() >= canvas.width() * 0.9);
    assert!(
        bubble.max.y <= human.min.y,
        "speech must not cover human cards: bubble {bubble:?}, human {human:?}"
    );
    assert!(
        chat.max.x < actions.min.x,
        "chat and controls must not overlap"
    );
    assert!(human.max.y <= chat.min.y && human.max.y <= actions.min.y);
    for rect in [felt, human, bubble, chat, actions] {
        assert!(rect.min.cmpge(Vec2::ZERO).all() && rect.max.cmple(screen + Vec2::ONE).all());
    }
    // Validate actual descendants too: an on-screen panel can still contain
    // an overflowing row of buttons when its minimum content width is too big.
    let panel = world
        .query_filtered::<Entity, With<ui::table::ActionPanel>>()
        .single(world)
        .unwrap();
    for (entity, node, position) in world
        .query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform), With<Button>>()
        .iter(world)
    {
        let mut ancestor = entity;
        while let Some(parent) = world.get::<ChildOf>(ancestor) {
            ancestor = parent.parent();
            if ancestor == panel {
                let rect = Rect::from_center_size(position.translation, node.size());
                assert!(
                    rect.min.cmpge(actions.min - Vec2::ONE).all()
                        && rect.max.cmple(actions.max + Vec2::ONE).all(),
                    "action button must fit within its panel"
                );
                break;
            }
        }
    }
}

fn layout_rect<T: Component>(world: &mut World) -> Rect {
    let (node, position) = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<T>>()
        .single(world)
        .unwrap();
    Rect::from_center_size(position.translation, node.size())
}

fn queue_chat_test(world: &mut World, text: &str, target: poker_lab::conversation::Target) {
    let context = poker_lab::conversation::public_context(
        &world
            .resource::<game::GameSession>()
            .engine
            .observe(Seat::Human),
        &[],
    );
    world
        .resource_mut::<game::GameSession>()
        .presentation
        .dialogue = None;
    let mut chat = world.resource_mut::<ui::conversation::ConversationUi>();
    chat.input.clear();
    chat.editing = false;
    chat.manager.human_message(text, target, context, 100.0);
}
