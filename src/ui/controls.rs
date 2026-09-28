use super::{BUTTON, BUTTON_HOVERED, BUTTON_PRESSED};
use crate::game::{AppState, GameSession, MatchSeed, NpcSettings};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use poker_lab::poker::{Action, Seat};

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Act(Action),
    Wager,
    EditAmount,
    Minimum,
    HalfPot,
    Pot,
    Maximum,
    NextHand,
    NewMatch,
    Menu,
}

type ButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static Control,
        &'static mut BackgroundColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

pub fn buttons(
    mut buttons: ButtonQuery,
    mut session: ResMut<GameSession>,
    seed: Res<MatchSeed>,
    settings: Res<NpcSettings>,
    mut next: ResMut<NextState<AppState>>,
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
        match *control {
            Control::Act(action) => session.submit(Seat::Human, action),
            Control::Wager => submit_wager(&mut session),
            Control::EditAmount => {
                session.editing = true;
                session.replace_on_type = true;
                session.dirty = true;
            }
            Control::Minimum | Control::HalfPot | Control::Pot | Control::Maximum => {
                let view = session.engine.observe(Seat::Human);
                if let Some(range) = view.legal.wager {
                    let to = match control {
                        Control::Minimum => range.min_to,
                        Control::Maximum => range.max_to,
                        Control::HalfPot => {
                            view.street_bets[0] + view.to_call + (view.pot + view.to_call) / 2
                        }
                        _ => view.street_bets[0] + view.to_call + view.pot + view.to_call,
                    }
                    .clamp(range.min_to, range.max_to);
                    session.bet_input = to.to_string();
                    session.editing = false;
                    session.dirty = true;
                }
            }
            Control::NextHand => session.next_hand(),
            Control::NewMatch => {
                *session =
                    GameSession::new(seed.0.unwrap_or_else(rand::random), session.mode, *settings)
            }
            Control::Menu => next.set(AppState::MainMenu),
        }
        // One button press should produce one command, even if entities are stale.
        break;
    }
}

fn submit_wager(session: &mut GameSession) {
    if let Some(range) = session.engine.legal_actions(Seat::Human).wager
        && let Ok(to) = session.bet_input.parse::<u32>()
    {
        session.submit(
            Seat::Human,
            if range.is_raise {
                Action::RaiseTo(to)
            } else {
                Action::BetTo(to)
            },
        );
    }
}

pub fn keyboard(mut events: MessageReader<KeyboardInput>, mut session: ResMut<GameSession>) {
    for event in events.read() {
        if !session.editing || event.state != ButtonState::Pressed {
            continue;
        }
        let Some(range) = session.engine.legal_actions(Seat::Human).wager else {
            continue;
        };
        match &event.logical_key {
            Key::Character(text) if text.chars().all(|c| c.is_ascii_digit()) => {
                if session.replace_on_type {
                    session.bet_input.clear();
                    session.replace_on_type = false;
                }
                for digit in text.chars() {
                    let proposed = format!("{}{digit}", session.bet_input);
                    if proposed.parse::<u32>().is_ok_and(|n| n <= range.max_to) {
                        session.bet_input = proposed;
                    }
                }
            }
            Key::Backspace => {
                session.bet_input.pop();
                session.replace_on_type = false;
            }
            Key::Enter => {
                let value = session
                    .bet_input
                    .parse::<u32>()
                    .unwrap_or(range.min_to)
                    .clamp(range.min_to, range.max_to);
                session.bet_input = value.to_string();
                session.editing = false;
            }
            Key::Escape => {
                session.bet_input = range.min_to.to_string();
                session.editing = false;
            }
            _ => continue,
        }
        session.dirty = true;
    }
}
