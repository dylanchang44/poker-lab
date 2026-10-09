use super::{
    cards::Card,
    deck::Deck,
    events::GameEvent,
    hand::{HandValue, evaluate},
    pots,
    state::*,
};

#[cfg(test)]
#[path = "multiway_tests.rs"]
mod multiway_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// The same rules implementation backs the Stage 1 and four-seat configurations.
pub type PokerMatch = Table<2>;
pub type FourPlayerMatch = Table<4>;

#[derive(Clone, Debug)]
pub struct Table<const N: usize> {
    config: Config<N>,
    seed: u64,
    hand_number: u64,
    dealer: Seat,
    small_blind_position: Seat,
    small_blind: Option<Seat>,
    big_blind: Seat,
    phase: Phase,
    actor: Option<Seat>,
    stacks: [Chips; N],
    street_bets: [Chips; N],
    contributions: [Chips; N],
    current_bet: Chips,
    last_full_raise: Chips,
    pending: [bool; N],
    raise_right: [bool; N],
    acted_at: [Option<Chips>; N],
    dealt: [bool; N],
    folded: [bool; N],
    deck: Deck,
    holes: Option<[[Card; 2]; N]>,
    board: Vec<Card>,
    revealed: bool,
    last_actions: [Option<Action>; N],
    public_history: Vec<PublicAction>,
    outcome: Option<Outcome>,
    events: Vec<GameEvent>,
}

impl<const N: usize> Table<N> {
    pub fn new(seed: u64) -> Self {
        Self::with_config(seed, Config::default()).expect("default config is valid")
    }

    pub fn with_config(seed: u64, config: Config<N>) -> Result<Self, &'static str> {
        if !(2..=4).contains(&N)
            || config.starting_stacks.iter().filter(|n| **n > 0).count() < 2
            || config.small_blind == 0
            || config.small_blind > config.big_blind
            || config
                .starting_stacks
                .iter()
                .map(|n| u64::from(*n))
                .sum::<u64>()
                > u64::from(u32::MAX)
            || config.big_blind > u32::MAX / 2
        {
            return Err("Invalid stacks or blinds");
        }
        Ok(Self {
            config,
            seed,
            hand_number: 0,
            dealer: Seat::Human,
            small_blind_position: Seat::Human,
            small_blind: None,
            big_blind: Seat::Npc,
            phase: Phase::WaitingForHand,
            actor: None,
            stacks: config.starting_stacks,
            street_bets: [0; N],
            contributions: [0; N],
            current_bet: 0,
            last_full_raise: config.big_blind,
            pending: [false; N],
            raise_right: [true; N],
            acted_at: [None; N],
            dealt: [false; N],
            folded: [false; N],
            deck: Deck::shuffled(seed),
            holes: None,
            board: Vec::new(),
            revealed: false,
            last_actions: [None; N],
            public_history: Vec::new(),
            outcome: None,
            events: Vec::new(),
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn actor(&self) -> Option<Seat> {
        self.actor
    }
    pub fn pot(&self) -> Chips {
        self.contributions.iter().sum()
    }
    pub fn stacks(&self) -> [Chips; N] {
        self.stacks
    }
    /// Privileged replay stream. Strategies must only receive observe().
    pub fn history(&self) -> &[GameEvent] {
        &self.events
    }

    fn next_where(&self, from: Seat, predicate: impl Fn(usize) -> bool) -> Option<Seat> {
        (1..=N)
            .map(|offset| (from.index() + offset) % N)
            .find(|i| predicate(*i))
            .map(|i| Seat::ALL[i])
    }
    fn live(&self) -> [bool; N] {
        std::array::from_fn(|i| self.dealt[i] && !self.folded[i])
    }
    fn can_act(&self, i: usize) -> bool {
        self.dealt[i] && !self.folded[i] && self.stacks[i] > 0
    }
    fn active_count(&self) -> usize {
        self.live().iter().filter(|b| **b).count()
    }

    pub fn start_next_hand(&mut self) -> Result<(), &'static str> {
        if !matches!(self.phase, Phase::WaitingForHand | Phase::HandComplete) {
            return Err("Finish this hand or start a new match first");
        }
        let survivors = self.stacks.iter().filter(|n| **n > 0).count();
        if self.hand_number == 0 {
            self.dealer = Seat::ALL[self.stacks.iter().position(|n| *n > 0).unwrap()];
            self.small_blind_position = if survivors == 2 {
                self.dealer
            } else {
                self.next_where(self.dealer, |i| self.stacks[i] > 0)
                    .unwrap()
            };
            self.big_blind = self
                .next_where(self.small_blind_position, |i| self.stacks[i] > 0)
                .unwrap();
        } else {
            // Dead-button rule: advance the BB through surviving seats. A busted
            // previous BB leaves a dead SB for one hand. Keep its button position.
            let old_big = self.big_blind;
            self.big_blind = self.next_where(old_big, |i| self.stacks[i] > 0).unwrap();
            if survivors == 2 {
                self.dealer = self
                    .next_where(self.big_blind, |i| self.stacks[i] > 0)
                    .unwrap();
                self.small_blind_position = self.dealer;
            } else {
                self.dealer = self.small_blind_position;
                self.small_blind_position = old_big;
            }
        }
        self.small_blind = (self.stacks[self.small_blind_position.index()] > 0)
            .then_some(self.small_blind_position);
        self.hand_number += 1;
        let seed = self.seed.wrapping_add(self.hand_number - 1);
        self.deck = Deck::shuffled(seed);
        self.board.clear();
        self.revealed = false;
        self.outcome = None;
        self.last_actions = [None; N];
        self.public_history.clear();
        self.street_bets = [0; N];
        self.contributions = [0; N];
        self.last_full_raise = self.config.big_blind;
        self.raise_right = [true; N];
        self.acted_at = [None; N];
        self.dealt = self.stacks.map(|n| n > 0);
        self.folded = [false; N];
        self.phase = Phase::PreFlop;
        self.events.push(GameEvent::HandStarted {
            number: self.hand_number,
            seed,
            dealer: self.dealer,
            stacks: self.stacks.to_vec(),
            blinds: [self.config.small_blind, self.config.big_blind],
        });
        for (seat, blind) in [
            (self.small_blind, self.config.small_blind),
            (Some(self.big_blind), self.config.big_blind),
        ] {
            if let Some(seat) = seat {
                let amount = blind.min(self.stacks[seat.index()]);
                self.pay(seat, amount);
                self.events.push(GameEvent::BlindPosted { seat, amount });
            }
        }
        // Fill only participating seats. Unused internal slots are never observed.
        let placeholder = Card {
            rank: super::cards::Rank::Two,
            suit: super::cards::Suit::Clubs,
        };
        let mut holes = [[placeholder; 2]; N];
        for round in [0, 1] {
            for offset in 1..=N {
                let i = (self.dealer.index() + offset) % N;
                if self.dealt[i] {
                    holes[i][round] = self.draw();
                }
            }
        }
        self.holes = Some(holes);
        for (i, cards) in holes.into_iter().enumerate() {
            if self.dealt[i] {
                self.events.push(GameEvent::CardsDealt {
                    seat: Seat::ALL[i],
                    cards,
                });
            }
        }
        self.current_bet = *self.street_bets.iter().max().unwrap();
        // A short BB doesn't lower the bring-in when two players can still bet.
        if (0..N).filter(|i| self.can_act(*i)).count() >= 2 {
            self.current_bet = self.current_bet.max(self.config.big_blind);
        }
        self.pending = std::array::from_fn(|i| self.can_act(i));
        self.advance_after(self.big_blind);
        Ok(())
    }

    pub fn legal_actions(&self, seat: Seat) -> LegalActions {
        let i = seat.index();
        if i >= N || self.actor != Some(seat) || !self.phase.is_betting() || !self.can_act(i) {
            return LegalActions::default();
        }
        let owed = self.current_bet.saturating_sub(self.street_bets[i]);
        let max_to = self.street_bets[i] + self.stacks[i];
        let can_raise = self.raise_right[i]
            && (0..N).any(|j| j != i && self.can_act(j))
            && max_to > self.current_bet;
        let min_to = if self.current_bet == 0 {
            self.config.big_blind
        } else {
            self.current_bet.saturating_add(self.last_full_raise)
        };
        LegalActions {
            fold: true,
            check: owed == 0,
            call: (owed > 0).then_some(owed.min(self.stacks[i])),
            wager: (can_raise && max_to >= min_to).then_some(WagerRange {
                min_to,
                max_to,
                is_raise: self.current_bet > 0,
            }),
            all_in: max_to <= self.current_bet || can_raise,
        }
    }

    /// Validation precedes all mutations, including history and turn counters.
    pub fn act(&mut self, seat: Seat, action: Action) -> Result<(), &'static str> {
        if !self.legal_actions(seat).accepts(action) {
            return Err("Action is not legal for this player now");
        }
        let i = seat.index();
        let target = match action {
            Action::Call => self.current_bet.min(self.street_bets[i] + self.stacks[i]),
            Action::BetTo(n) | Action::RaiseTo(n) => n,
            Action::AllIn => self.street_bets[i] + self.stacks[i],
            _ => self.street_bets[i],
        };
        let legal = self.legal_actions(seat);
        self.events.push(GameEvent::DecisionOffered {
            seat,
            phase: self.phase,
            pot: self.pot(),
            to_call: self.current_bet.saturating_sub(self.street_bets[i]),
            street_bet: self.street_bets[i],
            can_raise: legal.wager.is_some()
                || (legal.all_in && self.street_bets[i] + self.stacks[i] > self.current_bet),
        });
        let paid = target - self.street_bets[i];
        self.pay(seat, paid);
        self.pending[i] = false;
        self.raise_right[i] = false;
        self.acted_at[i] = Some(target);
        self.last_actions[i] = Some(action);
        if action == Action::Fold {
            self.folded[i] = true;
        }
        if target > self.current_bet {
            let raise = target - self.current_bet;
            if raise >= self.last_full_raise {
                self.last_full_raise = raise;
            }
            self.current_bet = target;
            for j in 0..N {
                if j != i && self.can_act(j) {
                    self.pending[j] = true;
                    // Several short all-ins can cumulatively reopen a prior actor.
                    self.raise_right[j] = self.acted_at[j]
                        .is_none_or(|at| target.saturating_sub(at) >= self.last_full_raise);
                }
            }
        }
        self.events.push(GameEvent::PlayerActed {
            seat,
            phase: self.phase,
            action,
            paid,
            street_total: self.street_bets[i],
            pot: self.pot(),
            stacks: self.stacks.to_vec(),
        });
        self.public_history.push(PublicAction {
            seat,
            phase: self.phase,
            action,
            paid,
            street_total: self.street_bets[i],
        });
        if self.active_count() == 1 {
            self.refund_uncalled();
            self.finish(None, Some(seat));
        } else {
            self.advance_after(seat);
        }
        Ok(())
    }

    fn exposed_cards(&self) -> Vec<Option<[Card; 2]>> {
        (0..N)
            .map(|i| {
                if self.live()[i] {
                    self.holes.map(|h| h[i])
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn observe(&self, seat: Seat) -> Observation {
        let i = seat.index();
        assert!(i < N, "seat outside table");
        Observation {
            seat,
            hole_cards: if self.dealt[i] {
                self.holes.map(|h| h[i])
            } else {
                None
            },
            revealed_cards: self.revealed.then(|| self.exposed_cards()),
            board: self.board.clone(),
            stacks: self.stacks.to_vec(),
            street_bets: self.street_bets.to_vec(),
            contributions: self.contributions.to_vec(),
            folded: self.folded.to_vec(),
            eliminated: (0..N)
                .map(|j| !self.dealt[j] || (!self.phase.is_betting() && self.stacks[j] == 0))
                .collect(),
            in_hand: self.live().to_vec(),
            pots: pots::construct(&self.contributions, &self.live()),
            small_blind: self.small_blind,
            big_blind: self.big_blind,
            history: self.public_history.clone(),
            pot: self.pot(),
            dealer: self.dealer,
            blinds: [self.config.small_blind, self.config.big_blind],
            phase: self.phase,
            actor: self.actor,
            hand_number: self.hand_number,
            to_call: self.current_bet.saturating_sub(self.street_bets[i]),
            legal: self.legal_actions(seat),
            last_actions: self.last_actions.to_vec(),
            outcome: self.outcome.clone(),
        }
    }

    fn draw(&mut self) -> Card {
        self.deck
            .deal()
            .expect("a four-seat hand uses at most 16 cards")
    }
    fn pay(&mut self, seat: Seat, amount: Chips) {
        let i = seat.index();
        self.stacks[i] -= amount;
        self.street_bets[i] += amount;
        self.contributions[i] += amount;
    }
    fn advance_after(&mut self, seat: Seat) {
        let actors = (0..N).filter(|i| self.can_act(*i)).collect::<Vec<_>>();
        if actors.len() <= 1 {
            let matched = self
                .street_bets
                .iter()
                .enumerate()
                .filter(|(i, _)| self.live()[*i])
                .map(|(_, n)| *n)
                .max()
                .unwrap_or(0);
            self.current_bet = self.current_bet.min(matched);
        }
        for i in 0..N {
            if !self.can_act(i) || (actors.len() == 1 && self.street_bets[i] >= self.current_bet) {
                self.pending[i] = false;
            }
        }
        self.actor = self.next_where(seat, |i| self.pending[i]);
        if self.actor.is_none() {
            self.close_round();
        }
    }
    fn close_round(&mut self) {
        self.events
            .push(GameEvent::BettingRoundCompleted { phase: self.phase });
        self.refund_uncalled();
        if (0..N).filter(|i| self.can_act(*i)).count() <= 1 {
            self.reveal();
            while self.board.len() < 5 {
                self.deal_street();
                self.events
                    .push(GameEvent::BettingRoundCompleted { phase: self.phase });
            }
            self.showdown();
        } else if self.phase == Phase::River {
            self.showdown();
        } else {
            self.deal_street();
            self.street_bets = [0; N];
            self.current_bet = 0;
            self.last_full_raise = self.config.big_blind;
            self.acted_at = [None; N];
            self.raise_right = [true; N];
            self.pending = std::array::from_fn(|i| self.can_act(i));
            self.actor = self.next_where(self.dealer, |i| self.pending[i]);
        }
    }
    fn deal_street(&mut self) {
        self.phase = match self.board.len() {
            0 => Phase::Flop,
            3 => Phase::Turn,
            4 => Phase::River,
            _ => unreachable!(),
        };
        let card = self.draw();
        self.events.push(GameEvent::CardBurned { card });
        let count = if self.phase == Phase::Flop { 3 } else { 1 };
        let cards: Vec<_> = (0..count).map(|_| self.draw()).collect();
        self.board.extend_from_slice(&cards);
        self.events.push(GameEvent::CommunityCardsDealt {
            phase: self.phase,
            cards,
        });
    }
    fn refund_uncalled(&mut self) {
        let mut ordered = self.contributions;
        ordered.sort_unstable();
        let amount = ordered[N - 1] - ordered[N - 2];
        if amount == 0 {
            return;
        }
        let i = self
            .contributions
            .iter()
            .position(|n| *n == ordered[N - 1])
            .unwrap();
        self.contributions[i] -= amount;
        self.street_bets[i] -= amount;
        self.stacks[i] += amount;
        self.events.push(GameEvent::UncalledBetReturned {
            seat: Seat::ALL[i],
            amount,
        });
    }
    fn reveal(&mut self) {
        if !self.revealed {
            self.revealed = true;
            self.events.push(GameEvent::ShowdownStarted {
                cards: self.exposed_cards(),
            });
        }
    }
    fn showdown(&mut self) {
        self.phase = Phase::Showdown;
        self.actor = None;
        self.reveal();
        let values = (0..N)
            .map(|i| {
                if self.live()[i] {
                    let mut cards = self.board.clone();
                    cards.extend(self.holes.unwrap()[i]);
                    Some(evaluate(&cards).expect("dealer produces unique cards"))
                } else {
                    None
                }
            })
            .collect();
        self.finish(Some(values), None);
    }
    fn finish(&mut self, hands: Option<Vec<Option<HandValue>>>, folded: Option<Seat>) {
        let pot = self.pot();
        let mut awards = vec![0; N];
        let mut results = Vec::new();
        for layer in pots::construct(&self.contributions, &self.live()) {
            assert!(
                !layer.eligible.is_empty(),
                "legal betting cannot produce an orphan pot"
            );
            let winners = if let Some(values) = &hands {
                let best = layer
                    .eligible
                    .iter()
                    .map(|s| values[s.index()].unwrap())
                    .max()
                    .unwrap();
                layer
                    .eligible
                    .iter()
                    .copied()
                    .filter(|s| values[s.index()] == Some(best))
                    .collect::<Vec<_>>()
            } else {
                layer.eligible.clone()
            };
            let shares = pots::split(layer.amount, &winners, self.dealer, N);
            for (i, share) in shares.iter().enumerate() {
                awards[i] += share;
            }
            results.push(PotResult {
                pot: layer,
                winners,
                awards: shares,
            });
        }
        let recipients = (0..N).filter(|i| awards[*i] > 0).collect::<Vec<_>>();
        let winner = (recipients.len() == 1).then(|| Seat::ALL[recipients[0]]);
        self.contributions = [0; N];
        self.street_bets = [0; N];
        self.current_bet = 0;
        self.actor = None;
        self.pending = [false; N];
        for (i, amount) in awards.iter().copied().enumerate() {
            self.stacks[i] += amount;
            if amount > 0 {
                self.events.push(GameEvent::PotAwarded {
                    seat: Seat::ALL[i],
                    amount,
                });
            }
            if self.dealt[i] && self.stacks[i] == 0 {
                self.events
                    .push(GameEvent::PlayerEliminated { seat: Seat::ALL[i] });
            }
        }
        let outcome = Outcome {
            winner,
            pot,
            awards,
            hands,
            folded,
            pots: results,
        };
        self.events.push(GameEvent::HandCompleted {
            outcome: outcome.clone(),
            stacks: self.stacks.to_vec(),
        });
        self.outcome = Some(outcome);
        self.phase = Phase::HandComplete;
        if self.stacks.iter().filter(|n| **n > 0).count() == 1 {
            self.phase = Phase::MatchComplete;
            self.events.push(GameEvent::MatchCompleted {
                winner: Seat::ALL[self.stacks.iter().position(|n| *n > 0).unwrap()],
            });
        }
        debug_assert_eq!(
            self.stacks.iter().sum::<Chips>(),
            self.config.starting_stacks.iter().sum::<Chips>()
        );
    }
}
