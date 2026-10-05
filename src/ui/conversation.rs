use super::{
    BUTTON, BUTTON_HOVERED, BUTTON_PRESSED, GOLD, MUTED, TEXT, characters::DialogueRequest, label,
};
use crate::game::GameSession;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use poker_lab::{
    conversation::{
        ConversationConfig, ConversationManager, Target, configured_provider, public_context,
    },
    poker::Seat,
};

#[derive(Resource, Clone, Default)]
pub struct ConversationSettings(pub ConversationConfig);

#[derive(Resource)]
pub struct ConversationUi {
    pub manager: ConversationManager,
    pub input: String,
    pub editing: bool,
    pub target: Target,
    clock: f32,
    event_cursor: usize,
    social_cursor: usize,
}
impl FromWorld for ConversationUi {
    fn from_world(world: &mut World) -> Self {
        let config = world
            .get_resource::<ConversationSettings>()
            .map(|r| r.0.clone())
            .unwrap_or_default();
        let provider = if let Some(memory) = world.get_resource::<super::memory::MemoryUi>() {
            std::sync::Arc::new(poker_lab::memory::RememberingProvider {
                inner: configured_provider(&config),
                memory: memory.service.clone(),
            }) as std::sync::Arc<dyn poker_lab::conversation::DialogueProvider>
        } else {
            configured_provider(&config)
        };
        Self {
            manager: ConversationManager::new(config, provider),
            input: String::new(),
            editing: false,
            target: Target::Table,
            clock: 0.0,
            event_cursor: 0,
            social_cursor: 0,
        }
    }
}

#[derive(Component)]
pub struct ChatHistory;
#[derive(Component)]
pub struct ChatInput;
#[derive(Component)]
pub struct ChatTarget;
#[derive(Component)]
pub struct ChatStatus;
#[derive(Component, Clone, Copy)]
pub enum ChatControl {
    Target,
    Input,
    Send,
}

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(670.0),
                width: Val::Px(250.0),
                height: Val::Px(266.0),
                padding: UiRect::all(Val::Px(9.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.055, 0.068, 0.09)),
            BorderColor::all(GOLD.with_alpha(0.55)),
            ZIndex(6),
        ))
        .with_children(|panel| {
            label(panel, "TABLE TALK", 15.0, GOLD);
            panel.spawn((
                ChatStatus,
                Text::new("Checking dialogue mode…"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(MUTED),
            ));
            panel.spawn((
                ChatHistory,
                Text::new("Say something to the table."),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(TEXT),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(142.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ));
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|row| {
                    chat_button(row, ChatControl::Target, 95.0, ChatTarget, "Everyone");
                    chat_button(row, ChatControl::Input, 133.0, ChatInput, "Click to chat");
                });
            chat_button(panel, ChatControl::Send, 228.0, (), "Send / Enter");
        });
}
fn chat_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    kind: ChatControl,
    width: f32,
    marker: B,
    caption: &str,
) {
    parent
        .spawn((
            Button,
            kind,
            Node {
                width: Val::Px(width),
                height: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(BUTTON),
        ))
        .with_children(|button| {
            button.spawn((
                marker,
                Text::new(caption),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(TEXT),
            ));
        });
}

type ChatButtons<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static ChatControl,
        &'static mut BackgroundColor,
    ),
    (Changed<Interaction>, With<Button>),
>;
pub fn buttons(
    mut buttons: ChatButtons,
    mut chat: ResMut<ConversationUi>,
    mut session: ResMut<GameSession>,
) {
    for (interaction, control, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::None => BUTTON.into(),
            Interaction::Hovered => BUTTON_HOVERED.into(),
            Interaction::Pressed => BUTTON_PRESSED.into(),
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        match control {
            ChatControl::Target => {
                let current = Target::ALL
                    .iter()
                    .position(|t| *t == chat.target)
                    .unwrap_or(0);
                let count = if session.mode == crate::game::TableMode::HeadsUp {
                    2
                } else {
                    Target::ALL.len()
                };
                chat.target = Target::ALL[(current + 1) % count];
            }
            ChatControl::Input => {
                chat.editing = true;
                session.editing = false;
            }
            ChatControl::Send => send(&mut chat, &session),
        }
    }
}
pub fn keyboard(
    mut events: MessageReader<KeyboardInput>,
    mut chat: ResMut<ConversationUi>,
    session: Res<GameSession>,
) {
    for event in events.read() {
        if !chat.editing || event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Character(value) => {
                for c in value.chars().filter(|c| !c.is_control()) {
                    if chat.input.chars().count() < 240 {
                        chat.input.push(c);
                    }
                }
            }
            Key::Space => {
                if chat.input.chars().count() < 240 {
                    chat.input.push(' ');
                }
            }
            Key::Backspace => {
                chat.input.pop();
            }
            Key::Enter => send(&mut chat, &session),
            Key::Escape => chat.editing = false,
            _ => {}
        }
    }
}
fn send(chat: &mut ConversationUi, session: &GameSession) {
    let view = session.engine.observe(Seat::Human);
    let winners: Vec<_> = session.public_winners.iter().cloned().collect();
    let context = public_context(&view, &winners);
    if chat
        .manager
        .human_message(&chat.input, chat.target, context, chat.clock)
    {
        chat.input.clear();
        chat.editing = false;
    }
}
pub fn update(
    time: Res<Time>,
    mut chat: ResMut<ConversationUi>,
    mut session: ResMut<GameSession>,
    mut output: MessageWriter<DialogueRequest>,
) {
    chat.clock += time.delta_secs();
    if chat.manager.session() != session.presentation.session {
        let now = chat.clock;
        chat.manager.reset_at(session.presentation.session, now);
        chat.event_cursor = 0;
        chat.social_cursor = 0;
        chat.input.clear();
        chat.editing = false;
    }
    session.presentation.suppress_samples = chat.manager.enabled();
    chat.manager.advance(time.delta_secs());
    for (id, event) in &session.social_events {
        if *id > chat.social_cursor {
            chat.manager.observe_social(event);
            chat.social_cursor = *id;
        }
    }
    let view = session.engine.observe(Seat::Human);
    let winners: Vec<_> = session.public_winners.iter().cloned().collect();
    let context = public_context(&view, &winners);
    for cue in session.conversation_cues.iter() {
        if cue.event_id <= chat.event_cursor {
            continue;
        }
        let now = chat.clock;
        chat.manager.offer_event(
            cue.event_id,
            cue.speaker,
            cue.priority,
            context.clone(),
            now,
        );
        chat.event_cursor = cue.event_id;
    }
    let now = chat.clock;
    chat.manager.offer_idle(context, now);
    if session.presentation.dialogue.is_none()
        && let Some(line) = chat
            .manager
            .tick(session.presentation.session, view.hand_number)
    {
        output.write(DialogueRequest(line));
    }
}
#[allow(clippy::type_complexity)] // Distinct marker filters keep Bevy text queries disjoint.
pub fn render(
    chat: Res<ConversationUi>,
    mut history: Query<&mut Text, With<ChatHistory>>,
    mut target: Query<&mut Text, (With<ChatTarget>, Without<ChatHistory>)>,
    mut input: Query<&mut Text, (With<ChatInput>, Without<ChatHistory>, Without<ChatTarget>)>,
    mut status: Query<
        &mut Text,
        (
            With<ChatStatus>,
            Without<ChatHistory>,
            Without<ChatTarget>,
            Without<ChatInput>,
        ),
    >,
) {
    if let Ok(mut value) = history.single_mut() {
        let lines = chat
            .manager
            .history()
            .iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>();
        value.0 = if lines.is_empty() {
            "Say something to the table.".into()
        } else {
            lines
                .into_iter()
                .rev()
                .map(|m| {
                    let short: String = m.text.chars().take(64).collect();
                    format!(
                        "{}{}{}: {}{}",
                        m.speaker.label(),
                        m.source.map_or("", |source| source.badge()),
                        if m.audience == poker_lab::memory::Audience::Public {
                            ""
                        } else {
                            " [private]"
                        },
                        short,
                        if m.text.chars().count() > 64 {
                            "…"
                        } else {
                            ""
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
    }
    if let Ok(mut value) = target.single_mut() {
        value.0 = chat.target.label().into();
    }
    if let Ok(mut value) = input.single_mut() {
        value.0 = if chat.input.is_empty() {
            if chat.editing {
                "Type a message…"
            } else {
                "Click to chat"
            }
            .into()
        } else {
            let tail: String = chat
                .input
                .chars()
                .rev()
                .take(18)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if chat.editing {
                format!("{tail}│")
            } else {
                tail
            }
        };
    }
    if let Ok(mut value) = status.single_mut() {
        value.0 = format!(
            "{}{}",
            chat.manager
                .thinking_speaker()
                .map(|speaker| format!("{} is thinking...", speaker.label()))
                .unwrap_or_else(|| chat.manager.label().into()),
            if chat.target == Target::Table {
                " / public"
            } else {
                " / private"
            }
        );
    }
}
