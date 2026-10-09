//! Bevy host for public statistical learning; SQLite stays on the existing worker.
use super::{BUTTON, GOLD, TEXT, memory::MemoryUi};
use crate::game::GameSession;
use bevy::prelude::*;
use poker_lab::npc::{
    opponent::{HandSample, Metric, OpponentModel},
    profiles::NpcId,
};

#[derive(Resource)]
pub struct LearningUi {
    pub model: OpponentModel,
    pub hints: Vec<String>,
    frozen_model: OpponentModel,
    ready: bool,
    frozen: (u64, u64),
    process: u64,
    pending: Vec<(String, HandSample)>,
    open: bool,
    npc: usize,
    metrics: bool,
}
impl Default for LearningUi {
    fn default() -> Self {
        Self {
            model: Default::default(),
            hints: Vec::new(),
            frozen_model: Default::default(),
            ready: false,
            frozen: (0, 0),
            process: rand::random(),
            pending: Vec::new(),
            open: false,
            npc: 0,
            metrics: false,
        }
    }
}
pub fn update(mut ui: ResMut<LearningUi>, mut session: ResMut<GameSession>, memory: Res<MemoryUi>) {
    let epoch = session.presentation.session;
    for (hand, sample) in session.completed_observations.drain(..) {
        let key = format!("{:016x}-{epoch}-{hand}", ui.process);
        ui.pending.push((key, sample));
    }
    if !ui.ready
        && let Some(model) = memory.service.snapshot().opponent
    {
        ui.model = model;
        ui.hints = ui.model.hints();
        ui.ready = true;
    }
    if ui.ready {
        let pending = std::mem::take(&mut ui.pending);
        for (key, sample) in pending {
            ui.model.record(sample.clone());
            ui.hints = ui.model.hints();
            memory.service.record_opponent(key, sample);
        }
    }
    let turn = (epoch, session.presentation.hand);
    if turn != ui.frozen {
        session.snapshot_strategy(&ui.model);
        ui.frozen_model = ui.model.clone();
        ui.frozen = turn;
    }
}
#[derive(Component)]
pub struct LearningPanel;
#[derive(Component)]
pub struct LearningText;
#[derive(Component, Clone, Copy)]
pub enum LearningControl {
    Toggle,
    Next,
    Metrics,
}
fn button(p: &mut ChildSpawnerCommands, label: &str, control: LearningControl) {
    p.spawn((
        Button,
        control,
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(BUTTON),
    ))
    .with_children(|p| super::label(p, label, 15.0, TEXT));
}
pub fn spawn(p: &mut ChildSpawnerCommands) {
    p.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Px(144.0),
        top: Val::Px(54.0),
        ..default()
    })
    .with_children(|p| button(p, "Poker reads", LearningControl::Toggle));
    p.spawn((
        LearningPanel,
        Visibility::Hidden,
        ZIndex(30),
        Interaction::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-440.0)),
            top: Val::Px(130.0),
            width: Val::Px(880.0),
            min_height: Val::Px(550.0),
            padding: UiRect::all(Val::Px(20.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.035, 0.05, 0.075)),
    ))
    .with_children(|p| {
        p.spawn(Node {
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|p| {
            button(p, "Next NPC read", LearningControl::Next);
            button(p, "Metrics / strategy", LearningControl::Metrics);
            button(p, "Close reads", LearningControl::Toggle);
        });
        p.spawn((
            LearningText,
            Text::new("Collecting evidence"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(GOLD),
        ));
        super::label(
            p,
            "Debug view. Reads freeze at hand start; counters update after completed hands.",
            13.0,
            TEXT,
        );
    });
}
pub fn buttons(
    query: Query<(&Interaction, &LearningControl), Changed<Interaction>>,
    mut ui: ResMut<LearningUi>,
) {
    for (interaction, control) in &query {
        if *interaction == Interaction::Pressed {
            match control {
                LearningControl::Toggle => ui.open = !ui.open,
                LearningControl::Next => ui.npc = (ui.npc + 1) % 3,
                LearningControl::Metrics => ui.metrics = !ui.metrics,
            }
        }
    }
}
pub fn render(
    ui: Res<LearningUi>,
    session: Res<GameSession>,
    mut panels: Query<&mut Visibility, With<LearningPanel>>,
    mut texts: Query<&mut Text, With<LearningText>>,
) {
    for mut v in &mut panels {
        *v = if ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !ui.open {
        return;
    }
    let npc = NpcId::ALL[ui.npc];
    let content = if ui.metrics {
        let recent = ui.model.recent_total();
        let mut lines = vec![format!(
            "PUBLIC HUMAN MODEL / {} hands / last {} hands",
            ui.model.hands,
            ui.model.recent.len()
        )];
        for m in Metric::ALL {
            let c = ui.model.total.get(m);
            let r = recent.get(m);
            if matches!(m, Metric::BetSize | Metric::RaiseSize) {
                lines.push(format!(
                    "{}: {} samples / mean {:.2} pot",
                    m.label(),
                    c.opportunities,
                    c.yes as f64 / (10_000.0 * c.opportunities.max(1) as f64)
                ));
            } else {
                lines.push(format!(
                    "{}: {}/{} / estimate {:.0}% / confidence {:.0}% / recent {}/{}",
                    m.label(),
                    c.yes,
                    c.opportunities,
                    ui.model.estimate(m) * 100.0,
                    c.confidence() * 100.0,
                    r.yes,
                    r.opportunities
                ));
            }
        }
        lines.join("\n")
    } else {
        session.reads[ui.npc].describe(npc, &ui.frozen_model)
    };
    for mut text in &mut texts {
        if text.0 != content {
            text.0 = content.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{NpcSettings, TableMode};
    use poker_lab::{npc::opponent::Count, poker::Action};

    #[test]
    fn reads_freeze_for_the_hand_and_completed_observations_survive_restart() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MemoryUi>()
            .init_resource::<LearningUi>()
            .insert_resource(GameSession::new(
                42,
                TableMode::Four,
                NpcSettings::default(),
            ))
            .add_systems(Update, update);
        let service = app.world().resource::<MemoryUi>().service.clone();
        assert!(service.flush());
        app.update();
        let frozen = app.world().resource::<GameSession>().reads.clone();
        let mut strong = OpponentModel::default();
        for _ in 0..100 {
            let mut sample = HandSample::default();
            sample.counts[Metric::Pfr as usize] = Count {
                yes: 1,
                opportunities: 1,
            };
            strong.record(sample);
        }
        app.world_mut().resource_mut::<LearningUi>().model = strong;
        app.update();
        assert_eq!(app.world().resource::<GameSession>().reads, frozen);
        {
            let mut game = app.world_mut().resource_mut::<GameSession>();
            while let Some(seat) = game.engine.actor() {
                game.submit(seat, Action::Fold);
            }
        }
        app.update();
        app.update(); // Repeated frame consumption must not double count.
        assert!(service.flush());
        assert_eq!(service.snapshot().opponent.unwrap().hands, 1);
        assert_eq!(app.world().resource::<LearningUi>().model.hands, 101);
        assert_eq!(app.world().resource::<GameSession>().reads, frozen);
        app.world_mut().insert_resource(GameSession::new(
            44,
            TableMode::Four,
            NpcSettings::default(),
        ));
        app.update();
        assert_ne!(app.world().resource::<GameSession>().reads, frozen);
        assert_eq!(app.world().resource::<LearningUi>().model.hands, 101);
    }
}
