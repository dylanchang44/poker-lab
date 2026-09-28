# Poker Lab

A native Linux Texas Hold'em game built with Rust and Bevy. Stage 3 adds illustrated characters, expressions, reactions and dialogue to your table against **Ananya (The Analyst)**, **Freya (The Gambler)**, and **Yuna (The Observer)**. Each starts with 1,000 virtual chips; blinds stay at 5/10. Play until one player holds all 4,000 chips.

## Requirements

Developed on CachyOS/Arch Linux with Rust/Cargo 1.94.1 stable, Bevy **0.18.1**, and Zed. Any editor works. Dependencies remain explicitly pinned; keep Cargo.lock.

You need Rust/Cargo, a C compiler/linker, pkg-config, a working desktop, and a Vulkan-capable GPU/driver. On Arch, inspect availability of base-devel, pkgconf, wayland, libxkbcommon, libx11, libxcb, vulkan-icd-loader, and your GPU driver; only install missing packages. `vulkaninfo --summary` can check graphics support. Stage 3 includes original generated portrait PNGs in assets/characters and enables Bevy's PNG decoder. No new crate versions, services or system packages are required.

The first Bevy build may take several minutes and several GB. Subsequent builds are incremental.

## Run, build and test

```sh
cargo run
cargo run -- --seed 42
cargo run -- --heads-up --seed 42  # original two-seat/basic-opponent practice

cargo check
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Headless personality evaluation: matches, seed, equity samples, hand cap/match
cargo run --example batch -- 100 42 96 10000
# Original Stage 1 headless example remains available:
cargo run --example simulate -- 42

# Rendered pointer-hit testing; needs desktop/GPU, then exits automatically:
cargo run --example ui_smoke
cargo run --example ui_smoke -- --small
cargo run --example ui_smoke -- --hd
cargo run --example ui_smoke -- --qhd
```

The batch runner opens no window and waits for no thinking timers. Release mode is optional for larger batches (`cargo run --release --example batch -- ...`), but its first compilation is separate. Seeded runs repeat when configuration, inputs, strategy parameters, sample count and pinned RNG versions are identical.

## Playing

1. Select **Start Game**. You sit at the bottom, Ananya left, Freya top, Yuna right. The highlighted seat acts next. Each opponent has an illustrated portrait, seven expressions, a current-action indicator, and a distinct accent color. Reactions follow public actions and outcomes, never private cards.
2. **Fold**, **Check / Call**, **Bet / Raise**, and **All-in** follow engine-provided legal actions. Unavailable controls are disabled. A short-stack call automatically spends only the available chips.
3. Click the numeric amount to type, or choose **Min**, **1/2**, **Pot**, or **Max**. Amounts are the **total contribution on this street**, including blinds/already committed chips. Raising to 30 after posting 5 costs 25 more.
4. Backspace edits; Enter confirms/clamps to the legal range; Escape resets. Then click Bet / Raise. Invalid partial input disables submission. Short all-in raises use All-in.
5. NPC decisions run in background threads with a default 0.55-second minimum thinking interval. There is no sleeping on the rendering thread.
6. Review each hand's main/side-pot awards, then choose **Next Hand**. Eliminated seats stay visibly marked OUT and are skipped. If you bust, you can spectate the remaining players and continue dealing.
7. **New Match** is always available in the header and resets all stacks. **Back to Menu** abandons the current match. At the final result, start another match or return to the menu.

D/SB/BB identify the button and blinds. A dead button can remain at an eliminated seat for one hand; sometimes there is no small blind. The engine switches to heads-up blind/action rules with two survivors.

The pot includes chips shown as “In front.” Side-pot layers shown during all-ins are provisional until responses/refunds settle. Final awards list each pot separately. Folded NPC cards remain hidden; eligible showdown hands are revealed. Suit letters are c/d/h/s, with suit names printed on cards.

## Architecture

```text
src/main.rs              Window, plugins, seed/practice arguments
src/lib.rs               Rendering-independent poker/NPC API
src/poker/
  cards.rs, deck.rs       Cards and seeded ChaCha8 shuffle
  hand.rs                Small rs_poker evaluator adapter
  state.rs               Phases, legal actions, observations, pot results
  engine.rs              One Table<N> rules implementation (2-4 seats)
  pots.rs                Contribution layers, eligibility, odd-chip split
  events.rs              Structured privileged replay events
  tests.rs               Original Stage 1 rule tests
  multiway_tests.rs      Four-seat rules and seeded full-match tests
src/characters/
  mod.rs                 Cast definitions, public reactions, dialogue values
  tests.rs               Identity, strategy mapping, expression/epoch tests
src/npc/
  mod.rs                 Strategy trait
  profiles.rs            Stable IDs, names, colors, personality parameters
  personality.rs         Shared probability-aware decision policy
  equity.rs              Unknown-card sampling, starting score, draw detection
  basic.rs               Original Stage 1 strategy
  simulation.rs          Window-free batch execution and statistics
  tests.rs               Equity, personality, reproducibility tests
src/game/
  mod.rs                 Bevy session resource, timer, worker/token checks
  match_engine.rs        Runtime selection of two/four-seat engine
src/ui/
  mod.rs                 Menu, navigation, integration tests
  table.rs               Four seats, cards, pots, results and legal controls
  controls.rs            Button commands and numeric entry
  characters.rs          Cached portraits, persistent entities, animation/dialogue
examples/batch.rs         Personality batch CLI
examples/simulate.rs      Original heads-up simulation
examples/ui_smoke.rs      Rendered interaction check and screenshots
assets/characters/       Three expression atlases, asset contract and prompts
```

### Rules and turn order

`Table<N>` is ordinary Rust without Bevy imports. `PokerMatch = Table<2>` preserves the Stage 1 rules/testing API; `FourPlayerMatch = Table<4>` uses the same implementation. Seats remain fixed as players leave, and a ring scan skips folded, all-in and eliminated players as appropriate. Legacy `Seat::Npc` means seat 1; presentation names come from profiles.

The poker phases are WaitingForHand, PreFlop, Flop, Turn, River, Showdown, HandComplete and MatchComplete. A street ends only when every required response is complete. Preflop starts left of the BB; later streets start left of the button. Heads-up, the button posts SB and acts first preflop, last postflop. Burn cards precede each community street.

Contributions, current-street bets and remaining stacks are separate integer quantities. Unique unmatched top contributions are refunded. Pot layers include folded money but only eligible live contributors can win. Every layer is evaluated and split separately. Odd chips go to winners clockwise left of the button. Cumulative short all-ins reopen a prior actor only once that actor faces at least a full raise. Players cannot bet into a dry side pot against only all-in opponents.

Button/blind progression uses the dead-button rule, and heads-up transition avoids the previous BB posting BB twice. These choices follow the relevant [Poker TDA rules](https://www.pokertda.com/view-poker-tda-rules/). All-in runouts and settlement are synchronous engine transitions recorded in events; the UI shows the final board/result without dealing animation.

Every command goes through `act(seat, action)`: validation precedes mutation. Both human and NPC inputs obey the same authority. Throughout play, `sum(stacks) + active pot == starting chips`.

### Bevy and background decisions

Bevy's outer states MainMenu/InGame govern screens, not poker rules. Each frame runs input, update, layout and rendering even when no player acts. This differs from a backend request loop: blocking the frame also blocks visible feedback and input.

The match is a **resource**, a single GameSession accessed by systems. Cards, buttons, labels and the camera are **entities**; layout, text, colors and Control are **components**. **Systems** translate interactions into commands, poll NPC work and redraw changed presentation. UI controls use LegalActions rather than duplicate betting rules. Portrait, dialogue and felt entities persist across hands and restarts. Board/control content rebuilds only on gameplay/input changes; small animation systems update persistent components each frame.

For an NPC turn, the host clones an owned Observation and strategy into a standard-library worker. The worker returns a proposed action and updated RNG state; Bevy polls the channel without blocking and applies it after the timer. Session ID + event revision + acting seat must still match. Restart/next-hand drops the old receiver; stale results cannot change the new game. Worker failure or an invalid strategy proposal is logged and uses a validated check/fold fallback.

Tune `NpcSettings` in game/mod.rs: default 192 equity samples and 0.55 seconds. Samples are bounded to 16–4,096. Headless simulation invokes the same strategies directly, without workers/timers; legacy heads-up practice retains BasicNpc.

### Strategy and incomplete information

`Strategy::decide(&Observation) -> Option<Action>` is the replacement point for future decision engines. It receives its own hole cards, public board/stacks/contributions, position, remaining players, public action history and legal choices. It receives **no unrestricted engine reference, seed, deck, private deal events or unrevealed opponent cards**.

Equity samples opponent hands and the remaining board uniformly from unknown cards, excluding only legitimately known cards. Best-five ranks use the existing pinned [rs_poker 4.1.0 adapter](https://docs.rs/rs_poker/4.1.0/rs_poker/core/trait.Rankable.html). No new evaluator or dependency was added.

The shared policy combines starting-hand quality, position, number of opponents, sampled equity, pot odds capped to contestable contributions, effective stacks, draws and public aggression history. Personality changes thresholds and eligible aggression/sizing; it never changes the rules. Weak hands cannot become automatic all-ins because aggression is high.

| Parameter | Ananya | Freya | Yuna | Effect |
| --- | ---: | ---: | ---: | --- |
| Selectivity | .74 | .20 | .82 | Starting-hand admission threshold |
| Aggression | .68 | .93 | .18 | Value threshold and frequency of value bets |
| Risk tolerance | .30 | .80 | .18 | Safety margin above calling pot odds |
| Bluff frequency | .035 | .16 | .01 | Conditional draw/blocker opportunities, not unconditional bets |
| Bet size | 60% | 85% | 45% | Pot-relative preference, clamped to legal/effective limits |
| Continuation | .65 | .80 | .20 | Eligible flop continuation bets |
| Position awareness | .90 | .50 | .65 | Late-position loosening of selection |

Parameters are policy modifiers, not promises of observed percentages. Ananya enters selectively and applies calculated pressure; Freya plays wider and pushes more; Yuna raises less and favors cautious calls/checks.

### Events and future work

The engine records starts, private deals/burns, blinds, actions/amounts, streets, refunds, showdown, awards, eliminations and completion. GameSession consumes only public events into the activity list. Observations carry public action history; the private `history()` stream is privileged and must never be handed to a live NPC or displayed wholesale. The simulator, as trusted host, uses events to count statistics.

Stage 3 retains the stable IDs `mira`, `jax`, `nova` and seat enum values; their display names are now Ananya, Freya and Yuna. Strategy numbers and RNG sequencing are unchanged. Profile names are the identity authority; character definitions add age, origin, asset path and reaction intensity without duplicating names.

The trusted GameSession maps engine events into a narrow `PresentationEvent`: hand start, public action or final awards/showdown participation. Private deal/burn cards, seed and evaluated hand strength never enter the reaction model. `PresentationState` stores expressions and timers separately from poker state. Thinking follows the current actor; public aggression produces Confident; a significant award produces Happy; multiple award recipients produce Surprised; revealed non-winners show Disappointed; elimination persists. Ananya/Yuna react subtly, Freya more strongly.

Bevy's AssetServer loads three sheets once. `ImageNode.rect` selects an expression cell, two image layers crossfade over 0.22 seconds, and `UiTransform` supplies small idle movement and action emphasis. Folded portraits dim; eliminated portraits settle and stay subdued. Frame delta time drives these effects; no animation delays a legal action. A session epoch resets animation and rejects stale dialogue after restart.

`DialogueLine` carries session/hand epochs, speaker seat, text, optional expression and duration. `DialogueRequest` is the Bevy message input. Stage 3 displays a few predefined public-event lines. A Stage 4 provider can produce this same value asynchronously; the UI resolves the speaker name/portrait and expires the bubble. Provider calls and credentials belong outside poker rules and strategies. No conversational model, memory, training, persistence or networking is implemented.

See [the character asset guide](assets/characters/README.md) for the seven-expression atlas contract, exact dimensions, provenance, replacement instructions and optional-file fallback. See [Stage 3 verification and architecture notes](docs/stage3-verification.md) for tests and practical implementation details.

## Verification and simulation

See [Stage 3 verification](docs/stage3-verification.md) for current checks and [Stage 2 verification](docs/stage2-verification.md) for the historical 100-match batch. VPIP/PFR/showdown/pot-win are per dealt hand; aggression/fold are per decision. Chips/hand counts commitments after refunds, including blinds. “Pot win” includes any shared or side-pot award; net chips and match wins are also reported. Seats rotate between matches to reduce fixed-seat bias. A cap reports incomplete matches rather than treating them as wins.

These are behavior diagnostics, not strength rankings. Uniform opponent ranges, small equity samples and a fixed heuristic policy are intentional limitations; there is no opponent learning or GTO solver. Other limits: fixed blinds, two/four-seat selectable configurations, generated portrait atlases, simple motion, instantaneous runouts, in-memory event history and no save/resume.

### Manual GUI checklist

- Use your physical mouse/keyboard and display scaling: confirm all four names/cards, active highlight, chip counts and controls fit.
- Type valid/invalid raise totals, use presets, and verify illegal actions remain unavailable.
- Play folded hands, multiway showdowns and unequal all-ins; check hidden cards, main/side awards and total chips.
- Advance through eliminations to heads-up; if you bust, try spectating and Next Hand.
- Restart while an NPC thinks; verify no old action arrives. Finish a match, start another, and return to the menu.

- Observe thinking/raise/win/loss/fold/elimination reactions and their return to neutral. A folded player's private cards must stay hidden.
- Watch a dialogue bubble expire; restart during a reaction and confirm the old line/expression disappears.
- Check 1920x1080 and 2560x1440 on your display. The desktop used for verification clamps large windows to a 1920x1052 client area; true QHD remains a manual display check.
