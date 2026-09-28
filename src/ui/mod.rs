use bevy::prelude::*;

use crate::game::AppState;
mod controls;
mod table;

const BACKGROUND: Color = Color::srgb(0.035, 0.05, 0.08);
const FELT: Color = Color::srgb(0.055, 0.27, 0.22);
const FELT_EDGE: Color = Color::srgb(0.46, 0.34, 0.18);
const GOLD: Color = Color::srgb(0.9, 0.72, 0.39);
const TEXT: Color = Color::srgb(0.94, 0.93, 0.87);
const MUTED: Color = Color::srgb(0.63, 0.72, 0.69);
const BUTTON: Color = Color::srgb(0.12, 0.38, 0.31);
const BUTTON_HOVERED: Color = Color::srgb(0.18, 0.49, 0.39);
const BUTTON_PRESSED: Color = Color::srgb(0.09, 0.29, 0.24);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(BACKGROUND))
            .init_resource::<crate::game::TableMode>()
            .init_resource::<crate::game::NpcSettings>()
            .add_systems(Startup, setup_camera)
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(AppState::MainMenu), despawn_screen)
            .add_systems(OnEnter(AppState::InGame), crate::game::start_match)
            .add_systems(
                OnExit(AppState::InGame),
                (despawn_screen, crate::game::end_match),
            )
            .add_systems(Update, button_interactions)
            .add_systems(
                Update,
                (
                    controls::buttons,
                    controls::keyboard,
                    crate::game::npc_turn,
                    table::render,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Component)]
struct ScreenRoot;

#[derive(Component)]
struct NavigateTo(AppState);

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn screen_root() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(22.0),
        padding: UiRect::all(Val::Px(24.0)),
        ..default()
    }
}

fn spawn_main_menu(mut commands: Commands, mode: Res<crate::game::TableMode>) {
    commands
        .spawn((ScreenRoot, screen_root()))
        .with_children(|parent| {
            label(parent, "POKER LAB", 54.0, GOLD);
            label(
                parent,
                if *mode == crate::game::TableMode::Four {
                    "Mira / Jax / Nova   -   1,000 chips each / 5-10 blinds"
                } else {
                    "Heads-up practice / 1,000 chips / 5-10 blinds"
                },
                19.0,
                MUTED,
            );
            table(parent, "YOUR TABLE AWAITS");
            button(parent, "Start Game", AppState::InGame);
        });
}

fn label(parent: &mut ChildSpawnerCommands, value: impl Into<String>, size: f32, color: Color) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    ));
}

fn table(parent: &mut ChildSpawnerCommands, message: &'static str) {
    parent
        .spawn((
            Node {
                width: Val::Percent(82.0),
                max_width: Val::Px(680.0),
                height: Val::Px(275.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                margin: UiRect::vertical(Val::Px(22.0)),
                border: UiRect::all(Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(FELT),
            BorderColor::all(FELT_EDGE),
        ))
        .with_children(|table| label(table, message, 19.0, TEXT));
}

fn button(parent: &mut ChildSpawnerCommands, text: &'static str, destination: AppState) {
    parent
        .spawn((
            Button,
            NavigateTo(destination),
            Node {
                width: Val::Px(205.0),
                height: Val::Px(56.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(BUTTON),
        ))
        .with_children(|button| label(button, text, 22.0, TEXT));
}

type NavigationButtons<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static NavigateTo,
        &'static mut BackgroundColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

fn button_interactions(
    mut buttons: NavigationButtons,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, destination, mut background) in &mut buttons {
        *background = match interaction {
            Interaction::Pressed => {
                next_state.set(destination.0);
                BUTTON_PRESSED.into()
            }
            Interaction::Hovered => BUTTON_HOVERED.into(),
            Interaction::None => BUTTON.into(),
        };
    }
}

fn despawn_screen(mut commands: Commands, screens: Query<Entity, With<ScreenRoot>>) {
    for screen in &screens {
        commands.entity(screen).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameSession, MatchSeed};
    use controls::Control;
    use poker_lab::poker::{Action, Phase, Seat};

    fn test_app() -> App {
        test_app_mode(crate::game::TableMode::HeadsUp)
    }

    fn test_app_mode(mode: crate::game::TableMode) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            bevy::input::InputPlugin,
        ))
        .insert_resource(MatchSeed(Some(42)))
        .insert_resource(mode)
        .insert_resource(crate::game::NpcSettings {
            delay_seconds: 0.85,
            equity_samples: 16,
        })
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ))
        .init_state::<AppState>()
        .add_plugins(UiPlugin);
        app.update();
        let mut query = app.world_mut().query::<(Entity, &NavigateTo)>();
        let start = query
            .iter(app.world())
            .find(|(_, n)| n.0 == AppState::InGame)
            .unwrap()
            .0;
        *app.world_mut().get_mut::<Interaction>(start).unwrap() = Interaction::Pressed;
        app.update();
        app.update();
        app
    }

    fn press(app: &mut App, control: Control) {
        let mut query = app.world_mut().query::<(Entity, &Control)>();
        let entity = query
            .iter(app.world())
            .find(|(_, c)| **c == control)
            .expect("enabled control exists")
            .0;
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
        app.update();
    }

    #[test]
    fn four_player_ui_background_turns_elimination_and_restart() {
        let mut app = test_app_mode(crate::game::TableMode::Four);
        for name in ["Mira", "Jax", "Nova"] {
            assert!(
                app.world_mut()
                    .query::<&Text>()
                    .iter(app.world())
                    .any(|t| t.0.starts_with(name))
            );
        }
        let mut saw_npc = [false; 4];
        let mut saw_elimination = false;
        for step in 0..15000 {
            let v = app
                .world()
                .resource::<GameSession>()
                .engine
                .observe(Seat::Human);
            assert_eq!(v.stacks.iter().sum::<u32>() + v.pot, 4000);
            saw_elimination |= v.eliminated.iter().any(|x| *x);
            if v.phase == Phase::MatchComplete {
                break;
            }
            assert!(step < 14999, "background NPC/UI match failed to finish");
            if v.phase == Phase::HandComplete {
                press(&mut app, Control::NextHand);
            } else if v.actor == Some(Seat::Human) {
                let action = if v.legal.all_in {
                    Action::AllIn
                } else if v.legal.call.is_some() {
                    Action::Call
                } else {
                    Action::Check
                };
                press(&mut app, Control::Act(action));
            } else {
                saw_npc[v.actor.unwrap().index()] = true;
                app.update();
                // Let the real worker run; game-time still advances deterministically.
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        assert!(saw_npc[1..].iter().all(|x| *x));
        assert!(saw_elimination);
        press(&mut app, Control::NewMatch);
        let v = app
            .world()
            .resource::<GameSession>()
            .engine
            .observe(Seat::Human);
        assert_eq!(v.hand_number, 1);
        assert_eq!(v.stacks, [1000, 995, 990, 1000]);
        press(&mut app, Control::Menu);
        app.update();
        assert!(!app.world().contains_resource::<GameSession>());
    }

    #[test]
    fn ui_starts_match_delays_npc_and_cleans_up_on_menu() {
        let mut app = test_app();
        assert_eq!(
            app.world().resource::<GameSession>().engine.actor(),
            Some(Seat::Human)
        );
        press(&mut app, Control::Act(Action::Call));
        assert_eq!(
            app.world().resource::<GameSession>().engine.actor(),
            Some(Seat::Npc)
        );
        let events = app.world().resource::<GameSession>().engine.history().len();
        app.update();
        assert_eq!(
            app.world().resource::<GameSession>().engine.history().len(),
            events
        );
        for _ in 0..9 {
            app.update();
        }
        assert!(app.world().resource::<GameSession>().engine.history().len() > events);
        press(&mut app, Control::Menu);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
        assert!(!app.world().contains_resource::<GameSession>());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ScreenRoot>>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn numeric_input_gates_wagers_and_submits_street_total() {
        use bevy::input::{
            ButtonState,
            keyboard::{Key, KeyboardInput},
        };
        fn key(app: &mut App, key: Key) {
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Digit1,
                logical_key: key,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
        }
        let mut app = test_app();
        press(&mut app, Control::EditAmount);
        key(&mut app, Key::Character("1".into()));
        assert_eq!(app.world().resource::<GameSession>().bet_input, "1");
        assert!(
            !app.world_mut()
                .query::<&Control>()
                .iter(app.world())
                .any(|c| *c == Control::Wager)
        );
        key(&mut app, Key::Enter);
        assert_eq!(app.world().resource::<GameSession>().bet_input, "20");
        press(&mut app, Control::EditAmount);
        key(&mut app, Key::Character("100".into()));
        key(&mut app, Key::Enter);
        press(&mut app, Control::Wager);
        assert_eq!(
            app.world()
                .resource::<GameSession>()
                .engine
                .observe(Seat::Human)
                .street_bets[0],
            100
        );
        assert_eq!(
            app.world().resource::<GameSession>().engine.stacks()[0],
            900
        );
    }

    #[test]
    fn ui_can_finish_match_advance_hands_and_restart() {
        let mut app = test_app();
        for step in 0..2000 {
            let view = app
                .world()
                .resource::<GameSession>()
                .engine
                .observe(Seat::Human);
            if view.phase == Phase::MatchComplete {
                break;
            }
            assert!(step < 1999, "UI match did not finish");
            if view.phase == Phase::HandComplete {
                press(&mut app, Control::NextHand);
            } else if view.actor == Some(Seat::Human) {
                let action = if view.legal.all_in {
                    Action::AllIn
                } else if view.legal.call.is_some() {
                    Action::Call
                } else {
                    Action::Check
                };
                press(&mut app, Control::Act(action));
            } else {
                app.update();
            }
        }
        assert_eq!(
            app.world().resource::<GameSession>().engine.phase(),
            Phase::MatchComplete
        );
        press(&mut app, Control::NewMatch);
        let view = app
            .world()
            .resource::<GameSession>()
            .engine
            .observe(Seat::Human);
        assert_eq!(view.hand_number, 1);
        assert_eq!(view.stacks, [995, 990]);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ScreenRoot>>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn buttons_navigate_in_both_directions() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .add_systems(Update, button_interactions);

        for (destination, expected) in [
            (AppState::InGame, AppState::InGame),
            (AppState::MainMenu, AppState::MainMenu),
        ] {
            let button = app
                .world_mut()
                .spawn((
                    Button,
                    NavigateTo(destination),
                    Interaction::Pressed,
                    BackgroundColor(BUTTON),
                ))
                .id();

            app.update(); // The button system requests the next state.
            app.update(); // Bevy applies that request before the next Update.
            assert_eq!(*app.world().resource::<State<AppState>>().get(), expected);
            app.world_mut().despawn(button);
        }
    }
}
