//! Bevy bridge: enqueue facts and render snapshots; all SQLite work is off-thread.
use super::{BUTTON, GOLD, MUTED, TEXT, label};
use crate::game::GameSession;
use bevy::prelude::*;
use poker_lab::memory::*;
use std::path::PathBuf;

#[derive(Resource, Default)]
pub struct MemorySettings(pub Option<PathBuf>);
#[derive(Resource)]
pub struct MemoryUi {
    pub service: MemoryService,
    key: Option<String>,
    epoch: u64,
    process: u64,
    cursor: usize,
    chat_sequence: u64,
    observer: PokerObserver,
    selected: usize,
    open: bool,
    snapshot: MemorySnapshot,
    refresh: f32,
}
impl FromWorld for MemoryUi {
    fn from_world(world: &mut World) -> Self {
        let path = world
            .get_resource::<MemorySettings>()
            .and_then(|s| s.0.clone());
        Self {
            service: MemoryService::start(path),
            key: None,
            epoch: 0,
            process: rand::random(),
            cursor: 0,
            chat_sequence: 0,
            observer: PokerObserver::default(),
            selected: 0,
            open: false,
            snapshot: MemorySnapshot::default(),
            refresh: 0.0,
        }
    }
}
pub fn capture(session: Res<GameSession>, mut memory: ResMut<MemoryUi>) {
    let epoch = session.presentation.session;
    if memory.epoch != epoch {
        if let Some(old) = memory.key.take() {
            memory.service.end(old);
        }
        memory.epoch = epoch;
        memory.cursor = 0;
        memory.chat_sequence = 0;
        memory.observer = PokerObserver::default();
        let key = format!("{:016x}-{epoch}", memory.process);
        let count = session
            .engine
            .observe(poker_lab::poker::Seat::Human)
            .stacks
            .len();
        let participants = NpcId::ALL
            .into_iter()
            .filter(|n| n.seat().index() < count)
            .collect();
        memory.service.begin(key.clone(), participants);
        memory.key = Some(key);
        memory.refresh = 0.0;
    }
    let key = memory.key.clone().unwrap();
    let history = session.engine.history();
    for (index, event) in history.iter().enumerate().skip(memory.cursor) {
        for (part, observed) in memory.observer.observe(event).into_iter().enumerate() {
            memory
                .service
                .record(key.clone(), format!("poker:{index}:{part}"), observed);
        }
    }
    memory.cursor = history.len();
}
pub fn capture_dialogue(
    mut chat: ResMut<super::conversation::ConversationUi>,
    session: Res<GameSession>,
    mut memory: ResMut<MemoryUi>,
) {
    let Some(key) = memory.key.clone() else {
        return;
    };
    let count = if session.mode == crate::game::TableMode::HeadsUp {
        2
    } else {
        4
    };
    for message in chat.manager.drain_observed() {
        // Outage fixtures are not meaningful character experiences. Human
        // messages still form memories offline, preserving mock recall tests.
        if message
            .source
            .is_some_and(|s| s != poker_lab::characters::DialogueSource::Model)
        {
            continue;
        }
        if conversation_kind(message.speaker.id(), &message.text).is_none() {
            continue;
        }
        memory.chat_sequence += 1;
        let witnesses = NpcId::ALL
            .into_iter()
            .filter(|n| n.seat().index() < count && message.audience.permits(*n))
            .collect();
        memory.service.record(
            key.clone(),
            format!("chat:{}", memory.chat_sequence),
            ObservedEvent {
                witnesses,
                fact: Fact::Conversation {
                    speaker: message.speaker.id().into(),
                    audience: message.audience,
                    text: message.text,
                },
            },
        );
    }
}
pub fn finish(mut memory: ResMut<MemoryUi>) {
    if let Some(key) = memory.key.take() {
        memory.service.end(key);
    }
    memory.epoch = 0;
}
pub fn shutdown(mut exits: MessageReader<AppExit>, mut memory: ResMut<MemoryUi>) {
    if exits.read().next().is_none() {
        return;
    }
    if let Some(key) = memory.key.take() {
        memory.service.end(key);
    }
    if !memory.service.flush() {
        warn!("Memory flush did not complete before shutdown");
    }
}

#[derive(Component, Clone, Copy)]
pub enum MemoryControl {
    Toggle,
    Next,
}
#[derive(Component)]
pub struct MemoryPanel;
#[derive(Component)]
pub struct MemoryText;
pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(24.0),
            top: Val::Px(54.0),
            ..default()
        })
        .with_children(|p| button(p, "Memories", MemoryControl::Toggle));
    parent
        .spawn((
            MemoryPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-380.0)),
                top: Val::Px(165.0),
                width: Val::Px(760.0),
                height: Val::Px(490.0),
                padding: UiRect::all(Val::Px(22.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            Visibility::Hidden,
            Interaction::default(),
            ZIndex(20),
            BackgroundColor(Color::srgb(0.035, 0.05, 0.075)),
            BorderColor::all(GOLD),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    column_gap: Val::Px(20.0),
                    ..default()
                })
                .with_children(|row| {
                    button(row, "Next character", MemoryControl::Next);
                    button(row, "Close memories", MemoryControl::Toggle);
                    label(row, "SOCIAL PROFILE / DEBUG", 16.0, GOLD);
                });
            panel.spawn((
                MemoryText,
                Text::new("Loading memories…"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT),
                Node {
                    width: Val::Percent(100.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ));
            label(
                panel,
                "Private entries are visible to you and their owner only. Scores: 0–1000.",
                12.0,
                MUTED,
            );
        });
}
fn button(parent: &mut ChildSpawnerCommands, text: &str, control: MemoryControl) {
    parent
        .spawn((
            Button,
            control,
            ZIndex(21),
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(BUTTON),
        ))
        .with_children(|p| label(p, text, 15.0, TEXT));
}
pub fn buttons(
    buttons: Query<(&Interaction, &MemoryControl), Changed<Interaction>>,
    mut memory: ResMut<MemoryUi>,
) {
    for (interaction, control) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match control {
            MemoryControl::Toggle => memory.open = !memory.open,
            MemoryControl::Next => memory.selected = (memory.selected + 1) % 3,
        }
        memory.refresh = 0.0;
    }
}
pub fn render(
    time: Res<Time>,
    mut memory: ResMut<MemoryUi>,
    mut panels: Query<&mut Visibility, With<MemoryPanel>>,
    mut text: Query<&mut Text, With<MemoryText>>,
) {
    for mut panel in &mut panels {
        *panel = if memory.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    memory.refresh -= time.delta_secs();
    if memory.refresh > 0.0 {
        return;
    }
    memory.refresh = 0.5;
    let snapshot = memory.service.snapshot();
    if snapshot.error != memory.snapshot.error
        && let Some(error) = &snapshot.error
    {
        warn!("{error}");
    }
    memory.snapshot = snapshot;
    let npc = NpcId::ALL[memory.selected];
    let status = if memory.snapshot.error.is_some() {
        "Save unavailable / retrying"
    } else if memory.snapshot.persistent {
        "Saved locally"
    } else {
        "Temporary memory"
    };
    let content = if let Some(profile) = memory.snapshot.characters.iter().find(|p| p.npc == npc) {
        let r = &profile.relationship;
        let mut lines = vec![
            format!("{} / {} / {status}", npc.name(), r.tier(npc)),
            format!(
                "{} sessions together / {} hands",
                profile.sessions, profile.hands
            ),
            format!(
                "Familiarity {} / Trust {} / Respect {} / Tension {} / Warmth {}",
                r.familiarity, r.trust, r.respect, r.tension, r.warmth
            ),
        ];
        for item in profile.memories.iter().take(4) {
            let summary: String = item.summary.chars().take(170).collect();
            lines.push(format!(
                "\n{}{}\nImportance {} / occurrences {} / recalled {}",
                if item.audience == Audience::Public {
                    ""
                } else {
                    "[Private] "
                },
                summary,
                item.importance,
                item.occurrences,
                item.recall_count
            ));
        }
        if profile.memories.is_empty() {
            lines.push("\nNo notable memories yet.".into());
        }
        lines.join("\n")
    } else {
        format!("{} · {status}\nLoading social profile…", npc.name())
    };
    for mut value in &mut text {
        if value.0 != content {
            value.0 = content.clone();
        }
    }
}
