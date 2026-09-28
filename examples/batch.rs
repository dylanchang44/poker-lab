//! cargo run --release --example batch -- 20 42 96
//! Arguments: matches, seed, equity samples/decision, hand cap/match.
use poker_lab::npc::{profiles::PROFILES, simulation::simulate};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let read = |i: usize, default: u64| {
        args.get(i)
            .map(|s| s.parse::<u64>().expect("arguments must be integers"))
            .unwrap_or(default)
    };
    let matches = u32::try_from(read(0, 20)).unwrap();
    let seed = read(1, 42);
    let samples = u32::try_from(read(2, 96)).unwrap();
    let cap = u32::try_from(read(3, 10000)).unwrap();
    let started = std::time::Instant::now();
    let batch = simulate(matches, seed, samples, cap);
    println!(
        "Seed {seed}; samples {}; {} completed, {} capped matches; {} hands; {:.2?}",
        samples.clamp(16, 4096),
        batch.completed,
        batch.truncated,
        batch.hands,
        started.elapsed()
    );
    println!(
        "Identity   Hands   VPIP%    PFR%   Aggr%   Fold%    SD%   Chips/hand  Pot-win%  Net chips  Matches"
    );
    for (profile, s) in PROFILES.iter().zip(batch.rows) {
        let percent = |n: u64, d: u64| 100.0 * n as f64 / d.max(1) as f64;
        println!(
            "{:<9} {:>6} {:>7.1} {:>7.1} {:>7.1} {:>7.1} {:>6.1} {:>12.1} {:>8.1} {:>10} {:>8}",
            profile.name,
            s.hands,
            percent(s.vpip, s.hands),
            percent(s.pfr, s.hands),
            percent(s.aggressive, s.actions),
            percent(s.folds, s.actions),
            percent(s.showdowns, s.hands),
            s.committed as f64 / s.hands.max(1) as f64,
            percent(s.hands_won, s.hands),
            s.net_chips,
            s.matches_won
        );
    }
    println!(
        "Seats rotate across matches. VPIP/PFR/SD/pot-win are per dealt hand; aggression/fold per decision. Pot-win includes shared/side pots. Small samples do not establish skill rankings."
    );
}
