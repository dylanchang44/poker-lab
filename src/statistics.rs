//! Player-facing formatting of existing counters, never a second collector.
use crate::npc::opponent::{Metric, OpponentModel};

pub const METRICS: [(Metric, &str); 9] = [
    (
        Metric::Vpip,
        "Voluntary preflop payment / hands with a preflop decision",
    ),
    (
        Metric::Pfr,
        "Preflop raises / hands with a preflop decision",
    ),
    (
        Metric::ThreeBet,
        "Reraises / first legal opportunity facing one preflop raise",
    ),
    (
        Metric::FoldThreeBet,
        "Folds / original opener facing a 3-bet",
    ),
    (
        Metric::Cbet,
        "Flop bets / unbet flop opportunities as last preflop raiser",
    ),
    (
        Metric::FoldCbet,
        "Folds / first response to an unraised continuation bet",
    ),
    (
        Metric::RiverAggression,
        "River bets or raises / decisions where raising was legal",
    ),
    (
        Metric::Showdown,
        "Public showdowns / hands reaching the flop without folding",
    ),
    (
        Metric::WonShowdown,
        "Any pot share won / public showdowns (ties count)",
    ),
];

pub fn report(model: &OpponentModel) -> String {
    let mut rows = vec![format!(
        "YOUR STATISTICS  /  {} completed hands dealt to you",
        model.hands
    )];
    for (metric, definition) in METRICS {
        let count = model.total.get(metric);
        let rate = if count.opportunities == 0 {
            "No opportunities yet (0 / 0)".to_owned()
        } else {
            format!(
                "{:.1}% ({} / {}){}",
                100.0 * count.yes as f64 / count.opportunities as f64,
                count.yes,
                count.opportunities,
                if count.opportunities < 30 {
                    " - small sample"
                } else {
                    ""
                }
            )
        };
        rows.push(format!("{}: {rate}\n  {definition}", metric.label()));
    }
    rows.push("Observed lifetime rates; no smoothing. Small samples are uncertain.\nPoker reads contains the separate smoothed estimates used by opponents.".into());
    rows.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::opponent::Count;
    #[test]
    fn displayed_rates_use_exact_existing_counts_not_priors() {
        let mut model = OpponentModel::default();
        assert!(report(&model).contains("No opportunities yet (0 / 0)"));
        model.total.counts[Metric::Vpip as usize] = Count {
            yes: 34,
            opportunities: 100,
        };
        model.total.counts[Metric::FoldThreeBet as usize] = Count {
            yes: 6,
            opportunities: 10,
        };
        let text = report(&model);
        assert!(text.contains("VPIP: 34.0% (34 / 100)"));
        assert!(text.contains("Fold to 3-bet: 60.0% (6 / 10) - small sample"));
        assert_eq!(model.total.get(Metric::FoldThreeBet).yes, 6);
    }
}
