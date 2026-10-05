//! Persistent portrait entities; images are loaded once, animation is frame-based.
use super::{
    GOLD, MUTED, TEXT, label,
    table::{column, row},
};
use crate::game::GameSession;
use bevy::prelude::*;
use poker_lab::{
    characters::{CAST, CharacterExpression as Expression, DialogueLine, definition},
    npc::profiles::{display_name, profile},
    poker::Seat,
};

#[derive(Resource, Default)]
pub struct PortraitAssets {
    pub sheets: [Handle<Image>; 3],
    /// Optional individual expression files, configured once by the host.
    /// Missing/loading/failed overrides fall back to the matching atlas frame.
    pub overrides: [[Option<Handle<Image>>; 7]; 3],
}
impl PortraitAssets {
    pub fn resolve(
        &self,
        images: &Assets<Image>,
        seat: Seat,
        expression: Expression,
    ) -> Option<(Handle<Image>, Option<Rect>)> {
        let i = seat.index().checked_sub(1)?;
        if let Some(handle) = &self.overrides[i][expression.index()]
            && images.contains(handle.id())
        {
            return Some((handle.clone(), None));
        }
        let handle = &self.sheets[i];
        let image = images.get(handle)?;
        let cell = image.size().as_vec2() / Vec2::new(4.0, 2.0);
        let index = expression.index();
        let min = Vec2::new((index % 4) as f32, (index / 4) as f32) * cell;
        Some((handle.clone(), Some(Rect::from_corners(min, min + cell))))
    }
}
pub fn load(server: Option<Res<AssetServer>>, mut assets: ResMut<PortraitAssets>) {
    if let Some(server) = server {
        for (i, c) in CAST.iter().enumerate() {
            assets.sheets[i] = server.load(c.portrait_path);
        }
    }
}

#[derive(Message)]
pub struct DialogueRequest(pub DialogueLine);

#[derive(Clone, Copy)]
struct Tween {
    expression: Expression,
    previous: Expression,
    blend: f32,
    dim: f32,
}
impl Default for Tween {
    fn default() -> Self {
        Self {
            expression: Expression::Neutral,
            previous: Expression::Neutral,
            blend: 1.0,
            dim: 1.0,
        }
    }
}
#[derive(Resource, Default)]
pub struct CharacterAnimation {
    session: u64,
    age: f32,
    tweens: [Tween; 4],
}

#[derive(Component)]
pub struct PortraitFrame(pub Seat);
#[derive(Component)]
pub struct PortraitLayer {
    pub seat: Seat,
    back: bool,
}
#[derive(Component)]
pub struct MissingPortrait(Seat);
#[derive(Component)]
pub struct SeatReadout {
    seat: Seat,
    field: Readout,
}
#[derive(Clone, Copy)]
enum Readout {
    Chips,
    Badges,
    Expression,
    Action,
}
#[derive(Component)]
pub struct NpcCard {
    seat: Seat,
    card: usize,
}
#[derive(Component)]
pub struct DialoguePanel;
#[derive(Component)]
pub struct DialogueText;
#[derive(Component)]
pub struct DialoguePortrait;
#[derive(Component)]
pub struct MenuPortrait(pub Seat);

pub fn menu_portraits(
    assets: Res<PortraitAssets>,
    images: Option<Res<Assets<Image>>>,
    mut portraits: Query<(&MenuPortrait, &mut ImageNode)>,
) {
    let Some(images) = images else {
        return;
    };
    for (portrait, mut node) in &mut portraits {
        if let Some((handle, rect)) = assets.resolve(&images, portrait.0, Expression::Neutral)
            && (node.image != handle || node.rect != rect)
        {
            node.image = handle;
            node.rect = rect;
        }
    }
}

pub fn spawn(parent: &mut ChildSpawnerCommands, seat: Seat, heads_up: bool) {
    let c = definition(seat).unwrap();
    let p = profile(seat);
    let accent = Color::srgb(p.accent[0], p.accent[1], p.accent[2]);
    let centered = heads_up || seat == Seat::Jax;
    let mut position = Node {
        position_type: PositionType::Absolute,
        width: Val::Px(230.0),
        top: Val::Px(if centered { 60.0 } else { 105.0 }),
        ..column(5.0)
    };
    if centered {
        position.left = Val::Percent(50.0);
        position.margin.left = Val::Px(-115.0);
    } else if seat == Seat::Npc {
        position.left = Val::Percent(4.0);
    } else {
        position.right = Val::Percent(4.0);
    }
    parent.spawn((position, ZIndex(3))).with_children(|panel| {
        panel
            .spawn((
                PortraitFrame(seat),
                UiTransform::default(),
                Node {
                    width: Val::Px(if centered { 156.0 } else { 186.0 }),
                    height: Val::Px(if centered { 208.0 } else { 248.0 }),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.04, 0.055, 0.07)),
                BorderColor::all(accent.with_alpha(0.5)),
            ))
            .with_children(|frame| {
                frame.spawn((
                    MissingPortrait(seat),
                    Text::new(format!("{}\nPortrait unavailable", p.name)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(accent),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(40.0),
                        left: Val::Px(8.0),
                        ..default()
                    },
                ));
                for back in [true, false] {
                    frame.spawn((
                        PortraitLayer { seat, back },
                        ImageNode::default(),
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                }
            });
        label(panel, p.name, 23.0, accent);
        label(panel, format!("{} / {}", c.origin, c.age), 11.0, MUTED);
        for field in [Readout::Chips, Readout::Badges] {
            panel.spawn((
                SeatReadout { seat, field },
                Text::new(""),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(TEXT),
            ));
        }
        panel.spawn(row(6.0)).with_children(|cards| {
            for card in 0..2 {
                cards
                    .spawn((
                        Node {
                            width: Val::Px(42.0),
                            height: Val::Px(52.0),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            justify_content: JustifyContent::Center,
                            ..column(0.0)
                        },
                        BackgroundColor(Color::srgb(0.08, 0.13, 0.17)),
                    ))
                    .with_children(|slot| {
                        slot.spawn((
                            NpcCard { seat, card },
                            Text::new("?"),
                            TextFont {
                                font_size: 22.0,
                                ..default()
                            },
                            TextColor(MUTED),
                        ));
                    });
            }
        });
        for (field, size, color) in [
            (Readout::Expression, 11.0, MUTED),
            (Readout::Action, 14.0, accent),
        ] {
            panel.spawn((
                SeatReadout { seat, field },
                Text::new(""),
                TextFont {
                    font_size: size,
                    ..default()
                },
                TextColor(color),
            ));
        }
    });
}

pub fn spawn_dialogue(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            DialoguePanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-285.0)),
                bottom: Val::Px(246.0),
                width: Val::Px(570.0),
                min_height: Val::Px(64.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                border: UiRect::left(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..row(14.0)
            },
            Visibility::Hidden,
            BackgroundColor(Color::srgb(0.055, 0.065, 0.09)),
            BorderColor::all(GOLD),
            ZIndex(5),
        ))
        .with_children(|panel| {
            panel.spawn((
                DialoguePortrait,
                ImageNode::default(),
                Node {
                    width: Val::Px(42.0),
                    height: Val::Px(56.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            panel.spawn((
                DialogueText,
                Node {
                    min_width: Val::Px(0.0),
                    flex_shrink: 1.0,
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT),
            ));
        });
}

pub fn tick(
    time: Res<Time>,
    mut session: ResMut<GameSession>,
    mut animation: ResMut<CharacterAnimation>,
    mut requests: MessageReader<DialogueRequest>,
) {
    if animation.session != session.presentation.session {
        *animation = CharacterAnimation {
            session: session.presentation.session,
            ..default()
        };
    }
    let dt = time.delta_secs();
    animation.age += dt;
    session.presentation.tick(dt);
    for request in requests.read() {
        session.presentation.say(request.0.clone());
    }
    for (i, tween) in animation.tweens.iter_mut().enumerate() {
        let state = &session.presentation.seats[i];
        if tween.expression != state.expression {
            tween.previous = tween.expression;
            tween.expression = state.expression;
            tween.blend = 0.0;
        }
        tween.blend = (tween.blend + dt / 0.22).min(1.0);
        let target = if state.eliminated {
            0.30
        } else if state.folded {
            0.58
        } else {
            1.0
        };
        tween.dim += (target - tween.dim) * (dt * 6.0).min(1.0);
    }
}

pub fn sync_readouts(
    session: Res<GameSession>,
    mut revision: Local<Option<(u64, usize)>>,
    mut readouts: Query<(Ref<SeatReadout>, &mut Text)>,
    mut cards: Query<(&NpcCard, &mut Text, &mut TextColor), Without<SeatReadout>>,
) {
    let current = (session.presentation.session, session.engine.history().len());
    if *revision == Some(current) && !readouts.iter().any(|(r, _)| r.is_added()) {
        return;
    }
    *revision = Some(current);
    let view = session.engine.observe(Seat::Human);
    for (readout, mut text) in &mut readouts {
        let i = readout.seat.index();
        if i >= view.stacks.len() {
            continue;
        }
        let value = match readout.field {
            Readout::Chips => format!("{} chips / Bet {}", view.stacks[i], view.street_bets[i]),
            Readout::Badges => {
                let mut badges = Vec::new();
                if view.dealer == readout.seat {
                    badges.push("D");
                }
                if view.small_blind == Some(readout.seat) {
                    badges.push("SB");
                }
                if view.big_blind == readout.seat {
                    badges.push("BB");
                }
                if view.eliminated[i] {
                    badges.push("OUT");
                } else if view.folded[i] {
                    badges.push("FOLDED");
                } else if view.stacks[i] == 0 {
                    badges.push("ALL-IN");
                } else if view.actor == Some(readout.seat) {
                    badges.push("TO ACT");
                }
                badges.join(" / ")
            }
            _ => continue,
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (slot, mut text, mut color) in &mut cards {
        let i = slot.seat.index();
        let card = view
            .revealed_cards
            .as_ref()
            .and_then(|h| h.get(i))
            .copied()
            .flatten()
            .map(|h| h[slot.card]);
        text.0 = card.map(|c| c.to_string()).unwrap_or_else(|| {
            if view.eliminated[i] {
                "-"
            } else if view.folded[i] {
                "x"
            } else {
                "?"
            }
            .into()
        });
        color.0 = if card.is_some_and(|c| c.suit.is_red()) {
            Color::srgb(1.0, 0.47, 0.42)
        } else {
            TEXT
        };
    }
}

// Animation systems only borrow the session immutably; they cannot submit actions.
#[allow(clippy::too_many_arguments)] // Disjoint ECS queries keep animation read-only on gameplay.
pub fn animate(
    session: Res<GameSession>,
    chat: Option<Res<super::conversation::ConversationUi>>,
    animation: Res<CharacterAnimation>,
    assets: Res<PortraitAssets>,
    images: Option<Res<Assets<Image>>>,
    mut frames: Query<(&PortraitFrame, &mut UiTransform, &mut BorderColor)>,
    mut layers: Query<(&PortraitLayer, &mut ImageNode)>,
    mut missing: Query<(&MissingPortrait, &mut Visibility)>,
    mut readouts: Query<(&SeatReadout, &mut Text, &mut TextColor)>,
) {
    for (frame, mut transform, mut border) in &mut frames {
        let i = frame.0.index();
        let state = &session.presentation.seats[i];
        let intensity = definition(frame.0).unwrap().intensity;
        let idle = if state.eliminated {
            0.0
        } else {
            (animation.age * 1.1 + i as f32).sin() * intensity * 1.8
        };
        transform.translation = Val2::px(
            0.0,
            idle + if state.eliminated {
                (1.0 - animation.tweens[i].dim) * 8.0
            } else {
                0.0
            },
        );
        transform.scale = Vec2::splat(1.0 + state.emphasis * intensity * 0.012);
        let p = profile(frame.0);
        let accent = Color::srgb(p.accent[0], p.accent[1], p.accent[2]);
        *border = BorderColor::all(if state.active {
            GOLD
        } else {
            accent.with_alpha(0.32 + state.emphasis * 0.68)
        });
    }
    for (layer, mut image) in &mut layers {
        let tween = animation.tweens[layer.seat.index()];
        let expression = if layer.back {
            tween.previous
        } else {
            tween.expression
        };
        if let Some((handle, rect)) = images
            .as_ref()
            .and_then(|images| assets.resolve(images, layer.seat, expression))
        {
            image.image = handle;
            image.rect = rect;
            let alpha = if layer.back {
                1.0 - tween.blend
            } else {
                tween.blend
            };
            image.color = Color::srgba(
                tween.dim,
                tween.dim,
                tween.dim,
                alpha * (animation.age / 0.4).min(1.0),
            );
        } else {
            image.color = Color::NONE;
        }
    }
    for (fallback, mut visibility) in &mut missing {
        *visibility = if images.as_ref().is_some_and(|images| {
            assets
                .resolve(images, fallback.0, Expression::Neutral)
                .is_some()
        }) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    for (readout, mut text, mut color) in &mut readouts {
        let state = &session.presentation.seats[readout.seat.index()];
        match readout.field {
            Readout::Expression => {
                let caption = if state.eliminated || state.active || state.reaction_left > 0.0 {
                    state.expression.label()
                } else {
                    chat.as_ref()
                        .and_then(|chat| {
                            poker_lab::memory::NpcId::from_seat(readout.seat)
                                .map(|npc| chat.manager.mood(npc).mood.label())
                        })
                        .unwrap_or(state.expression.label())
                };
                if text.0 != caption {
                    text.0 = caption.into();
                }
            }
            Readout::Action => {
                if text.0 != state.action_text {
                    text.0.clone_from(&state.action_text);
                }
                // Keep the last action readable after its brief emphasis fades.
                color.0 = color.0.with_alpha(0.45 + 0.55 * state.action_left.min(1.0));
            }
            _ => {}
        }
    }
}

pub fn dialogue(
    session: Res<GameSession>,
    assets: Res<PortraitAssets>,
    images: Option<Res<Assets<Image>>>,
    mut panel: Query<(&mut Visibility, &mut BorderColor), With<DialoguePanel>>,
    mut text: Query<(&mut Text, &mut TextColor), With<DialogueText>>,
    mut portraits: Query<&mut ImageNode, With<DialoguePortrait>>,
) {
    let state = &session.presentation;
    for (mut visibility, mut border) in &mut panel {
        *visibility = if state.dialogue.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(line) = &state.dialogue {
            let p = profile(line.speaker);
            *border = BorderColor::all(Color::srgb(p.accent[0], p.accent[1], p.accent[2]));
        }
    }
    if let Some(line) = &state.dialogue {
        for (mut text, mut color) in &mut text {
            // Update text only when the line changes, not once per frame.
            let prefix = format!("{}{}", display_name(line.speaker), line.source.badge());
            let rendered = format!("{prefix}: {}", line.text);
            if text.0 != rendered {
                text.0 = rendered;
            }
            color.0 = TEXT.with_alpha(state.dialogue_left.min(1.0));
        }
        for mut portrait in &mut portraits {
            let expression = state.seats[line.speaker.index()].expression;
            if let Some((handle, rect)) = images
                .as_ref()
                .and_then(|i| assets.resolve(i, line.speaker, expression))
            {
                portrait.image = handle;
                portrait.rect = rect;
                portrait.color = Color::WHITE.with_alpha(state.dialogue_left.min(1.0));
            } else {
                portrait.color = Color::NONE;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_optional_expression_falls_back_to_atlas_then_named_placeholder() {
        let mut images = Assets::<Image>::default();
        let mut assets = PortraitAssets::default();
        assert!(
            assets
                .resolve(&images, Seat::Npc, Expression::Happy)
                .is_none()
        );
        assets.sheets[0] = images.add(Image::default());
        assets.overrides[0][Expression::Happy.index()] = Some(Handle::default());
        let (handle, rect) = assets
            .resolve(&images, Seat::Npc, Expression::Happy)
            .unwrap();
        assert_eq!(handle, assets.sheets[0]);
        assert!(rect.is_some());
        let replacement = images.add(Image::default());
        assets.overrides[0][Expression::Happy.index()] = Some(replacement.clone());
        assert_eq!(
            assets.resolve(&images, Seat::Npc, Expression::Happy),
            Some((replacement, None))
        );
    }
}
