//! Read-only modal views. Only the game host can create safe review frames.
use super::{BUTTON, GOLD, MUTED, TEXT, conversation::ConversationUi, learning::LearningUi};
use crate::game::GameSession;
use bevy::prelude::*;
use poker_lab::{
    npc::profiles::display_name,
    poker::Seat,
    review::{Moment, Selection},
};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum View {
    #[default]
    Closed,
    Review,
    Statistics,
}
#[derive(Resource, Default)]
pub struct ReviewUi {
    view: View,
    selection: Selection,
    epoch: u64,
}
pub fn table_active(ui: Res<ReviewUi>) -> bool {
    !ui.is_open()
}
impl ReviewUi {
    pub fn is_open(&self) -> bool {
        self.view != View::Closed
    }
}
pub fn sync(session: Res<GameSession>, mut ui: ResMut<ReviewUi>) {
    if ui.epoch != session.presentation.session {
        *ui = ReviewUi {
            epoch: session.presentation.session,
            ..Default::default()
        };
    }
}
#[derive(Component)]
pub struct ReviewPanel;
#[derive(Component)]
pub struct ReviewText;
#[derive(Component)]
pub struct ReviewNavigation;
#[derive(Default)]
pub struct RenderCache(Option<(View, u64, usize, usize, usize, u64)>);
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum ReviewControl {
    Open,
    Statistics,
    PreviousHand,
    NextHand,
    PreviousStep,
    NextStep,
    Close,
}

fn button(parent: &mut ChildSpawnerCommands, text: &str, control: ReviewControl) {
    parent
        .spawn((
            Button,
            control,
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(BUTTON),
        ))
        .with_children(|p| super::label(p, text, 16.0, TEXT));
}
pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(310.0),
            top: Val::Px(54.0),
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|p| {
            button(p, "Hand Review", ReviewControl::Open);
            button(p, "Statistics", ReviewControl::Statistics);
        });
    parent
        .spawn((
            ReviewPanel,
            Visibility::Hidden,
            ZIndex(40),
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.025, 0.04, 0.065)),
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Percent(90.0),
                max_width: Val::Px(1080.0),
                height: Val::Px(920.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(18.0),
                ..default()
            })
            .with_children(|p| {
                super::label(p, "POKER LAB / AT A GLANCE", 24.0, GOLD);
                p.spawn((
                    ReviewNavigation,
                    Node {
                        column_gap: Val::Px(10.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    button(row, "Previous Hand", ReviewControl::PreviousHand);
                    button(row, "Next Hand", ReviewControl::NextHand);
                    button(row, "Previous Action", ReviewControl::PreviousStep);
                    button(row, "Next Action", ReviewControl::NextStep);
                });
                p.spawn((
                    ReviewText,
                    Text::new(""),
                    TextFont {
                        font_size: 17.0,
                        ..default()
                    },
                    TextColor(TEXT),
                    TextLayout::new_with_linebreak(bevy::text::LineBreak::WordOrCharacter),
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                super::label(
                    p,
                    "Betting waits while this view is open. Return to continue your live table.",
                    14.0,
                    MUTED,
                );
                button(p, "Return to Table", ReviewControl::Close);
            });
        });
}
type ReviewButtons<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ReviewControl),
    (Changed<Interaction>, With<Button>),
>;
pub fn buttons(
    buttons: ReviewButtons,
    mut ui: ResMut<ReviewUi>,
    mut session: ResMut<GameSession>,
    mut chat: ResMut<ConversationUi>,
) {
    for (interaction, control) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match control {
            ReviewControl::Open => {
                ui.view = View::Review;
                ui.selection = Selection::latest(&session.reviews);
            }
            ReviewControl::Statistics => ui.view = View::Statistics,
            ReviewControl::PreviousHand => ui.selection.move_hand(&session.reviews, false),
            ReviewControl::NextHand => ui.selection.move_hand(&session.reviews, true),
            ReviewControl::PreviousStep => ui.selection.move_step(&session.reviews, false),
            ReviewControl::NextStep => ui.selection.move_step(&session.reviews, true),
            ReviewControl::Close => ui.view = View::Closed,
        }
        // Clear entry focus so closing a modal cannot submit a stale edit.
        session.editing = false;
        chat.editing = false;
        break;
    }
}

#[allow(clippy::too_many_arguments)] // Bevy injects separate resources and disjoint UI queries.
pub fn render(
    mut commands: Commands,
    ui: Res<ReviewUi>,
    session: Res<GameSession>,
    learning: Res<LearningUi>,
    mut panel: Query<&mut Visibility, With<ReviewPanel>>,
    mut texts: Query<&mut Text, With<ReviewText>>,
    mut navigation: Query<&mut Node, With<ReviewNavigation>>,
    mut cache: Local<RenderCache>,
    mut buttons: Query<(
        Entity,
        &ReviewControl,
        &mut BackgroundColor,
        Option<&Button>,
    )>,
) {
    for mut visible in &mut panel {
        *visible = if ui.view == View::Closed {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let hands = session.reviews.hands();
    let key = (
        ui.view,
        ui.epoch,
        ui.selection.hand,
        ui.selection.step,
        hands.len(),
        learning.model.hands,
    );
    if cache.0 == Some(key) {
        return;
    }
    cache.0 = Some(key);
    for mut node in &mut navigation {
        node.display = if ui.view == View::Statistics {
            Display::None
        } else {
            Display::Flex
        };
    }
    for (entity, control, mut color, button) in &mut buttons {
        let enabled = match control {
            ReviewControl::PreviousHand => ui.view == View::Review && ui.selection.hand > 0,
            ReviewControl::NextHand => {
                ui.view == View::Review && ui.selection.hand + 1 < hands.len()
            }
            ReviewControl::PreviousStep => ui.view == View::Review && ui.selection.step > 0,
            ReviewControl::NextStep => {
                ui.view == View::Review
                    && hands
                        .get(ui.selection.hand)
                        .is_some_and(|h| ui.selection.step + 1 < h.frames.len())
            }
            _ => true,
        };
        if enabled != button.is_some() {
            if enabled {
                commands.entity(entity).insert(Button);
            } else {
                commands
                    .entity(entity)
                    .remove::<Button>()
                    .insert(Interaction::None);
            }
        }
        color.0 = if enabled {
            BUTTON
        } else {
            Color::srgb(0.08, 0.10, 0.13)
        };
    }
    if ui.view == View::Closed {
        return;
    }
    let content = if ui.view == View::Statistics {
        poker_lab::statistics::report(&learning.model)
    } else {
        review_text(&session.reviews, ui.selection)
    };
    for mut text in &mut texts {
        if text.0 != content {
            text.0 = content.clone();
        }
    }
}

fn cards(cards: impl IntoIterator<Item = poker_lab::poker::cards::Card>) -> String {
    cards
        .into_iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("  ")
}
fn review_text(history: &poker_lab::review::ReviewHistory, selection: Selection) -> String {
    let Some(frame) = selection.frame(history) else {
        return "HAND REVIEW\n\nComplete a hand to review it here.\nThe last 20 completed hands in this match are retained.\nOpponent cards stay hidden unless publicly revealed.".into();
    };
    let hand = &history.hands()[selection.hand];
    let description = match frame.moment {
        Moment::Start => "Hand begins (before blinds)".into(),
        Moment::Blind { seat, amount } => format!("{} posts {amount}", display_name(seat)),
        Moment::YourCards => "Your hole cards are dealt".into(),
        Moment::Action { seat, action, paid } => {
            format!("{}: {action}  /  {paid} chips paid", display_name(seat))
        }
        Moment::Board => format!("{} dealt", frame.phase.label()),
        Moment::Refund { seat, amount } => {
            format!("{amount} uncalled chips returned to {}", display_name(seat))
        }
        Moment::Reveal => "Remaining hands publicly revealed".into(),
        Moment::Award { seat, amount } => format!("{} receives {amount} chips", display_name(seat)),
        Moment::Complete => {
            "Hand complete. Final awards below include all main/side-pot shares.".into()
        }
    };
    let mut lines = vec![
        format!(
            "HAND #{}  /  Review {} of {}  /  Step {} of {}\nDealer: {}  /  Blinds {} / {}",
            hand.number,
            selection.hand + 1,
            history.hands().len(),
            selection.step + 1,
            hand.frames.len(),
            display_name(hand.dealer),
            hand.blinds[0],
            hand.blinds[1]
        ),
        format!(
            "{}  /  Pot remaining: {}\nBoard: {}",
            frame.phase.label(),
            frame.pot,
            if frame.board.is_empty() {
                "Not dealt".into()
            } else {
                cards(frame.board.iter().copied())
            }
        ),
        description,
    ];
    for (i, stack) in frame.stacks.iter().enumerate() {
        let hole = if i == 0 {
            frame.your_cards
        } else {
            frame.shown[i]
        };
        lines.push(format!(
            "{}: {} chips  /  Street wager {}  /  {}{}",
            display_name(Seat::ALL[i]),
            stack,
            frame.street_bets[i],
            hole.map(cards).unwrap_or_else(|| "Cards unshown".into()),
            if frame.folded[i] { "  /  FOLDED" } else { "" }
        ));
    }
    if let Some(outcome) = &frame.outcome {
        lines.push(format!("Total awarded: {}", outcome.pot));
        for (i, pot) in outcome.pots.iter().enumerate() {
            let awards = pot
                .awards
                .iter()
                .enumerate()
                .filter(|(_, a)| **a > 0)
                .map(|(seat, a)| format!("{} +{a}", display_name(Seat::ALL[seat])))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "{} {}: {awards}",
                if i == 0 {
                    "Main pot".into()
                } else {
                    format!("Side pot {i}")
                },
                pot.pot.amount
            ));
        }
    }
    lines.join("\n\n")
}
