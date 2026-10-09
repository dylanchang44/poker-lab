//! Public-action statistics. This reducer explicitly discards secret replay events.
use crate::poker::{Action, Phase, Seat, events::GameEvent};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const RECENT_HANDS: usize = 50;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Metric {
    Vpip,
    Pfr,
    Limp,
    ThreeBet,
    FoldThreeBet,
    Cbet,
    FoldCbet,
    TurnAggression,
    RiverAggression,
    FoldRiver,
    FoldLargeRiver,
    Showdown,
    WonShowdown,
    BetSize,
    RaiseSize,
}
impl Metric {
    pub const ALL: [Self; 15] = [
        Self::Vpip,
        Self::Pfr,
        Self::Limp,
        Self::ThreeBet,
        Self::FoldThreeBet,
        Self::Cbet,
        Self::FoldCbet,
        Self::TurnAggression,
        Self::RiverAggression,
        Self::FoldRiver,
        Self::FoldLargeRiver,
        Self::Showdown,
        Self::WonShowdown,
        Self::BetSize,
        Self::RaiseSize,
    ];
    pub fn label(self) -> &'static str {
        [
            "VPIP",
            "PFR",
            "Limp",
            "3-bet",
            "Fold to 3-bet",
            "C-bet",
            "Fold to c-bet",
            "Turn aggression",
            "River aggression",
            "Fold to river bet",
            "Fold to large river bet",
            "Went to showdown",
            "Won showdown share",
            "Bet size / pot",
            "Raise increment / pot after call",
        ][self as usize]
    }
    pub fn prior(self) -> f64 {
        match self {
            Self::Vpip => 0.30,
            Self::Pfr => 0.20,
            Self::Limp => 0.10,
            Self::ThreeBet => 0.08,
            Self::Cbet => 0.55,
            Self::TurnAggression | Self::RiverAggression => 0.30,
            Self::Showdown => 0.35,
            _ => 0.50,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub yes: u64,
    pub opportunities: u64,
}
impl Count {
    pub fn observe(&mut self, success: bool) {
        self.opportunities += 1;
        self.yes += u64::from(success);
    }
    pub fn estimate(self, metric: Metric) -> f64 {
        (self.yes as f64 + 12.0 * metric.prior()) / (self.opportunities as f64 + 12.0)
    }
    pub fn confidence(self) -> f64 {
        self.opportunities as f64 / (self.opportunities as f64 + 40.0)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandSample {
    pub counts: [Count; 15],
}
impl HandSample {
    pub fn get(&self, m: Metric) -> Count {
        self.counts[m as usize]
    }
    fn tally(&mut self, m: Metric, yes: bool) {
        self.counts[m as usize].observe(yes);
    }
    pub fn add(&mut self, other: &Self) {
        for (a, b) in self.counts.iter_mut().zip(&other.counts) {
            a.yes = a.yes.saturating_add(b.yes);
            a.opportunities = a.opportunities.saturating_add(b.opportunities);
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpponentModel {
    pub hands: u64,
    pub total: HandSample,
    pub recent: VecDeque<HandSample>,
}
impl OpponentModel {
    pub fn record(&mut self, sample: HandSample) {
        self.hands += 1;
        self.total.add(&sample);
        self.recent.push_back(sample);
        while self.recent.len() > RECENT_HANDS {
            self.recent.pop_front();
        }
    }
    pub fn recent_total(&self) -> HandSample {
        let mut total = HandSample::default();
        for h in &self.recent {
            total.add(h);
        }
        total
    }
    pub fn estimate(&self, m: Metric) -> f64 {
        let long = self.total.get(m);
        let recent = self.recent_total().get(m);
        let weight = 0.45 * (recent.opportunities as f64 / 25.0).min(1.0);
        long.estimate(m) * (1.0 - weight) + recent.estimate(m) * weight
    }
    pub fn archetype(&self) -> &'static str {
        if self.total.get(Metric::Vpip).opportunities < 40 {
            return "Unknown";
        }
        let v = self.estimate(Metric::Vpip);
        let p = self.estimate(Metric::Pfr);
        if v < 0.18 {
            "Nit"
        } else if v > 0.50 && p > 0.36 {
            "Loose aggressive"
        } else if v > 0.42 && p < 0.20 {
            "Loose passive"
        } else if v < 0.30 && p > 0.16 {
            "Tight aggressive"
        } else if v < 0.30 {
            "Tight passive"
        } else {
            "Balanced / mixed"
        }
    }
    pub fn hints(&self) -> Vec<String> {
        let mut result = Vec::new();
        for (m, threshold, line) in [
            (Metric::Pfr, 0.38, "The player has often raised preflop."),
            (
                Metric::FoldLargeRiver,
                0.67,
                "The player has often folded when facing large river bets.",
            ),
            (
                Metric::Showdown,
                0.55,
                "The player has often stayed in to showdown after seeing the flop.",
            ),
        ] {
            if self.total.get(m).opportunities >= 30 && self.estimate(m) > threshold {
                result.push(line.into());
            }
        }
        let recent = self.recent_total().get(Metric::Pfr);
        let long = self.total.get(Metric::Pfr);
        if long.opportunities >= 100
            && recent.opportunities >= 30
            && recent.estimate(Metric::Pfr) > long.estimate(Metric::Pfr) + 0.12
        {
            result.push("The player's recent preflop play has been more aggressive than their longer-term play.".into());
        }
        result.truncate(2);
        result
    }
}

#[derive(Clone, Copy)]
struct Opportunity {
    seat: Seat,
    phase: Phase,
    pot: u32,
    call: u32,
    street: u32,
    raise: bool,
}
#[derive(Default)]
pub struct Observer {
    sample: HandSample,
    hand: u64,
    human_dealt: bool,
    human_folded: bool,
    pre_seen: bool,
    vpip: bool,
    pfr: bool,
    limp: bool,
    three_seen: bool,
    fold_three_seen: bool,
    raises: u32,
    opener: Option<Seat>,
    last_raiser: Option<Seat>,
    high: u32,
    street_aggression: u32,
    cbet_by: Option<Seat>,
    cbet_seen: bool,
    fold_cbet_seen: bool,
    river_seen: bool,
    river_large: bool,
    flop_seen: bool,
    shown: bool,
    opportunity: Option<Opportunity>,
}
impl Observer {
    /// Only returns a completed public hand. No private cards are retained, even
    /// at showdown: eligibility booleans and public awards suffice for these metrics.
    pub fn observe(&mut self, event: &GameEvent) -> Option<(u64, HandSample)> {
        match event {
            GameEvent::HandStarted {
                number,
                stacks,
                blinds,
                ..
            } => {
                *self = Self {
                    hand: *number,
                    human_dealt: stacks[0] > 0,
                    high: blinds[1],
                    ..Self::default()
                };
            }
            GameEvent::DecisionOffered {
                seat,
                phase,
                pot,
                to_call,
                street_bet,
                can_raise,
            } => {
                self.opportunity = Some(Opportunity {
                    seat: *seat,
                    phase: *phase,
                    pot: *pot,
                    call: *to_call,
                    street: *street_bet,
                    raise: *can_raise,
                });
            }
            GameEvent::CommunityCardsDealt { phase, .. } => {
                self.high = 0;
                self.street_aggression = 0;
                if *phase == Phase::Flop {
                    self.flop_seen = self.human_dealt && !self.human_folded;
                }
            }
            GameEvent::PlayerActed {
                seat,
                phase,
                action,
                paid,
                street_total,
                ..
            } => {
                let o = self
                    .opportunity
                    .take()
                    .filter(|o| o.seat == *seat && o.phase == *phase)?;
                self.high = o.street.saturating_add(o.call);
                let aggression = *street_total > self.high;
                let human = *seat == Seat::Human;
                if human {
                    if *phase == Phase::PreFlop {
                        self.pre_seen = true;
                        self.vpip |= *paid > 0;
                        self.pfr |= aggression;
                        self.limp |= *paid > 0 && !aggression && self.raises == 0;
                        if self.raises == 1 && o.raise && !self.three_seen {
                            self.sample.tally(Metric::ThreeBet, aggression);
                            self.three_seen = true;
                        }
                        if self.raises == 2
                            && self.opener == Some(Seat::Human)
                            && o.call > 0
                            && !self.fold_three_seen
                        {
                            self.sample
                                .tally(Metric::FoldThreeBet, *action == Action::Fold);
                            self.fold_three_seen = true;
                        }
                    }
                    if *phase == Phase::Flop {
                        if self.last_raiser == Some(Seat::Human)
                            && self.street_aggression == 0
                            && o.raise
                            && !self.cbet_seen
                        {
                            self.sample.tally(Metric::Cbet, aggression);
                            self.cbet_seen = true;
                        }
                        if self.cbet_by.is_some_and(|s| s != Seat::Human)
                            && self.street_aggression == 1
                            && o.call > 0
                            && !self.fold_cbet_seen
                        {
                            self.sample.tally(Metric::FoldCbet, *action == Action::Fold);
                            self.fold_cbet_seen = true;
                        }
                    }
                    if o.raise && matches!(phase, Phase::Turn | Phase::River) {
                        self.sample.tally(
                            if *phase == Phase::Turn {
                                Metric::TurnAggression
                            } else {
                                Metric::RiverAggression
                            },
                            aggression,
                        );
                    }
                    if *phase == Phase::River
                        && self.street_aggression == 1
                        && o.call > 0
                        && !self.river_seen
                    {
                        self.sample
                            .tally(Metric::FoldRiver, *action == Action::Fold);
                        if self.river_large {
                            self.sample
                                .tally(Metric::FoldLargeRiver, *action == Action::Fold);
                        }
                        self.river_seen = true;
                    }
                    if aggression && o.pot > 0 {
                        let raising = self.high > 0;
                        let increment = if raising {
                            street_total - self.high
                        } else {
                            *paid
                        };
                        let denominator = u64::from(o.pot)
                            + if raising {
                                u64::from(o.call.min(*paid))
                            } else {
                                0
                            };
                        let count = &mut self.sample.counts[if raising {
                            Metric::RaiseSize
                        } else {
                            Metric::BetSize
                        } as usize];
                        count.yes += u64::from(increment) * 10_000 / denominator.max(1);
                        count.opportunities += 1;
                    }
                    if *action == Action::Fold {
                        self.human_folded = true;
                    }
                }
                if aggression {
                    if *phase == Phase::PreFlop {
                        self.raises += 1;
                        if self.raises == 1 {
                            self.opener = Some(*seat);
                        }
                        self.last_raiser = Some(*seat);
                    } else {
                        if *phase == Phase::Flop
                            && self.street_aggression == 0
                            && self.last_raiser == Some(*seat)
                        {
                            self.cbet_by = Some(*seat);
                        }
                        if *phase == Phase::River && self.street_aggression == 0 {
                            self.river_large = u64::from(*paid) * 4 >= u64::from(o.pot) * 3;
                        }
                        self.street_aggression += 1;
                    }
                    self.high = *street_total;
                }
            }
            GameEvent::ShowdownStarted { cards } => {
                self.shown = cards.first().is_some_and(Option::is_some);
            }
            GameEvent::HandCompleted { outcome, .. } if self.human_dealt => {
                if self.pre_seen {
                    self.sample.tally(Metric::Vpip, self.vpip);
                    self.sample.tally(Metric::Pfr, self.pfr);
                    self.sample.tally(Metric::Limp, self.limp);
                }
                if self.flop_seen {
                    self.sample.tally(Metric::Showdown, self.shown);
                }
                if self.shown {
                    self.sample
                        .tally(Metric::WonShowdown, outcome.awards[0] > 0);
                }
                self.human_dealt = false;
                return Some((self.hand, self.sample.clone()));
            }
            _ => {}
        }
        None
    }
}
