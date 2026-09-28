# Stage 2 verification

Executed on 2026-09-27 on CachyOS, Rust/Cargo 1.94.1, Bevy 0.18.1. Existing pinned dependencies were reused; no packages or crates were added. Cargo commands used `--offline` where applicable because dependencies were already cached.

## Executed checks

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo check --offline` / `cargo build --offline` | Passed |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed, no warnings |
| `cargo test --offline` | 40 passed: 32 library + 8 application tests; zero failures |
| `cargo run --offline` | Compiled and opened Poker Lab on Vulkan, AMD Radeon RX 7700 XT (RADV/Mesa 26.2.3) |
| `cargo run --offline --example ui_smoke -- --small` | Passed at 1000x820 logical resolution / 125% display scale |
| `cargo run --offline --example simulate -- 42` | Original heads-up runner finished: 393 hands, stacks [0, 2000], 5,895 events |
| `cargo run --offline --example batch -- 100 42 96` | 100 completed matches, zero capped matches, 15,906 hands |

The graphical smoke test used Bevy pointer hit-testing, not OS mouse injection. It checked all four names, Start Game, legal pot-size preset/raise, check/call play to settlement, Next Hand/button rotation, mid-hand New Match, and Back to Menu. It saved menu/table/board/result/next-hand screenshots in `/tmp/poker-lab-*.png`. Table and result screenshots were visually inspected. An earlier smoke run found the missing mid-hand restart control; the successful run above followed its addition. Physical mouse/keyboard and arbitrary display scaling still need the README's manual checks.

## Rule and integration coverage

Original Stage 1 rule tests remain intact. Existing navigation, input, timer and match-flow assertions run in heads-up practice mode. New tests exercise:

- Unique four-seat dealing, preflop/postflop order and folded/eliminated-seat skipping.
- Dealer/blind rotation, dead SB/button positions and heads-up transition.
- Short BB bring-in, full/short raises and cumulative reopening.
- Unequal all-ins, multiple pot layers, folded money, eligibility, distinct main/side winners and odd chips.
- Uncalled refunds, atomic invalid actions, chip conservation and hidden-card filtering.
- 100 seeded random four-player matches that terminate and reproduce their full event streams.
- Equity ties, reproducible unknown-card sampling, starting-hand/draw sanity checks.
- Three personalities facing the same 180 initial observations: distinct entry/raising counts, all decisions legal, all rejecting a weak 7-2 offsuit fixture.
- Two seeded personality batches repeating exactly, finishing, and summing to zero net chips.
- Background NPC play through a full four-player UI match, elimination, restart and menu cleanup.
- Stale decision rejection across both turn changes and fresh sessions; safe validated fallback for invalid proposals.

## Personality batch

Command: `cargo run --offline --example batch -- 100 42 96`.

Seed 42, 96 equity samples per decision, 10,000-hand cap per match. Four identities rotate seats each match; Baseline is a neutral bot standing in for the human. The run completed 15,906 hands across 100 matches in 37.89 seconds (unoptimized application build, concurrently with a graphical check). Timing is illustrative, not a benchmark.

| Identity | Dealt hands | VPIP | PFR | Aggressive actions | Folds | Showdown | Chips/hand | Pot wins | Net chips | Match wins |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Baseline | 10,803 | 26.5% | 3.8% | 15.5% | 22.3% | 27.9% | 40.5 | 43.3% | +4,000 | 26 |
| Mira | 10,280 | 17.6% | 5.0% | 17.4% | 30.2% | 22.4% | 40.1 | 36.0% | 0 | 25 |
| Jax | 6,287 | 37.1% | 10.3% | 23.9% | 18.4% | 29.5% | 62.0 | 45.7% | -4,000 | 24 |
| Nova | 14,565 | 15.7% | 1.7% | 9.2% | 27.0% | 27.8% | 32.5 | 33.3% | 0 | 25 |

Definitions:

- VPIP: dealt hands with a voluntary preflop chip payment, excluding blind posting.
- PFR: dealt hands with a preflop raise, including raising all-ins.
- Aggressive actions / folds: fraction of that identity's decisions, not hands.
- Showdown: dealt hands in which the player was eligible and exposed at showdown.
- Chips/hand: committed chips after uncalled refunds, including blinds, per dealt hand.
- Pot wins: hands receiving any award, including ties or a side pot. Several players can win in one hand.
- Net chips: final minus initial stacks, accumulated over matches. The batch sums to zero.

Jax enters substantially more hands and raises more often. Mira is selective but takes aggressive actions more often than Nova. Nova contributes less per hand and raises least. Elimination changes sample sizes and the number of opponents; these are mixed four-/three-/two-handed observations, not isolated four-handed rates. Near-even match wins do not establish equal strength. Uniform unknown opponent ranges and noisy equity estimates are deliberately simple; no statistical significance or professional bot strength is claimed.

## Remaining limitations

Fixed blinds, heuristic non-learning strategies, uniform opponent ranges, bounded Monte Carlo samples, simple card art and instantaneous all-in runouts. Event history is in-memory and privileged; there is no replay UI, persistence, conversation, memory, training or multiplayer. The developer batch reports cap exhaustion explicitly. A complete match has no general finite-hand guarantee, though all batches and full-match tests above finished.
