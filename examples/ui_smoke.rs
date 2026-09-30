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
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Poker Lab UI check".into(),
                resize_constraints: if std::env::args().any(|a| a == "--qhd") {
                    bevy::window::WindowResizeConstraints {
                        min_width: 2560.0,
                        min_height: 1440.0,
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
            type_chat(world, "Hello, Ananya");
        }
        6 => {
            let chat = world.resource::<ui::conversation::ConversationUi>();
            assert!(
                chat.manager
                    .history()
                    .iter()
                    .any(|m| m.text == "Hello, Ananya")
            );
            if !chat
                .manager
                .history()
                .iter()
                .any(|m| m.speaker == poker_lab::conversation::Speaker::Ananya)
            {
                return; // A current bubble gets its full reading time before the reply.
            }
            capture(world, "chat");
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
            click(world, "New Match");
        }
        13 => {
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
