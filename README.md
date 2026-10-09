# Poker Lab

**v1.0** — a native Linux Texas Hold'em game with three fictional opponents who have distinct poker styles, conversations and local memories. One table, four players, 1,000 virtual chips each, 5/10 blinds. No real money.

Ananya is composed and analytical, Freya is playful and aggressive, and Yuna is patient and quietly observant. They learn from your public poker actions over time. Rust controls betting; a local language model gives the characters their conversational voice. Poker remains playable without a model or saved data.

## Features

- Four-player no-limit Hold'em, side pots, split pots, elimination and heads-up play.
- Three illustrated characters with expressions and public-event reactions.
- Distinct adaptive strategies with bounded adjustments, confidence and sample-size requirements.
- Public table talk, private conversations, temporary moods and persistent social memories.
- Wide, scrollable Table Talk with complete wrapped replies and player input.
- **Hand Review:** step through the last 20 completed hands in the current match.
- **Statistics:** see actual rates and opportunity counts; Poker reads offers developer detail.

## Requirements and quick start

Tested on CachyOS/Arch Linux, Rust/Cargo 1.94.1 stable and AMD Radeon Vulkan graphics. Bevy is pinned to **0.18.1**; keep Cargo.lock. Both Wayland and X11 are enabled. Zed or any editor works.

You need a C compiler/linker, pkg-config, Linux window libraries and system SQLite. On Arch, check `base-devel`, `pkgconf`, `wayland`, `libxkbcommon`, `libx11`, `libxcb`, `sqlite`, `vulkan-icd-loader` and your GPU driver. Install only missing packages. `vulkaninfo --summary` helps diagnose graphics support. The first Bevy build can take several minutes and several GB.

From the repository directory, in bash or fish:

```sh
cargo run
cargo run -- --seed 42
cargo build --release
cargo run --release
# Run the release executable directly with repository assets:
env BEVY_ASSET_ROOT="$PWD" ./target/release/poker-lab
```

For a portable local folder, place the executable alongside a copy of `assets/`. Bevy finds those assets beside the executable. System graphics/window and SQLite libraries are still needed. LM Studio manages model files separately. This release does not include an installer.

## LM Studio

1. Open LM Studio, load one supported chat model, and start its local server in the Developer tab at `127.0.0.1:1234`.
2. Run `cargo run`. Poker Lab discovers the single **loaded** chat model. Launching the game does not start the server or load a model.
3. A validated generated response changes the status to **Local model connected**. **[scripted]** means mock/preset text; **[scripted fallback]** means inference failed.

The development machine has used the already-installed `google/gemma-4-12b-qat`:

```sh
~/.lmstudio/bin/lms status
~/.lmstudio/bin/lms ps
~/.lmstudio/bin/lms server start --port 1234 --bind 127.0.0.1
~/.lmstudio/bin/lms load google/gemma-4-12b-qat --context-length 8192 --yes
cargo run --example social_probe -- --require-model
```

Substitute your installed model if different. No paid API account is required. Multiple loaded models require an explicit model identifier.

Configuration precedence: `POKER_LAB_CONFIG` → ignored local `config/conversation.json` → built-in local defaults. Adapt [conversation.example.json](config/conversation.example.json) for overrides:

```sh
env POKER_LAB_CONFIG=config/conversation.example.json cargo run
```

Defaults use local discovery (`model: "auto"`), a 60-second deadline, 512 output tokens, structured JSON, reasoning disabled, 16 history messages and 240 dialogue characters. `max_dialogue_chars` supports 80–360; `history_limit` supports 4–32. `initiative_frequency: 0` disables unsolicited speech. `provider: "mock"` selects scripted dialogue; `enabled: false` disables generated conversation.

If a model rejects optional fields, set `reasoning_effort` to `null` and/or `structured_output` to `false`. Replies remain validated. If dialogue stops, check the source badge, `lms status`, and `lms ps`: models can unload when LM Studio closes or becomes idle. Diagnostics distinguish unavailable/unloaded models, timeouts, token limits, invalid JSON/schema, request rejection and worker failure.

The optional remote OpenAI-compatible adapter takes `provider: "remote"`, an HTTPS `base_url`, an explicit `model`, and the environment variable named by `api_key_env`. Never put keys in committed configuration. Ordinary play and automated tests need no remote credentials.

## How to play

- Select **Start Game**. You sit below, Ananya left, Freya above and Yuna right. D / SB / BB mark the button and blinds. Highlighting identifies the acting player.
- **Fold**, **Check / Call**, **Bet / Raise**, and **All-in** follow engine-provided legal actions. Click the wager to type, or use Min / 1/2 / Pot / Max. Enter confirms the amount; then click Bet / Raise. Escape cancels editing.
- Wagers are the **total contribution on this street**. Raising to 30 after posting 5 spends another 25. Short-stack calls spend only available chips.
- Review main/side-pot awards, then choose **Next Hand**. Eliminated players are skipped; if you bust, you can spectate. **New Match** resets chips; **Back to Menu** leaves the match. Saved social and strategic learning survive.
- In **Table Talk**, cycle Everyone / Ananya / Freya / Yuna. Everyone is public; addressing one NPC is private. Click the input, type up to 240 characters and press Enter or Send. Escape leaves chat. Chat typing does not submit wagers.
- Scroll over the transcript or use **Older / Newer**. **Latest** follows new messages. Full replies wrap, with visible scripted/fallback badges.
- **Memories** inspects stored experiences, relationships and save status. **Poker reads** inspects evidence and adaptations. Freya reacts sooner, Ananya waits for stronger evidence, and Yuna waits longest. Reads freeze at hand start. Social warmth and model output never choose bets.

### Hand Review

Open **Hand Review** after completing a hand. Previous / Next Hand selects among the last 20 completed hands in this match. Previous / Next Action steps through blinds, your deal, public actions, board cards, reveals, refunds and awards. Boundary buttons are disabled. **Return to Table** resumes betting where you left it.

The review shows stacks, street wagers, remaining pot and final main/side-pot awards. Cards appear at the appropriate event. You see your own cards and public opponent reveals, never folded/unshown opponent cards, burns or seeds. It does not grade decisions or call the model. New Match, leaving the table or closing the application clears reviews.

### Statistics

**Statistics** uses the existing persistent human opponent model: VPIP, PFR, 3-bet, fold to 3-bet, continuation bet, fold to continuation bet, river aggression, showdown frequency and receiving a showdown share. Each shows actual numerator/denominator and an opportunity definition. Zero opportunities show no rate; fewer than 30 are marked small. These are observed rates; the separate smoothed estimates remain in Poker reads.

“Hands” counts completed hands dealt to you. VPIP/PFR denominators require a preflop decision, excluding forced blind-only all-ins. River aggression is a fraction of eligible decisions that increased the wager, not a bluff rate. Any showdown share includes ties and side pots, not necessarily net profit. See [precise metric definitions](docs/stage7-verification.md#exact-metrics).

## Data, privacy and reset

Default database: `$XDG_DATA_HOME/poker-lab/poker_lab.db`, otherwise `~/.local/share/poker-lab/poker_lab.db`. `POKER_LAB_DB` overrides the path. `POKER_LAB_PROFILE=alice` selects `poker-lab/profiles/alice/poker_lab.db`; names allow letters, digits, underscores and hyphens, up to 40 characters.

```sh
env POKER_LAB_PROFILE=alice cargo run
env POKER_LAB_DB=/absolute/path/poker_lab.db cargo run
```

Memories and relationships persist per character; poker observations use separate tables. Each NPC retains at most 96 derived memories, retrieval supplies only a few relevant excerpts, and live conversation history is bounded. Existing SQLite schema 4 is retained: no reset or new migration is needed.

Private messages reach only their participant NPC and are excluded from public prompts. This is an information boundary, not encryption against other computer users. A configured provider receives the latest message, permitted history/memories, relationship/mood context and a public table snapshot. A **remote** provider sends that context off the machine, including private messages addressed to that NPC. Keys are not saved in SQLite or logs.

Database failures report a save problem and use temporary memory while poker continues. Normal shutdown drains queued writes with a bounded wait; abrupt exit or long outages can lose pending observations. Back up the database with the game closed; SQLite may use associated `-wal` / `-shm` files while open.

Close the game before inspection/reset. Apply the same profile/path environment variables to these commands when using another profile:

```sh
cargo run --example social -- inspect
cargo run --example social -- inspect-strategy
cargo run --example social -- reset-strategy --confirm
cargo run --example social -- reset-memories all --confirm
cargo run --example social -- reset-relationships all --confirm
cargo run --example social -- reset-everything --confirm
env POKER_LAB_PROFILE=fresh_test cargo run
```

Strategy reset leaves social data intact; memory reset leaves relationships and learning intact. Reset-everything clears derived state but retains historical audit events/summaries. A fresh profile also separates that history.

## Verification and architecture

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
cargo run --example batch -- 100 42 96 10000
cargo run --example adaptive -- 2000 42 32
cargo run --example social_probe
cargo run --example social_probe -- --require-model
cargo run --example ui_smoke -- --small --long-chat
cargo run --example ui_smoke -- --hd --long-chat
cargo run --example ui_smoke -- --qhd --long-chat
cargo run --example ui_smoke -- --small --local
```

Normal tests use temporary storage and mock/loopback providers. Graphical checks require a desktop/GPU and report actual window sizes. Model probes report actual generated replies separately from fallback. See the [v1.0 readiness report](docs/v1-readiness.md) for results and remaining manual checks. Earlier [dialogue diagnosis](docs/dialogue-regression.md) and [layout verification](docs/table-layout.md) remain developer references.

`poker/` owns rules/events; `npc/` owns strategies/opponent learning; `review.rs` converts trusted events into safe frames; `statistics.rs` formats existing counters; `memory/` owns SQLite/social state; `social/` owns moods/personality; `conversation/` owns dialogue/providers; and `game/` + `ui/` host Bevy. Portrait replacement is documented in [assets/characters/README.md](assets/characters/README.md).

## Known limitations and feature freeze

- Dialogue quality depends on the loaded model. Replies can be awkward or invent details; transport validation cannot guarantee grounding. Mock/fallback speech is intentionally scripted.
- No saved poker match or cross-session replay. Reviews retain 20 hands; an exceptional hand exceeding 4,096 frames is omitted rather than shown incompletely.
- The engine retains its audit log until New Match. All-in runouts resolve immediately; review shows their individual steps.
- Adaptive opponents are bounded heuristics with sampled equity, not professional poker bots.
- Optional `--heads-up` practice uses the original basic opponent.
- Simultaneous instances do not live-sync their already-loaded opponent estimates.
- Linux desktop is the supported target. No mobile, multiplayer, voice or cloud service.

v1.0 is in feature freeze. Further maintenance is limited to bug fixes, compatibility, performance, minor usability, and security/data-integrity corrections.
