//! Fixed hand-start reads. Only public poker counters influence these modifiers.
use super::{
    opponent::{Metric, OpponentModel},
    profiles::{NpcId, Personality, profile},
};
use crate::poker::{Observation, Phase};

#[derive(Clone, Debug, PartialEq)]
pub struct Exploit {
    pub reason: &'static str,
    pub metric: Metric,
    pub opportunities: u64,
    pub minimum: u64,
    pub confidence: f64,
    pub threshold: f64,
    pub strength: f64,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Adaptation {
    pub active: Vec<Exploit>,
    pub preflop_pressure: f64,
    pub river_bluff: f64,
    pub value: f64,
    pub caution: f64,
    pub trap: f64,
    pub flop_counter: f64,
}
impl Adaptation {
    pub fn for_player(npc: NpcId, model: &OpponentModel) -> Self {
        let (minimum, threshold, scale) = match npc {
            NpcId::Freya => (12, 0.23, 1.0),
            NpcId::Ananya => (30, 0.42, 0.80),
            NpcId::Yuna => (55, 0.56, 0.55),
        };
        let mut result = Self::default();
        for (metric, boundary, high, reason) in [
            (
                Metric::FoldRiver,
                0.32,
                false,
                "Human calls river bets often: reduce bluffs and size value up",
            ),
            (
                Metric::FoldThreeBet,
                0.62,
                true,
                "Human folds to 3-bets often: selective preflop pressure",
            ),
            (
                Metric::FoldLargeRiver,
                0.65,
                true,
                "Human overfolds to large river bets: selective river bluffs",
            ),
            (
                Metric::Pfr,
                0.36,
                true,
                "Human raises preflop often: stronger counters and traps",
            ),
            (
                Metric::Vpip,
                0.45,
                true,
                "Human enters many pots: selective isolation and value",
            ),
            (
                Metric::Cbet,
                0.72,
                true,
                "Human continuation-bets often: selective strong check-raises",
            ),
            (
                Metric::RiverAggression,
                0.14,
                false,
                "Human seldom uses river aggression: respect this action more",
            ),
        ] {
            let count = model.total.get(metric);
            let estimate = model.estimate(metric);
            let evidence = if high {
                estimate - boundary
            } else {
                boundary - estimate
            };
            if count.opportunities < minimum || count.confidence() < threshold || evidence <= 0.0 {
                continue;
            }
            let strength = (evidence / 0.22).clamp(0.0, 1.0) * scale * count.confidence();
            result.active.push(Exploit {
                reason,
                metric,
                opportunities: count.opportunities,
                minimum,
                confidence: count.confidence(),
                threshold,
                strength,
            });
            match metric {
                Metric::FoldRiver => {
                    result.river_bluff -= 0.08 * strength;
                    result.value += 0.10 * strength;
                }
                Metric::FoldThreeBet => result.preflop_pressure += 0.12 * strength,
                Metric::FoldLargeRiver => result.river_bluff += 0.08 * strength,
                Metric::Pfr => {
                    result.preflop_pressure += 0.08 * strength;
                    result.caution += 0.035 * strength;
                    result.trap += if npc == NpcId::Yuna { 0.30 } else { 0.10 } * strength;
                }
                Metric::Vpip => {
                    result.preflop_pressure += 0.06 * strength;
                    result.value += 0.07 * strength;
                }
                Metric::Cbet => result.flop_counter += 0.12 * strength,
                Metric::RiverAggression => result.caution += 0.06 * strength,
                _ => {}
            }
            if result.active.len() == 3 {
                break;
            }
        }
        result.preflop_pressure = result.preflop_pressure.min(0.12);
        result.value = result.value.min(0.10);
        result.caution = result.caution.min(0.08);
        result
    }
    pub fn effective(&self, base: Personality, phase: Phase) -> Personality {
        let mut p = base;
        if phase == Phase::PreFlop {
            p.aggression = (p.aggression + self.preflop_pressure).min(0.98);
            p.selectivity =
                (p.selectivity + self.caution - self.preflop_pressure * 0.4).clamp(0.08, 0.94);
        } else {
            p.aggression = (p.aggression + self.value).min(0.98);
            if phase == Phase::River {
                p.bluff_frequency = (p.bluff_frequency + self.river_bluff)
                    .clamp(0.0, (base.bluff_frequency + 0.08).min(0.24));
            }
        }
        p
    }
    pub fn applies(view: &Observation) -> bool {
        view.seat.index() != 0 && view.in_hand.first() == Some(&true)
    }
    pub fn describe(&self, npc: NpcId, model: &OpponentModel) -> String {
        let b = profile(npc.seat()).personality;
        let p = self.effective(b, Phase::PreFlop);
        let r = self.effective(b, Phase::River);
        let reasons = self
            .active
            .iter()
            .map(|a| {
                format!(
                    "{}\n  n={} (min {}), confidence {:.0}% (min {:.0}%), weight {:.2}",
                    a.reason,
                    a.opportunities,
                    a.minimum,
                    a.confidence * 100.0,
                    a.threshold * 100.0,
                    a.strength
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{} / human read: {} / {} completed hands\nBase: aggression {:.2} / bluff {:.3} / selectivity {:.2}\nEffective preflop: aggression {:.2} / selectivity {:.2}\nEffective river: aggression {:.2} / bluff {:.3}\nPressure +{:.3} / value +{:.3} / caution +{:.3} / trap {:.3}\n{}",
            npc.name(),
            model.archetype(),
            model.hands,
            b.aggression,
            b.bluff_frequency,
            b.selectivity,
            p.aggression,
            p.selectivity,
            r.aggression,
            r.bluff_frequency,
            self.preflop_pressure,
            self.value,
            self.caution,
            self.trap,
            if reasons.is_empty() {
                "Collecting evidence; base personality active"
            } else {
                &reasons
            }
        )
    }
}

pub fn sizing_bucket(target: u32) -> u32 {
    [33u32, 50, 66, 75, 100]
        .into_iter()
        .min_by_key(|n| n.abs_diff(target))
        .unwrap()
}
