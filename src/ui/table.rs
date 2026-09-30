use super::{BUTTON, FELT, GOLD, MUTED, ScreenRoot, TEXT, controls::Control, label};
use crate::game::GameSession;
use bevy::prelude::*;
use poker_lab::npc::profiles::display_name;
use poker_lab::poker::{
    Action, Observation, Phase, Seat,
    cards::{Card, Suit},
};

pub(super) fn column(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(gap),
        ..default()
    }
}
pub(super) fn row(gap: f32) -> Node {
    Node {
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        column_gap: Val::Px(gap),
        ..default()
    }
}

#[derive(Component)]
pub struct TableCanvas;
#[derive(Component)]
pub(super) struct TableContent;

pub fn render(
    mut commands: Commands,
    mut session: ResMut<GameSession>,
    canvases: Query<Entity, With<TableCanvas>>,
    content: Query<Entity, With<TableContent>>,
) {
    if !session.dirty {
        return;
    }
    session.dirty = false;
    let view = session.engine.observe(Seat::Human);
    let canvas = if let Ok(canvas) = canvases.single() {
        canvas
    } else {
        let mut canvas = Entity::PLACEHOLDER;
        commands
            .spawn((
                ScreenRoot,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..column(0.0)
                },
            ))
            .with_children(|root| {
                canvas = root
                    .spawn((
                        TableCanvas,
                        Node {
                            width: Val::Percent(100.0),
                            max_width: Val::Px(1400.0),
                            height: Val::Px(960.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ))
                    .with_children(|table| {
                        // Felt and character entities persist through hands and restarts.
                        table.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(8.0),
                                top: Val::Px(420.0),
                                width: Val::Percent(84.0),
                                height: Val::Px(238.0),
                                border: UiRect::all(Val::Px(7.0)),
                                border_radius: BorderRadius::all(Val::Percent(50.0)),
                                ..default()
                            },
                            BackgroundColor(FELT),
                            BorderColor::all(Color::srgb(0.30, 0.25, 0.17)),
                        ));
                        for &seat in &Seat::ALL[1..view.stacks.len()] {
                            super::characters::spawn(table, seat, view.stacks.len() == 2);
                        }
                        super::characters::spawn_dialogue(table);
                        super::conversation::spawn(table);
                    })
                    .id();
            });
        canvas
    };
    for entity in &content {
        commands.entity(entity).despawn();
    }
    commands.entity(canvas).with_children(|canvas| {
        canvas
            .spawn((
                TableContent,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ZIndex(2),
            ))
            .with_children(|root| {
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(24.0),
                    right: Val::Px(24.0),
                    top: Val::Px(12.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..row(12.0)
                })
                .with_children(|header| {
                    label(
                        header,
                        format!(
                            "POKER LAB  /  Hand {}  /  {}",
                            view.hand_number,
                            view.phase.label()
                        ),
                        22.0,
                        GOLD,
                    );
                    header.spawn(row(8.0)).with_children(|buttons| {
                        control(buttons, "New Match", Control::NewMatch, true, 130.0);
                        control(buttons, "Back to Menu", Control::Menu, true, 150.0);
                    });
                });
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    margin: UiRect::left(Val::Px(-260.0)),
                    top: Val::Px(455.0),
                    ..default()
                })
                .with_children(|center| board(center, &view));
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    margin: UiRect::left(Val::Px(-350.0)),
                    width: Val::Px(700.0),
                    top: Val::Px(709.0),
                    ..row(20.0)
                })
                .with_children(|human| player(human, &view, Seat::Human));
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(793.0),
                    width: Val::Percent(100.0),
                    ..row(0.0)
                })
                .with_children(|status_node| {
                    label(
                        status_node,
                        session.error.clone().unwrap_or_else(|| status(&view)),
                        18.0,
                        GOLD,
                    );
                });
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(824.0),
                    width: Val::Percent(100.0),
                    ..row(0.0)
                })
                .with_children(|actions| controls(actions, &view, &session));
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(942.0),
                    width: Val::Percent(100.0),
                    ..row(0.0)
                })
                .with_children(|footer| {
                    label(
                        footer,
                        session
                            .feedback
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("  /  "),
                        11.0,
                        MUTED,
                    );
                });
            });
    });
}

fn board(parent: &mut ChildSpawnerCommands, view: &Observation) {
    parent
        .spawn((
            Node {
                width: Val::Px(520.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::ZERO,
                border_radius: BorderRadius::all(Val::Px(45.0)),
                ..column(6.0)
            },
            BackgroundColor(FELT),
            BorderColor::all(Color::srgb(0.19, 0.40, 0.34)),
        ))
        .with_children(|felt| {
            felt.spawn(row(10.0)).with_children(|cards| {
                for i in 0..5 {
                    card(cards, view.board.get(i).copied(), false);
                }
            });
            label(
                felt,
                view.outcome
                    .as_ref()
                    .map(|r| format!("POT AWARDED  {}", r.pot))
                    .unwrap_or_else(|| format!("POT  {}", view.pot)),
                23.0,
                TEXT,
            );
            if let Some(result) = &view.outcome {
                for (index, pot) in result.pots.iter().enumerate() {
                    let awards = pot
                        .awards
                        .iter()
                        .enumerate()
                        .filter(|(_, amount)| **amount > 0)
                        .map(|(i, amount)| format!("{} +{amount}", display_name(Seat::ALL[i])))
                        .collect::<Vec<_>>()
                        .join(", ");
                    label(
                        felt,
                        format!("{} {}: {awards}", pot_name(index), pot.pot.amount),
                        13.0,
                        TEXT,
                    );
                }
            } else if view
                .in_hand
                .iter()
                .enumerate()
                .any(|(i, live)| *live && view.stacks[i] == 0)
            {
                label(
                    felt,
                    view.pots
                        .iter()
                        .enumerate()
                        .map(|(i, p)| format!("{} {}", pot_name(i), p.amount))
                        .collect::<Vec<_>>()
                        .join(" / "),
                    13.0,
                    MUTED,
                );
                label(
                    felt,
                    "Pot layers provisional until bets settle",
                    11.0,
                    MUTED,
                );
            } else {
                label(
                    felt,
                    format!("Blinds {} / {}", view.blinds[0], view.blinds[1]),
                    13.0,
                    MUTED,
                );
                label(felt, "Pot includes all chips in front", 11.0, MUTED);
            }
        });
}
fn pot_name(index: usize) -> String {
    if index == 0 {
        "Main".into()
    } else {
        format!("Side {index}")
    }
}

fn player(parent: &mut ChildSpawnerCommands, view: &Observation, seat: Seat) {
    let i = seat.index();
    parent.spawn(column(4.0)).with_children(|info| {
        label(
            info,
            format!("YOU   /   {} chips", view.stacks[i]),
            19.0,
            GOLD,
        );
        let mut badges = Vec::new();
        if view.dealer == seat {
            badges.push("D");
        }
        if view.small_blind == Some(seat) {
            badges.push("SB");
        }
        if view.big_blind == seat {
            badges.push("BB");
        }
        if view.eliminated[i] {
            badges.push("OUT");
        } else if view.folded[i] {
            badges.push("FOLDED");
        }
        label(
            info,
            format!(
                "{}   /   In front: {}",
                badges.join(" / "),
                view.street_bets[i]
            ),
            13.0,
            MUTED,
        );
    });
    for index in 0..2 {
        card(parent, view.hole_cards.map(|h| h[index]), false);
    }
    parent.spawn(column(4.0)).with_children(|info| {
        label(
            info,
            view.last_actions[i]
                .map(|a| a.to_string())
                .unwrap_or_default(),
            14.0,
            TEXT,
        );
        if let Some(hand) = view
            .outcome
            .as_ref()
            .and_then(|o| o.hands.as_ref())
            .and_then(|h| h[i])
        {
            label(info, hand.category(), 13.0, MUTED);
        }
    });
}

fn card(parent: &mut ChildSpawnerCommands, card: Option<Card>, hidden: bool) {
    let background = if card.is_some() {
        Color::srgb(0.92, 0.93, 0.90)
    } else {
        Color::srgb(0.065, 0.12, 0.18)
    };
    parent
        .spawn((
            Node {
                width: Val::Px(58.0),
                height: Val::Px(76.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..column(3.0)
            },
            BackgroundColor(background),
            BorderColor::all(Color::srgb(0.32, 0.43, 0.46)),
        ))
        .with_children(|node| {
            let color = if card.is_some_and(|c| c.suit.is_red()) {
                Color::srgb(0.72, 0.12, 0.18)
            } else {
                Color::srgb(0.08, 0.13, 0.2)
            };
            if let Some(card) = card {
                label(node, card.to_string(), 27.0, color);
                let suit = match card.suit {
                    Suit::Clubs => "CLUBS",
                    Suit::Diamonds => "DIAMONDS",
                    Suit::Hearts => "HEARTS",
                    Suit::Spades => "SPADES",
                };
                label(node, suit, 9.0, color);
            } else {
                label(node, if hidden { "?" } else { "-" }, 26.0, MUTED);
            }
        });
}

fn status(view: &Observation) -> String {
    if let Some(result) = &view.outcome {
        let awards = result
            .awards
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(i, n)| format!("{} +{n}", display_name(Seat::ALL[i])))
            .collect::<Vec<_>>()
            .join(" / ");
        if view.phase == Phase::MatchComplete {
            let winner = view.stacks.iter().position(|n| *n > 0).unwrap();
            format!(
                "{} wins the match!  {awards}",
                display_name(Seat::ALL[winner])
            )
        } else {
            format!("Hand complete  /  {awards}")
        }
    } else if view.actor == Some(Seat::Human) {
        if view.to_call > 0 {
            format!("Your turn   /   {} to call", view.legal.call.unwrap_or(0))
        } else {
            "Your turn   /   Check or bet".into()
        }
    } else {
        format!(
            "{} is thinking...{}",
            view.actor.map(display_name).unwrap_or("Table"),
            if view.eliminated[0] {
                "  /  You are out; spectating"
            } else {
                ""
            }
        )
    }
}

fn controls(parent: &mut ChildSpawnerCommands, view: &Observation, session: &GameSession) {
    if matches!(view.phase, Phase::HandComplete | Phase::MatchComplete) {
        parent.spawn(row(12.0)).with_children(|row| {
            if view.phase == Phase::HandComplete {
                control(row, "Next Hand", Control::NextHand, true, 160.0);
            }
            if view.phase == Phase::MatchComplete {
                label(
                    row,
                    "New Match starts a fresh table; Back to Menu leaves the table.",
                    14.0,
                    MUTED,
                );
            }
        });
        return;
    }
    parent.spawn(column(8.0)).with_children(|panel| {
        if let Some(range)=view.legal.wager {
            panel.spawn(row(8.0)).with_children(|row| {
                label(row,format!("{} to  [{}-{}]",if range.is_raise {"Raise"} else {"Bet"},range.min_to,range.max_to),15.0,MUTED);
                control(row,&format!("{}{}",session.bet_input,if session.editing {" |"} else {""}),Control::EditAmount,true,110.0);
                for (name,action) in [("Min",Control::Minimum),("1/2",Control::HalfPot),("Pot",Control::Pot),("Max",Control::Maximum)] {control(row,name,action,true,70.0);}
            });
            label(panel,if session.editing {"Type chips; Backspace edits; Enter confirms amount; Escape resets. Then click Bet / Raise."} else {"Click the amount to type. This is your TOTAL bet on this street."},12.0,MUTED);
        }
        panel.spawn(row(10.0)).with_children(|row| {
            control(row,"Fold",Control::Act(Action::Fold),view.legal.fold,120.0);
            if view.legal.check {control(row,"Check",Control::Act(Action::Check),true,140.0);}
            else {control(row,&view.legal.call.map(|n|format!("Call {n}")).unwrap_or_else(||"Check / Call".into()),Control::Act(Action::Call),view.legal.call.is_some(),140.0);}
            let wager=view.legal.wager;
            let valid=wager.is_some_and(|r|session.bet_input.parse::<u32>().is_ok_and(|n|(r.min_to..=r.max_to).contains(&n)));
            control(row,if wager.is_some_and(|r|r.is_raise) {"Raise"} else {"Bet"},Control::Wager,valid,120.0);
            control(row,&format!("All-in {}",view.stacks[0]),Control::Act(Action::AllIn),view.legal.all_in,150.0);
        });
    });
}

fn control(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    control: Control,
    enabled: bool,
    width: f32,
) {
    let mut entity = parent.spawn((
        Node {
            width: Val::Px(width),
            height: Val::Px(40.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(if enabled {
            BUTTON
        } else {
            Color::srgb(0.10, 0.14, 0.18)
        }),
    ));
    // Disabled controls have no Button or Control components and cannot dispatch.
    if enabled {
        entity.insert((Button, control));
    }
    entity.with_children(|parent| label(parent, text, 17.0, if enabled { TEXT } else { MUTED }));
}
