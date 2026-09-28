# Stage 3: characters at the table

Implemented and verified on 2026-09-28, using Rust/Cargo 1.94.1 and the existing pinned Bevy 0.18.1. Poker engine files and strategy algorithms were not changed. The only dependency feature addition is Bevy `png`; no dependency versions, services, system packages or API credentials were added.

## Identity migration

| Previous display | Current display | Stable ID / seat | Retained strategy |
| --- | --- | --- | --- |
| Mira | Ananya (Anya), 32, India | `mira` / `Seat::Npc` | Tight-aggressive |
| Jax | Freya, 24, Sweden | `jax` / `Seat::Jax` | Loose-aggressive |
| Nova | Yuna, 28, Japan | `nova` / `Seat::Nova` | Tight-passive |

`npc/profiles.rs` remains the source of display names, stable IDs and strategy parameters. New ANANYA/FREYA/YUNA constants retain legacy aliases. A regression test asserts all seven strategy parameters for each seat, not just the renamed labels. Historical Stage 2 measurements retain their original names in that report.

`characters::CAST` adds origin, age, portrait path, reaction intensity and a few sample dialogue lines. It refers to the same seats and does not duplicate names or strategy values. Ananya has intensity 0.35, Freya 1.0, Yuna 0.22; these affect motion and reaction duration, never betting choices.

## Assets and UI

Three original, generated, transparent RGBA expression atlases are included under `assets/characters/{ananya,freya,yuna}/expressions.png`. Each is 1536x1024 with eight 384x512 portrait cells. Seven cells implement Neutral, Thinking, Confident, Happy, Surprised, Disappointed and Eliminated; the eighth is a reserved neutral duplicate. They are actual illustrated adult human characters. See the [asset contract and replacement guide](../assets/characters/README.md) and [exact built-in generation prompts](../assets/characters/PROMPTS.md).

Bevy's AssetServer starts three loads at startup. Strong handles in the PortraitAssets resource retain the decoded textures across menus and matches. ImageNode rectangles select cells without loading additional files. Optional individual expression handles take priority once available; a loading/failed override falls back to the atlas. A missing atlas shows an explicit named fallback label, while gameplay continues. The behavior is covered with real Assets<Image> handle-resolution tests.

Portrait frames, two crossfade image layers, seat readouts, felt and dialogue entities persist through hands and restarts. Board/control entities are refreshed when input/game state changes. Components identify each entity's seat and role, so animation systems update the existing images/transforms/colors instead of spawning new portraits. Public stack/card readouts update only on engine-event revisions; short expression/action timers update each frame.

Animation uses frame delta time: 0.4-second entrance, 0.22-second expression crossfades, small sinusoidal idle motion, a brief action pulse, folded dimming and eliminated dimming/downward settling. Last-action text remains faintly readable after its emphasis fades. Animations do not submit or delay poker actions. The existing background NPC worker and thinking timer continue unchanged.

The layout uses a 1200x960 design size with a centered canvas and bounded UiScale derived from the client dimensions. Ananya is upper-left, Freya top-center, Yuna upper-right; community cards/pot stay central, human cards and controls below. The main menu previews the same cast. The new default window is 1280x960; the existing 1000x820 minimum remains supported.

Relevant pinned API references: [ImageNode](https://docs.rs/bevy/0.18.1/bevy/prelude/struct.ImageNode.html), [UiTransform](https://docs.rs/bevy/0.18.1/bevy/prelude/struct.UiTransform.html), [AssetServer](https://docs.rs/bevy/0.18.1/bevy/asset/struct.AssetServer.html).

## Public reactions and dialogue

GameSession is the trusted boundary between privileged GameEvent history and the presentation model. It emits only hand number/stacks, public actions, and final pot/awards/showdown-participation flags. Private CardsDealt/CardBurned events and shuffle seeds are discarded; even showdown hand ranks/cards are unnecessary for expressions.

- Active turn: Thinking, unless a brief public reaction is still finishing.
- Normal action: Neutral plus an action indicator; fold additionally subdues the portrait/cards.
- Public bet/raise/all-in: short Confident reaction.
- Award of a pot of at least 100 chips: Happy; a smaller award: Confident.
- Multiple award recipients: Surprised for recipients, reflecting the public shared outcome. This is not an equity-based prediction.
- Revealed showdown non-winner: Disappointed. A folded hidden hand does not receive a hand-strength reaction.
- Zero stack at settlement: persistent Eliminated, taking precedence over temporary reactions.

The pure Rust PresentationState owns expressions, action emphasis and dialogue expiry. It has no engine reference, hole-card fields or strategy RNG. A test compares the filtered presentation stream for different private deals and confirms presentation updates leave the engine unchanged.

DialogueLine contains session epoch, hand number, speaker seat, text, optional expression and duration. DialogueRequest wraps it as a Bevy message. The persistent dialogue panel resolves name/portrait from the speaker seat, displays literal text and expires automatically. Stage 3 uses only a few predefined lines, with one bubble at a time rather than a queue of obsolete chatter.

Session restart resets presentation and animation epochs. Late messages for a prior session or hand are rejected. A UI test starts dialogue, restarts, submits the old request and verifies both that the line is rejected and that portrait entity IDs were reused.

Stage 4 can run a provider outside the engine, construct dialogue from permitted public context and emit DialogueRequest on completion. The same epoch check protects against late provider replies. Poker decisions remain a separate Strategy interface; dialogue cannot call act(). No LLM API, conversations generated at runtime, memory, voice or persistence is implemented here.

## Executed verification

| Command/check | Result |
| --- | --- |
| Baseline `cargo test --offline` before changes | 40 tests passed |
| `cargo check --offline` | Passed |
| `cargo fmt --check` | Passed |
| `cargo clippy --offline --all-targets -- -D warnings` | Passed, no warnings |
| `cargo test --offline` | 47 passed: 36 library + 11 application tests |
| `cargo run --offline -- --seed 42` | Built and opened Poker Lab on Vulkan / AMD Radeon RX 7700 XT (RADV) |
| `cargo run --offline --example ui_smoke -- --small` | Passed at 1000x820 |
| `cargo run --offline --example ui_smoke -- --hd` | Passed; requested 1920x1080, compositor supplied 1920x1052 |
| `cargo run --offline --example ui_smoke -- --qhd` | Passed; requested 2560x1440, compositor again supplied 1920x1052 |
| PNG inspection | All three 1536x1024 RGBA files contain transparent pixels |
| `git diff --check` | Passed |

The graphical runner exercises Bevy pointer hit testing, portrait asset loading (large runs), Start Game, pot-size preset/raise, check/call play to a result, Next Hand, restart and menu return. Screenshots were saved to `/tmp/poker-lab-*.png`; table and result were inspected, including a Happy win portrait and the speaker's matching dialogue portrait. An initial run exposed an overlapping pot label and ambiguous test button lookup; both were corrected before the successful runs above.

All original Stage 1/2 rule tests remain. Existing UI name assertions now expect the new cast. New coverage checks identity/strategy/presentation mapping, all expression transitions, revealed-loss/elimination behavior, timed reset, speaker identity, missing optional assets, private-event filtering, unchanged engine state, and rejection of stale dialogue with persistent portrait entities.

## Limitations and manual checks

Artwork is generated illustration, with minor alignment/style variation possible between frames. Delivered cells are 384x512, below the suggested 768x1024; higher-resolution atlases fit the same contract. Motion is limited to transforms, tint and crossfades. Dialogue is predefined text; there is no voice, lip sync, memory or model integration.

The current desktop prevented a true QHD rendering check. Test on a 2560x1440 display and at your preferred scaling; the smoke runner prints actual client dimensions so a clamped request is not mistaken for full-resolution coverage.

Manual checks: use physical mouse/keyboard for bet entry, resizing and navigation; observe each character thinking, raising, folding, winning, losing and being eliminated; check multiway side-pot labels; restart during a reaction/dialogue; ensure folded cards stay hidden; temporarily configure a missing optional expression to observe atlas fallback. Complete a match and confirm New Match restores all portraits and stacks.
