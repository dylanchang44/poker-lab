//! Run a seeded, window-free match: cargo run --example simulate -- 42
use poker_lab::{
    npc::{BasicNpc, Strategy},
    poker::{Phase, PokerMatch},
};

fn main() {
    let seed = std::env::args()
        .nth(1)
        .map(|s| s.parse().expect("seed must be a u64"))
        .unwrap_or(42);
    let mut game = PokerMatch::new(seed);
    let mut bots = [
        BasicNpc::new(seed.wrapping_add(1)),
        BasicNpc::new(seed.wrapping_add(2)),
    ];
    let mut hands = 0;
    while game.phase() != Phase::MatchComplete {
        game.start_next_hand().unwrap();
        hands += 1;
        while let Some(seat) = game.actor() {
            let action = bots[seat.index()].decide(&game.observe(seat)).unwrap();
            game.act(seat, action).unwrap();
        }
    }
    println!(
        "Seed {seed}: {hands} hands; final stacks {:?}; {} events",
        game.stacks(),
        game.history().len()
    );
}
