# Poker Lab

A native Linux Texas Hold'em game built with Rust and Bevy. Stage 4 adds optional local or remote LLM conversations with **Ananya (The Analyst)**, **Freya (The Gambler)**, and **Yuna (The Observer)**. Their poker strategies remain unchanged. Each starts with 1,000 virtual chips; blinds stay at 5/10. Play until one player holds all 4,000 chips.

## Requirements

Developed on CachyOS/Arch Linux with Rust/Cargo 1.94.1 stable, Bevy **0.18.1**, and Zed. Any editor works. Dependencies are explicitly pinned; keep Cargo.lock.

You need Rust/Cargo, a C compiler/linker, pkg-config, a working desktop, and a Vulkan-capable GPU/driver. On Arch, inspect availability of base-devel, pkgconf, wayland, libxkbcommon, libx11, libxcb, vulkan-icd-loader, and your GPU driver; only install missing packages. `vulkaninfo --summary` can check graphics support. Original generated portrait PNGs are in assets/characters. No LLM installation or API key is needed for the default mock mode or to play poker.

The first Bevy build may take several minutes and several GB. Subsequent builds are incremental.

## Run, build and test

```sh
cargo run
cargo run -- --seed 42
cargo run -- --heads-up --seed 42  # original two-seat/basic-opponent practice
POKER_LAB_CONFIG=config/conversation.example.json cargo run  # local model (edit model first)

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
8. Use **Table Talk** at lower left: click **Everyone** to cycle to Ananya, Freya, or Yuna; click the input, type up to 240 characters, then press Enter or **Send**. Recent shared dialogue remains visible. Escape leaves chat entry. Clicking a poker control leaves chat entry, so typing a wager does not send chat.

### Conversation configuration

Without `POKER_LAB_CONFIG`, a deterministic mock provider gives short test dialogue. Poker remains fully playable if a configured provider is down: a short preset line replaces a failed response, while the small status label says **Dialogue fallback**. Set `"enabled": false` to disable generated conversation and retain Stage 3's preset public-event lines.

For a local model, use [the example config](config/conversation.example.json): load a chat-tuned model in LM Studio, enable its local server in the Developer tab, and replace `YOUR_LOADED_MODEL_ID` with the identifier shown by LM Studio. Its OpenAI-compatible base URL is normally `http://127.0.0.1:1234/v1`; the app posts to `/chat/completions`. LM Studio documents [server startup](https://lmstudio.ai/docs/developer/openai-compat/tools), [chat completions](https://lmstudio.ai/docs/developer/openai-compat/chat-completions), and [model listing](https://lmstudio.ai/docs/developer/openai-compat/models). Copy the example to a local file if you want to preserve it while editing.

For a remote OpenAI-compatible service, set `"provider": "remote"`, an **HTTPS** `"base_url"` ending at the API version (for example `https://api.openai.com/v1`), and a supported `"model"`. Set `"api_key_env": "POKER_LAB_API_KEY"`, then export that variable in your shell without writing the key into the config or repository. The adapter uses the [Chat Completions API](https://developers.openai.com/api/reference/resources/chat), not poker-action tools. A remote provider with no key falls back safely. The endpoint must support compatible `messages`, `model`, `temperature`, `max_tokens`, and non-streaming responses. Models vary in JSON reliability; all responses are locally validated.

Other settings: `timeout_seconds` (1–120), `max_output_tokens` (16–1024), `temperature` (0–2), `initiative_frequency` (0–3, with 0 disabling unsolicited speech), and `history_limit` (4–32 messages). The history is in memory for the current match only; no API key or chat transcript is saved. A turn sends at most the recent bounded dialogue and a concise public table snapshot. A slow call runs in a worker and never pauses betting/rendering.

D/SB/BB identify the button and blinds. A dead button can remain at an eliminated seat for one hand; sometimes there is no small blind. The engine switches to heads-up blind/action rules with two survivors.

The pot includes chips shown as “In front.” Side-pot layers shown during all-ins are provisional until responses/refunds settle. Final awards list each pot separately. Folded NPC cards remain hidden; eligible showdown hands are revealed. Suit letters are c/d/h/s, with suit names printed on cards.

## Architecture

```text
src/main.rs              Window, plugins, seed/practice arguments
src/lib.rs               Rendering-independent poker/NPC API
src/conversation/
  config.rs              Provider settings and safe validation
  provider.rs            OpenAI-compatible HTTP and deterministic mock adapters
  mod.rs, tests.rs        Shared history, safe context, routing, validation, worker
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
  conversation.rs        Table Talk input, target selector, transcript/status, Bevy bridge
examples/batch.rs         Personality batch CLI
examples/simulate.rs      Original heads-up simulation
examples/ui_smoke.rs      Rendered interaction check and screenshots
assets/characters/       Three expression atlases, asset contract and prompts
config/                  Safe example conversation settings
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

`DialogueLine` carries session/hand epochs, speaker seat, text, optional expression and duration. `DialogueRequest` is the Bevy message input. Stage 4's `ConversationManager` owns one bounded shared transcript and at most one in-flight provider call. A human message prioritizes its target; table messages select a respondent, and occasional table exchanges may invite one NPC interjection. Public hand/action/win cues and idle time may start a conversation, subject to cooldown. It never requests poker actions from a model.

The conversation context builder whitelists street, board, pot, public stacks/actor, last five public actions and recent awards from an `Observation`; it does **not** serialize its hole cards, revealed private cards, privileged event log or shuffle seed. A worker owns a cloned request and returns only JSON text. Parsing restricts speaker, expression and length; `Eliminated` and gameplay commands are not accepted. Bevy polls the result, checks session/hand epochs, and sends the validated line to the existing portrait/dialogue presentation. Provider errors or timeouts produce harmless preset text. This separation lets Stage 5 store selected dialogue/relationship facts without changing poker rules or NPC strategies.

See [the character asset guide](assets/characters/README.md) for the seven-expression atlas contract, exact dimensions, provenance, replacement instructions and optional-file fallback. See [Stage 3 verification and architecture notes](docs/stage3-verification.md) for tests and practical implementation details.

## Verification and simulation

See [Stage 4 verification](docs/stage4-verification.md), [Stage 3 verification](docs/stage3-verification.md), and [Stage 2 verification](docs/stage2-verification.md). VPIP/PFR/showdown/pot-win are per dealt hand; aggression/fold are per decision. Chips/hand counts commitments after refunds, including blinds. “Pot win” includes any shared or side-pot award; net chips and match wins are also reported. Seats rotate between matches to reduce fixed-seat bias. A cap reports incomplete matches rather than treating them as wins.

These are behavior diagnostics, not strength rankings. Uniform opponent ranges, small equity samples and a fixed heuristic policy are intentional limitations; there is no opponent learning or GTO solver. Other limits: fixed blinds, two/four-seat selectable configurations, generated portrait atlases, simple motion, instantaneous runouts, in-memory event history and no save/resume. Chat is short-text only, one visible recent-history panel, no streaming tokens, no cross-session memory, no model-driven bets and no moderation service. The mock dialogue is intentionally simple; natural conversation requires a suitable configured model.

### Manual GUI checklist

- Use your physical mouse/keyboard and display scaling: confirm all four names/cards, active highlight, chip counts and controls fit.
- Type valid/invalid raise totals, use presets, and verify illegal actions remain unavailable.
- Play folded hands, multiway showdowns and unequal all-ins; check hidden cards, main/side awards and total chips.
- Advance through eliminations to heads-up; if you bust, try spectating and Next Hand.
- Restart while an NPC thinks; verify no old action arrives. Finish a match, start another, and return to the menu.

- Observe thinking/raise/win/loss/fold/elimination reactions and their return to neutral. A folded player's private cards must stay hidden.
- Watch a dialogue bubble expire; restart during a reaction and confirm the old line/expression disappears.
- Check 1920x1080 and 2560x1440 on your display. The desktop used for verification clamps large windows to a 1920x1052 client area; true QHD remains a manual display check.
- Select each chat target, type/send with mouse and Enter, watch recent messages and temporary expressions, and verify wager entry still works. Stop LM Studio mid-request and restart the match; the poker game should continue, old replies should not appear, and the status should switch to fallback.
