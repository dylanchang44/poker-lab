use bevy::prelude::*;
use poker_lab::{
    characters::{PresentationEvent, PresentationState},
    conversation::{ConversationCue, Speaker},
    npc::{
        BasicNpc, PersonalityStrategy, Strategy,
        profiles::{PROFILES, display_name},
    },
    poker::{Action, FourPlayerMatch, PokerMatch, Seat, events::GameEvent},
};
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};
mod match_engine;
use match_engine::MatchEngine;

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum TableMode {
    HeadsUp,
    #[default]
    Four,
}

#[derive(Resource, Clone, Copy)]
pub struct NpcSettings {
    pub delay_seconds: f32,
    pub equity_samples: u32,
}
impl Default for NpcSettings {
    fn default() -> Self {
        Self {
            delay_seconds: 0.55,
            equity_samples: 192,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TurnToken {
    session: u64,
    revision: usize,
    seat: Seat,
}
struct Decision {
    token: TurnToken,
    strategy: PersonalityStrategy,
    action: Option<Action>,
}
struct PendingDecision {
    receiver: Mutex<mpsc::Receiver<Decision>>,
}
static SESSION_ID: AtomicU64 = AtomicU64::new(1);

// Screen navigation is Bevy state; betting phases live in the independent engine.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
}

#[derive(Resource, Default)]
pub struct MatchSeed(pub Option<u64>);

#[derive(Resource)]
pub struct GameSession {
    pub engine: MatchEngine,
    pub mode: TableMode,
    npc: BasicNpc,
    strategies: [PersonalityStrategy; 4],
    pending: Option<PendingDecision>,
    id: u64,
    delay: Timer,
    pub dirty: bool,
    pub bet_input: String,
    pub editing: bool,
    pub replace_on_type: bool,
    pub error: Option<String>,
    pub feedback: VecDeque<String>,
    pub conversation_cues: VecDeque<ConversationCue>,
    pub public_winners: VecDeque<String>,
    pub social_events: VecDeque<(usize, poker_lab::social::PublicSocialEvent)>,
    pub presentation: PresentationState,
    event_cursor: usize,
    hand_start_stacks: Vec<u32>,
    opponent_observer: poker_lab::npc::opponent::Observer,
    pub completed_observations: VecDeque<(u64, poker_lab::npc::opponent::HandSample)>,
    pub reads: [poker_lab::npc::adaptation::Adaptation; 3],
    pub reviews: poker_lab::review::ReviewHistory,
}

impl GameSession {
    pub fn new(seed: u64, mode: TableMode, settings: NpcSettings) -> Self {
        let mut engine = match mode {
            TableMode::HeadsUp => MatchEngine::HeadsUp(Box::new(PokerMatch::new(seed))),
            TableMode::Four => MatchEngine::Four(Box::new(FourPlayerMatch::new(seed))),
        };
        engine.start_next_hand().expect("new match can start");
        let id = SESSION_ID.fetch_add(1, Ordering::Relaxed);
        let mut session = Self {
            engine,
            mode,
            npc: BasicNpc::new(seed ^ 0xa53c_91e7_1122_3344),
            strategies: std::array::from_fn(|i| {
                PersonalityStrategy::new(
                    PROFILES[i].personality,
                    seed ^ (i as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15),
                    settings.equity_samples,
                )
            }),
            pending: None,
            id,
            delay: Timer::from_seconds(settings.delay_seconds.max(0.0), TimerMode::Once),
            dirty: true,
            bet_input: String::new(),
            editing: false,
            replace_on_type: true,
            error: None,
            feedback: VecDeque::new(),
            conversation_cues: VecDeque::new(),
            public_winners: VecDeque::new(),
            social_events: VecDeque::new(),
            event_cursor: 0,
            hand_start_stacks: Vec::new(),
            opponent_observer: Default::default(),
            completed_observations: VecDeque::new(),
            reads: std::array::from_fn(|_| Default::default()),
            reviews: Default::default(),
            presentation: PresentationState {
                session: id,
                ..Default::default()
            },
        };
        session.refresh();
        session
    }

    pub fn submit(&mut self, seat: Seat, action: Action) {
        match self.engine.act(seat, action) {
            Ok(()) => self.refresh(),
            Err(error) => {
                self.error = Some(error.into());
                self.dirty = true;
            }
        }
    }

    pub fn next_hand(&mut self) {
        match self.engine.start_next_hand() {
            Ok(()) => {
                self.feedback.clear();
                self.refresh();
            }
            Err(error) => {
                self.error = Some(error.into());
                self.dirty = true;
            }
        }
    }

    fn refresh(&mut self) {
        // A dropped receiver makes any in-flight old result harmless.
        self.pending = None;
        self.dirty = true;
        self.editing = false;
        self.error = None;
        self.delay.reset();
        self.bet_input = self
            .engine
            .legal_actions(Seat::Human)
            .wager
            .map(|r| r.min_to.to_string())
            .unwrap_or_default();
        // Consume only public events into the live activity list. Private deal/burn
        // events stay in the engine's replay history and never enter UI text.
        for (offset, event) in self.engine.history()[self.event_cursor..]
            .iter()
            .enumerate()
        {
            let event_id = self.event_cursor + offset + 1;
            self.reviews.observe(event);
            if let Some(sample) = self.opponent_observer.observe(event) {
                self.completed_observations.push_back(sample);
            }
            if let GameEvent::HandStarted { stacks, .. } = event {
                self.hand_start_stacks = stacks.clone();
            }
            if let Some(public) = presentation_event(event) {
                if let PresentationEvent::Settled {
                    pot,
                    awards,
                    shown,
                    stacks,
                } = &public
                {
                    self.social_events.push_back((
                        event_id,
                        poker_lab::social::PublicSocialEvent::Settled {
                            pot: *pot,
                            awards: awards.clone(),
                            shown: shown.clone(),
                            net: stacks
                                .iter()
                                .zip(&self.hand_start_stacks)
                                .map(|(after, before)| i64::from(*after) - i64::from(*before))
                                .collect(),
                        },
                    ));
                }
                self.presentation.apply(&public);
            }
            if let Some(cue) = conversation_cue(event_id, event) {
                self.conversation_cues.push_back(cue);
            }
            if let GameEvent::PotAwarded { seat, amount } = event {
                self.public_winners
                    .push_back(format!("{} won {amount} chips", display_name(*seat)));
                while self.public_winners.len() > 3 {
                    self.public_winners.pop_front();
                }
            }
            let text = match event {
                GameEvent::BlindPosted { seat, amount } => {
                    Some(format!("{}: blind {amount}", display_name(*seat)))
                }
                GameEvent::PlayerActed {
                    seat, action, paid, ..
                } => Some(format!("{}: {action} (+{paid})", display_name(*seat))),
                GameEvent::CommunityCardsDealt { phase, cards } => Some(format!(
                    "{}: {}",
                    phase.label(),
                    cards
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                )),
                GameEvent::UncalledBetReturned { seat, amount } => Some(format!(
                    "{}: {amount} uncalled chips returned",
                    display_name(*seat)
                )),
                GameEvent::PotAwarded { seat, amount } => {
                    Some(format!("{}: awarded {amount}", display_name(*seat)))
                }
                _ => None,
            };
            if let Some(text) = text {
                self.feedback.push_back(text);
            }
        }
        self.event_cursor = self.engine.history().len();
        self.presentation.set_actor(self.engine.actor());
        while self.feedback.len() > 3 {
            self.feedback.pop_front();
        }
        while self.conversation_cues.len() > 8 {
            self.conversation_cues.pop_front();
        }
        while self.social_events.len() > 8 {
            self.social_events.pop_front();
        }
    }
}

fn conversation_cue(event_id: usize, event: &GameEvent) -> Option<ConversationCue> {
    let (speaker, priority) = match event {
        GameEvent::HandStarted { .. } => (Speaker::Freya, 1),
        GameEvent::PlayerActed {
            seat: Seat::Human,
            action: Action::BetTo(to) | Action::RaiseTo(to),
            ..
        } if *to >= 100 => (Speaker::Ananya, 2),
        GameEvent::PlayerActed {
            seat: Seat::Human,
            action: Action::AllIn,
            ..
        } => (Speaker::Freya, 3),
        GameEvent::ShowdownStarted { .. } => (Speaker::Yuna, 2),
        GameEvent::PotAwarded { seat, amount } if *amount >= 150 => (
            if *seat == Seat::Human {
                Speaker::Freya
            } else {
                Speaker::from_seat(*seat)
            },
            2,
        ),
        GameEvent::PlayerEliminated { seat } if *seat == Seat::Human => (Speaker::Ananya, 3),
        _ => return None,
    };
    Some(ConversationCue {
        event_id,
        speaker,
        priority,
    })
}

/// The trusted host strips private fields before anything reaches presentation.
fn presentation_event(event: &GameEvent) -> Option<PresentationEvent> {
    match event {
        GameEvent::HandStarted { number, stacks, .. } => Some(PresentationEvent::HandStarted {
            number: *number,
            stacks: stacks.clone(),
        }),
        GameEvent::PlayerActed { seat, action, .. } => Some(PresentationEvent::Acted {
            seat: *seat,
            action: *action,
        }),
        GameEvent::HandCompleted { outcome, stacks } => Some(PresentationEvent::Settled {
            pot: outcome.pot,
            awards: outcome.awards.clone(),
            stacks: stacks.clone(),
            shown: outcome
                .hands
                .as_ref()
                .map(|h| h.iter().map(Option::is_some).collect())
                .unwrap_or_else(|| vec![false; stacks.len()]),
        }),
        _ => None,
    }
}

pub fn start_match(
    mut commands: Commands,
    seed: Res<MatchSeed>,
    mode: Res<TableMode>,
    settings: Res<NpcSettings>,
) {
    commands.insert_resource(GameSession::new(
        seed.0.unwrap_or_else(rand::random),
        *mode,
        *settings,
    ));
}

pub fn end_match(mut commands: Commands) {
    commands.remove_resource::<GameSession>();
}

pub fn npc_turn(time: Res<Time>, mut session: ResMut<GameSession>) {
    let Some(seat) = session.engine.actor() else {
        return;
    };
    if seat == Seat::Human || !session.engine.phase().is_betting() {
        return;
    }
    session.delay.tick(time.delta());
    if session.mode == TableMode::HeadsUp {
        if !session.delay.is_finished() {
            return;
        }
        let view = session.engine.observe(seat);
        if let Some(action) = session.npc.decide(&view) {
            session.submit(seat, action);
        }
        return;
    }
    if session.pending.is_none() {
        let token = session.token(seat);
        let view = session.engine.observe(seat);
        let mut strategy = session.strategies[seat.index()].clone();
        let (sender, receiver) = mpsc::channel();
        // Only an owned, filtered snapshot crosses this thread boundary.
        // No renderer, engine or hidden event stream is available to the worker.
        match std::thread::Builder::new()
            .name("poker-decision".into())
            .spawn(move || {
                let action = strategy.decide(&view);
                let _ = sender.send(Decision {
                    token,
                    strategy,
                    action,
                });
            }) {
            Ok(_) => {
                session.pending = Some(PendingDecision {
                    receiver: Mutex::new(receiver),
                })
            }
            Err(error) => {
                session.fallback(format!("NPC worker unavailable: {error}"));
                return;
            }
        }
    }
    if !session.delay.is_finished() {
        return;
    }
    let received = session
        .pending
        .as_ref()
        .unwrap()
        .receiver
        .lock()
        .expect("receiver lock")
        .try_recv();
    match received {
        Ok(decision) => session.apply_decision(decision),
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => session.fallback("NPC worker stopped".into()),
    }
}

impl GameSession {
    pub fn snapshot_strategy(&mut self, model: &poker_lab::npc::opponent::OpponentModel) {
        for (i, npc) in poker_lab::npc::profiles::NpcId::ALL.into_iter().enumerate() {
            self.reads[i] = poker_lab::npc::adaptation::Adaptation::for_player(npc, model);
            self.strategies[npc.seat().index()].adaptation = self.reads[i].clone();
        }
    }
    fn token(&self, seat: Seat) -> TurnToken {
        TurnToken {
            session: self.id,
            revision: self.engine.history().len(),
            seat,
        }
    }
    fn apply_decision(&mut self, decision: Decision) {
        if self.engine.actor() != Some(decision.token.seat)
            || self.token(decision.token.seat) != decision.token
        {
            return;
        }
        self.pending = None;
        let seat = decision.token.seat;
        self.strategies[seat.index()] = decision.strategy;
        if let Some(action) = decision
            .action
            .filter(|a| self.engine.legal_actions(seat).accepts(*a))
        {
            self.submit(seat, action);
        } else {
            self.fallback("NPC proposed an invalid action".into());
        }
    }
    fn fallback(&mut self, message: String) {
        warn!("{message}");
        if let Some(seat) = self.engine.actor() {
            let legal = self.engine.legal_actions(seat);
            self.submit(
                seat,
                if legal.check {
                    Action::Check
                } else {
                    Action::Fold
                },
            );
            self.error = Some(format!("{message}; used a safe legal action."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_filter_discards_private_deals_burns_and_shuffle_seed() {
        let mut first = PokerMatch::new(1);
        let mut second = PokerMatch::new(999);
        first.start_next_hand().unwrap();
        second.start_next_hand().unwrap();
        let public_a: Vec<_> = first
            .history()
            .iter()
            .filter_map(presentation_event)
            .collect();
        let public_b: Vec<_> = second
            .history()
            .iter()
            .filter_map(presentation_event)
            .collect();
        assert_eq!(public_a, public_b); // Different hidden cards/seed, identical presentation.
        for event in first.history() {
            if matches!(
                event,
                GameEvent::CardsDealt { .. } | GameEvent::CardBurned { .. }
            ) {
                assert!(presentation_event(event).is_none());
            }
        }
        let card = "As".parse().unwrap();
        assert!(presentation_event(&GameEvent::CardBurned { card }).is_none());
        let before = format!("{first:?}");
        let mut model = PresentationState::default();
        model.reset(1);
        for event in public_a {
            model.apply(&event);
        }
        model.tick(100.0);
        assert_eq!(format!("{first:?}"), before);
    }

    #[test]
    fn delayed_decisions_cannot_cross_turns_or_restarts() {
        let mut game = GameSession::new(42, TableMode::Four, NpcSettings::default());
        let seat = game.engine.actor().unwrap();
        let old = game.token(seat);
        game.submit(seat, Action::Fold);
        let snapshot = game.engine.history().to_vec();
        game.apply_decision(Decision {
            token: old,
            strategy: game.strategies[seat.index()].clone(),
            action: Some(Action::AllIn),
        });
        assert_eq!(game.engine.history(), snapshot);
        let mut fresh = GameSession::new(42, TableMode::Four, NpcSettings::default());
        let snapshot = fresh.engine.history().to_vec();
        fresh.apply_decision(Decision {
            token: old,
            strategy: game.strategies[seat.index()].clone(),
            action: Some(Action::AllIn),
        });
        assert_eq!(fresh.engine.history(), snapshot);
    }

    #[test]
    fn invalid_strategy_proposal_uses_only_validated_safe_fallback() {
        let mut game = GameSession::new(42, TableMode::Four, NpcSettings::default());
        let seat = game.engine.actor().unwrap();
        let token = game.token(seat);
        game.apply_decision(Decision {
            token,
            strategy: game.strategies[seat.index()].clone(),
            action: Some(Action::RaiseTo(u32::MAX)),
        });
        assert_eq!(
            game.engine.observe(seat).last_actions[seat.index()],
            Some(Action::Fold)
        );
        assert!(game.error.is_some());
        assert_eq!(
            game.engine.stacks().iter().sum::<u32>() + game.engine.observe(seat).pot,
            4000
        );
    }

    #[test]
    fn application_starts_in_main_menu() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
    }
}
