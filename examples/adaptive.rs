//! cargo run --example adaptive -- 2000 42 32
use poker_lab::npc::{
    opponent::Metric,
    profiles::NpcId,
    simulation::{HumanStyle, simulate_adaptive},
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let hands = args
        .first()
        .map(|s| s.parse().expect("hands"))
        .unwrap_or(2000);
    let seed = args.get(1).map(|s| s.parse().expect("seed")).unwrap_or(42);
    let samples = args
        .get(2)
        .map(|s| s.parse().expect("samples"))
        .unwrap_or(32);
    for (style, switch) in HumanStyle::ALL
        .into_iter()
        .map(|s| (s, None))
        .chain([(HumanStyle::LooseAggressive, Some(HumanStyle::TightPassive))])
    {
        let r = simulate_adaptive(hands, seed, samples, style, switch);
        println!(
            "\n{style:?} -> {switch:?}: {} hands / {}",
            r.model.hands,
            r.model.archetype()
        );
        for m in [
            Metric::Vpip,
            Metric::Pfr,
            Metric::FoldThreeBet,
            Metric::FoldRiver,
            Metric::Showdown,
        ] {
            let c = r.model.total.get(m);
            let recent = r.model.recent_total().get(m);
            println!(
                "{} {}/{} estimate {:.3}, recent {}/{}",
                m.label(),
                c.yes,
                c.opportunities,
                r.model.estimate(m),
                recent.yes,
                recent.opportunities
            );
        }
        for (i, npc) in NpcId::ALL.into_iter().enumerate() {
            let s = r.rows[i + 1];
            println!(
                "{}: first {:?}, changed {} decisions, VPIP {:.1}% PFR {:.1}% aggression {:.1}% non-value aggression {} / {} actions, mean wager {:.1}% pot, wins {}/{}, net {} chips\n{}",
                npc.name(),
                r.first_adaptation[i],
                r.changed_decisions[i],
                100.0 * s.vpip as f64 / s.hands.max(1) as f64,
                100.0 * s.pfr as f64 / s.hands.max(1) as f64,
                100.0 * s.aggressive as f64 / s.actions.max(1) as f64,
                r.bluff_actions[i],
                s.actions,
                r.wager_percent_sum[i] as f64 / r.wagers[i].max(1) as f64,
                s.hands_won,
                s.hands,
                s.net_chips,
                r.final_reads[i].describe(npc, &r.model)
            );
        }
    }
}
