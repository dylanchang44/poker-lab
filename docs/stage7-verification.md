# Stage 7 — adaptive opponents and readable Table Talk

Implemented against `main` / `origin/main` at `d108123` (rechecked October 8, 2026). No dependency, base personality, evaluator, provider or poker-rule replacement. Existing Stage 0–6 tests remain; the migration fixture now correctly removes Stage 7 tables when constructing a simulated old database.

## Architecture and changed modules

```text
Poker engine: validates actions, emits DecisionOffered + PlayerActed
    │ public event whitelist (no private replay payload retained)
    ▼
npc/opponent.rs: completed-hand samples → long totals + last 50 hands
    ├── memory worker → existing SQLite database, schema 4
    ├── bounded text hints → conversation context → local/mock/remote provider
    └── npc/adaptation.rs: character-specific hand-start snapshot
                ▼
       existing PersonalityStrategy::decide(&Observation)
                ▼
       validated Action → authoritative poker engine

Social memories / relationships / mood ──► conversation only
LLM output ──► validated dialogue / expression only, NEVER betting
```

- `poker/events.rs`, `engine.rs`: public pre-action opportunity facts, emitted only after successful action validation. Illegal actions still leave history/state unchanged.
- `npc/opponent.rs`, `opponent_tests.rs`: reducer, counters, estimates, recent window, labels, tests.
- `npc/adaptation.rs`, `personality.rs`: explainable modifiers, existing policy integration and legal sizing buckets.
- `npc/simulation.rs`, `examples/adaptive.rs`: extend the existing simulation/collector with synthetic human policies.
- `memory/repository.rs`, `memory/mod.rs`, `memory/tests.rs`: schema migration, transactional model persistence, existing worker/retry integration and tests.
- `game/mod.rs`, `ui/learning.rs`, `ui/mod.rs`: event consumption, per-hand strategy snapshots, persistence bridge and optional debug panel.
- `conversation/mod.rs`, `provider.rs`, `ui/conversation.rs`: high-level public reads, prompt guardrails, full wrapped transcript/input and scrolling.
- `ui/table.rs`, `ui/characters.rs`: reserve space for chat, cards, controls and wrapped temporary dialogue.
- `examples/social.rs`, `examples/ui_smoke.rs`: independent reset tools and long-chat/debug/restart checks.

## Exact metrics

These are explicitly defined application metrics, not a claim of complete compatibility with commercial trackers. A voluntary increase includes a legal short all-in raise; an all-in call is not a raise. All rates retain numerator and denominator. Opportunities with no possible decision are not fabricated.

| Metric | Numerator | Denominator / opportunity |
| --- | --- | --- |
| VPIP | Hands with voluntary preflop chip payment | Completed human-dealt hands in which the human had a preflop decision; excludes blind-only forced all-ins and walks with no decision |
| PFR | Hands with a voluntary preflop increase above the current wager | Same decision-hand denominator as VPIP; at most once per hand |
| Limp | Hands with a preflop call before any voluntary raise | Same denominator; includes overlimps and small-blind completion |
| 3-bet | Human raises at the opportunity | First human decision per hand facing exactly one previous preflop increase, with legal raising rights and chips to increase |
| Fold to 3-bet | Human folds at the opportunity | Human was the original opener and faces exactly the second preflop increase; excludes intervening 4-bets; once per hand |
| Continuation bet | Human bets at the opportunity | Human was the last preflop raiser, can bet on the flop, and no one has bet yet; a donk bet removes the opportunity |
| Fold to c-bet | Human folds at the opportunity | Facing the first unraised flop bet from the last preflop raiser; intervening raises excluded; once per hand |
| Turn aggression | Human increases the wager | Human turn decisions where a bet/raise, including a short all-in increase, is legal |
| River aggression | Human increases the wager | Same definition on the river; this is NOT an aggression factor or a measured bluff rate |
| Fold to river bet | Human folds | First decision facing the unraised opening river bet; excludes facing raises |
| Fold to large river bet | Human folds | Above opportunity when the opening bet is at least 75% of the pot before that bet |
| Went to showdown | Human appears in public showdown | Human reached the flop without folding; includes all-in runouts |
| Won showdown share | Human receives any public award | Human appeared at showdown; split/side-pot shares count as a success, not fractional wins |
| Average bet size | Sum of `paid / pot_before`, in integer basis points | Human opening postflop bets, including all-ins; nonzero pot |
| Average raise size | Sum of `(new_total - previous_high) / (pot_before + amount_to_call)`, in integer basis points | Human wager increases over an existing wager, including preflop raises; nonzero pot |

Bet-size counters store sums of basis points, **not success counts**; their mean is `sum / (10_000 × opportunities)`. They are not passed through the probability smoother. Integer division introduces less than one basis point of rounding per observation.

The reducer pairs `DecisionOffered` with the validated action. Its pre-action wager, pot and raising permission resolve short-stack/reopening ambiguities that cannot reliably be reconstructed from action names alone. A completed hand is consumed once. Incomplete abandoned hands do not affect persistent statistics. Previous-version privileged histories are not retrospectively mined.

Not implemented: a separate check-raise frequency, inferred bluff frequency, hand-range inference, or proof that large river bets are rarely bluffs. Public aggression alone cannot establish hidden bluff intent.

## Confidence, recent form and style labels

For a binary metric with `s` successes and `n` opportunities:

```text
smoothed = (s + 12 × prior) / (n + 12)
confidence_weight = n / (n + 40)
recent_weight = 0.45 × min(recent_opportunities / 25, 1)
effective_estimate = (1 - recent_weight) × long_smoothed
                    + recent_weight × recent_smoothed
```

The 12-opportunity prior is VPIP .30, PFR .20, limp .10, 3-bet .08, c-bet .55, turn/river aggression .30, showdown .35; other binary metrics use .50. Confidence is an understandable **evidence-weight heuristic**, not a calibrated posterior probability or credible interval.

Long totals include all committed hands. Recent samples retain the last **50 completed human-dealt hands**, including samples with no relevant opportunity. Sparse river observations therefore receive less recent weight. Five unusual hands cannot erase a large history; sustained new behavior progressively changes the recent component. Old sparse tendencies can remain influential until new opportunities exist.

Archetypes remain `Unknown` below 40 VPIP opportunities. Thereafter effective VPIP/PFR produce Nit, Tight passive/aggressive, Loose passive/aggressive or Balanced/mixed. Labels are convenient descriptions, not policy inputs. The overfolder simulator, for example, can have a mixed VPIP/PFR label while its conditional fold statistics correctly activate pressure rules.

## Character-specific adaptation

| Character | Minimum opportunities per rule | Minimum confidence weight | Scaling |
| --- | ---: | ---: | ---: |
| Freya | 12 | .23 | 1.00 |
| Ananya | 30 | .42 | .80 |
| Yuna | 55 | .56 | .55 |

Both opportunity and confidence gates must pass. Rule strength is `clamp(distance beyond boundary / .22, 0, 1) × scaling × confidence`. At most **three** rules activate, in the following priority order:

| Observed estimate | Adjustment at maximum strength |
| --- | --- |
| Fold to river bet < .32 | River bluff −.08, value +.10 |
| Fold to 3-bet > .62 | Selective preflop pressure +.12 |
| Fold to large river bet > .65 | River bluff +.08; eligible bluff sizing targets 50% pot |
| PFR > .36 | Preflop pressure +.08, caution +.035, strong-hand traps (.30 Yuna / .10 others) |
| VPIP > .45 | Preflop pressure +.06, value +.07 |
| C-bet > .72 | Selective strong flop counters +.12 |
| River aggression < .14 | Marginal river-call caution +.06; not a claim of known hand strength |

Aggregate caps: pressure .12, value .10, caution .08. Effective aggression caps at .98; selectivity stays .08–.94; river bluff stays nonnegative and at most base+.08 (also capped .24). Static `profiles.rs` values are unchanged.

The existing policy uses these modifiers for admission thresholds, selective preflop counter-pressure, stronger flop counters, strong-hand checks/traps, pot-odds call margins, value thresholds, bluff eligibility and sizing. Sizing chooses 33/50/66/75/100% buckets only when adaptations are active, then respects legal min/max raises, stacks and existing speculative-wager safety checks. Bluff eligibility still needs suitable draw/blocker/public-board conditions; aggression alone never causes a weak automatic shove.

Reads freeze at **hand start**, including the debug strategy view. Completed-hand counters update separately. No recalculation in the middle of a hand replaces a worker's read. An adaptive strategy receives only its observation plus a cloned hand-start adjustment, never a database or conversation handle. Once the human folds, human-specific modifiers stop applying. Normal four-player elimination into heads-up remains adaptive; explicit legacy `--heads-up` practice remains BasicNpc.

## Persistence, information boundaries and resets

Schema 4 adds two tables to the **existing** SQLite database:

```sql
opponent_model(id INTEGER PRIMARY KEY CHECK(id=1), model TEXT, updated_at INTEGER)
opponent_commits(id INTEGER PRIMARY KEY, event_key TEXT UNIQUE)
```

The single JSON model contains integer long totals and a bounded 50-hand window; estimates are derived, not independently stored. Stable metric array order is part of this schema. Future metric reordering requires a migration. A transaction records the deduplication key, reads/updates the aggregate and prunes commit keys to 1,024 (larger than the existing 256-write retry journal). Reads occur after acquiring the write transaction to avoid lost updates between writers.

The in-memory model serves decisions. Completed hands enqueue work on the existing bounded SQLite worker; no per-decision database calls and no LLM dependency. Normal worker flush/retry/fallback behavior is reused. A long storage outage/queue overflow or abrupt exit can lose uncommitted observations; gameplay remains available. Two simultaneous app instances preserve database increments, but their already-loaded in-memory reads are not live-synchronized. Close the game before using reset tools.

The reducer explicitly ignores hidden deals, burns and shuffle seeds. Showdown handling retains only public participation and awards, not cards. No hidden bluff labels are inferred. Social messages, warmth, trust, irritation and model-generated claims cannot become statistical evidence. All NPCs share the public human counter history, but interpret it through distinct gates/modifiers; this is not three private per-seat databases.

Conversation context can receive at most two evidence-gated qualitative observations. It receives no raw numeric model or hidden cards. Hints accompany public/idle cues; ordinary human messages retain their topic priority. The provider prompt prohibits inventing statistics or controlling betting. The provider abstraction, local LM Studio preference and deterministic mock/fallback are unchanged.

Database: `$XDG_DATA_HOME/poker-lab/poker_lab.db`, otherwise `~/.local/share/poker-lab/poker_lab.db`; `POKER_LAB_DB` overrides it. A fresh `POKER_LAB_PROFILE` separates both social and strategic history.

```sh
cargo run --example social -- inspect-strategy
cargo run --example social -- reset-strategy --confirm
cargo run --example social -- reset-memories all --confirm
cargo run --example social -- reset-relationships all --confirm
cargo run --example social -- reset-everything --confirm
POKER_LAB_PROFILE=fresh_test cargo run
```

The first reset leaves social data intact; memory-only and relationship-only resets leave strategy intact. Reset-everything clears derived state but intentionally retains historical audit events/session summaries. Use a fresh profile for completely separate history. No real user save was reset during verification.

## Headless evaluation

Executed `cargo run --example adaptive -- 2000 42 32`: **10,000 hands**, four fixed synthetic styles plus loose-aggressive → tight-passive halfway through the fifth batch. All submitted decisions are validated and each action asserts conservation of 4,000 chips. Whole-table bankrolls reset when anyone busts so every NPC remains sampled. This is a rebuy behavior experiment, not a tournament win-rate experiment.

Each table entry below is **completed hands before first adaptation / number of decisions differing from a cloned base-only policy on the same observation and RNG state**:

| Synthetic human | Ananya | Freya | Yuna |
| --- | ---: | ---: | ---: |
| Tight passive | 316 / 65 | 316 / 175 | 520 / 39 |
| Loose aggressive | 33 / 450 | 14 / 1,248 | 61 / 274 |
| Calling station | 35 / 751 | 14 / 1,548 | 64 / 264 |
| Overfolder | 35 / 345 | 13 / 744 | 63 / 150 |
| Loose aggressive → tight passive | 33 / 259 | 14 / 741 | 61 / 147 |

Against the calling station, Ananya/Freya/Yuna respectively produced VPIP **13.3/30.3/7.5%**, PFR **8.5/19.7/2.2%**, aggression per action **20.8/33.8/8.3%**, and mean aggressive wager increments **71.8/93.2/61.3%** of pot-after-call. Hand awards were **294/499/214** out of 2,000; net chips **+39,354/+69,271/+12,034**, across rebuys. These numbers do not prove strength or improvement over a full counterfactual baseline match.

The overfolder produced **109/128** folds to 3-bets and **109/135** folds to opening river bets. The calling station produced **0/617** river folds. The style-switch batch retained long PFR **634/1,758**, while recent PFR fell to **2/45** and its blended estimate to **.232**; final preflop pressure returned to zero without deleting historical counts.

The CLI additionally prints all NPC action counts, policy-tagged non-value aggressive actions, sizing, pot-share wins, chip net and final rule explanations. “Non-value aggression” is an internal policy diagnostic, **not proof of an actual bluff**. Different styles visit different streets, so raw bluff counts are not directly comparable. Fixed seeds reproduce results for the pinned dependencies and sample count. The automated suite repeats seeded batches and checks decision changes.

## Table Talk and text clipping

The old transcript explicitly shortened messages to roughly 64 characters, so increasing the provider limit could not fix what the player saw. A narrow 250px panel, clipped fixed-height history and an 18-character input tail compounded the problem.

Now the design-space panel is **380×400**, with a 190px scrollable history and 360×98 wrapped input. The complete bounded transcript is rendered with speaker lines, public/private context and subdued source badges. No message substring/tail truncation remains. `WordOrCharacter` wrapping handles ordinary sentences and unbroken text. **Older** moves upward; **Latest** follows new messages. Bevy computes the scroll extent after UI layout, rather than guessing a fixed number of text rows.

The 1,040-unit-high table canvas scales with the window. A reserved left chat column leaves community cards, pot, human cards and betting controls to its right. Portrait dialogue wraps in a separate band above human cards. Existing length-dependent display duration remains capped at ten seconds; expired bubbles remain in scrollable history. This is still simple append/backspace input, not a full cursor/selection editor. Long history requires scrolling; unsupported font glyphs remain subject to the existing font's coverage.

`ui_smoke --long-chat` validates a 360-character reply, 240-character full input, 180-character public reply, private labels, fallback badge, multiple wrapped messages, Older/Latest, computed newest-message fit and speech/chat bounds. Ordinary smoke still covers short replies and gameplay. It also opens the strategy/metric overlays, completes a hand, advances, restarts and returns to the menu. Fixtures use mock inference and isolated memory by default.

## Verification and remaining acceptance work

Executed October 8, 2026:

| Check | Actual result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test` | Passed: 96 library + 12 application tests = **108**, zero failures; original baseline was 94 |
| `cargo build` | Passed |
| `git diff --check` | Passed |
| `cargo run --example adaptive -- 2000 42 32` | Passed twice, 10,000 hands each; complete stdout byte-identical |
| `ui_smoke --small --long-chat` | Passed at **1000×820 logical/physical**, including debug panel, full-length chat and control interaction |
| Separate second process, same disposable DB, `--small --long-chat --restored` | Passed; prior social memory and opponent model restored, two completed hands persisted |
| `social -- inspect-strategy` on that disposable DB | Passed; all three reads correctly Unknown with two hands, unchanged base parameters |
| `ui_smoke --hd --long-chat` | Passed at actual **1920×1052 logical/physical**, not requested 1920×1080 |
| `ui_smoke --qhd --long-chat`, Wayland | Passed at actual **1920×1052**, compositor resized request |
| Same QHD command using X11 (`env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET`) | Passed at **2560×1052 logical / 3200×1315 physical**, not 2560×1440 |
| `cargo run -- --seed 42`, disposable DB and mock config | Created the Poker Lab window using Vulkan/RADV on RX 7700 XT; intentionally interrupted by a 12-second timeout (exit 124, **not** a clean-exit test) |

Graphical checks drive Bevy's real pointer hit-testing/input messages and Vulkan renderer; they are automated graphical checks, not a claim of human physical-mouse playtesting. Generated maximum-message screenshots were visually inspected: full text/end marker and full input were visible with the cards and controls unobscured. Initial test captures were moved one frame later to allow scroll layout to settle before inspection. **Exact 1080p and QHD remain manual checks** because the available compositor resized those windows. No paid API, internet inference or live LM Studio was used for these tests. Existing local/mock provider tests still pass; no new natural-language-quality claim is made.

Temporary evidence: `/tmp/poker-lab-stage7-tests.log`, `/tmp/poker-lab-stage7-adaptive-final.log`, `/tmp/poker-lab-stage7-small.log`, `/tmp/poker-lab-stage7-restored.log`, `/tmp/poker-lab-stage7-hd.log`, `/tmp/poker-lab-stage7-qhd-x11.log` and `/tmp/poker-lab-*.png`. These are local verification artifacts, not required game assets. Persistent restart verification used `/tmp/poker-lab-stage7-RHxNb5/restart.db`, not the user's normal database.

Manual follow-up:

1. `cargo run`, start a normal four-player match, open Poker reads, then close it for normal play.
2. Play repeated overfolding/calling/aggressive sessions. Verify rule evidence grows, Freya generally reacts sooner, and no adaptation activates on tiny samples.
3. Change style for at least 50–100 hands; compare long and recent counters without expecting sparse river metrics to change instantly.
4. Restart the app and check learned counts; New Match must preserve them. Try a separate profile without altering the original.
5. Restart/menu immediately after a completed hand; ensure no duplicate count or stale worker action.
6. Converse normally with a configured local model. A relevant public read may appear occasionally, not in every greeting. Live-model wording/quality of the new hints has not been manually assessed.
7. Stop the local server or make a disposable database unavailable: poker must continue; fallback/save status should be visible.
8. Read a maximum-length reply, scroll older messages, type a full sentence, and verify controls remain usable at your actual desktop scaling and QHD resolution.

## Limits and Stage 8

This is bounded exploitative heuristic learning, not reinforcement learning, a GTO solver, a calibrated range model or perfect knowledge. Uniform sampled opponent ranges remain. Four simple synthetic profiles are stress fixtures, not competent human benchmarks. No assertion is made that these modifiers maximize profit. The shared model covers the human only; social state intentionally never changes poker decisions. Recent history is hand-window-based, not wall-clock decay. Completed hands persist; mid-hand save/resume and full replay UI remain out of scope.

Recommended Stage 8: replay/decision-analysis tools and better evaluation fixtures. Surface public evidence, frozen reads and legal alternatives for completed hands; compare base/adaptive policies over multiple seeds and seat rotations. Keep privileged replay data separate from live observations. This would make future improvements measurable before adding richer hand-range inference or more exploit rules.
