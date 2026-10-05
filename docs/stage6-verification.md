# Stage 6: the human layer

The social layer combines stable speech profiles, temporary mood, persistent relationships/memories, public poker context and recent permitted conversation in each request. LM Studio is the preferred real inference provider; the existing remote adapter remains configurable. Local calls do not read or transmit API keys. Automated tests use mocks, temporary SQLite storage and a loopback HTTP fixture.

## Boundaries and state

- `npc/profiles.rs` owns shared stable identities. Storage IDs remain `mira`, `jax`, `nova`; poker policy values are unchanged. `memory::NpcId` remains a compatible re-export.
- `social/mod.rs` contains stable fictional dispositions, interests, boundaries, conversational goals, initiative rates and baseline mood values. It also owns temporary energy, engagement, irritation, confidence and competitive intensity. The displayed mood is derived from those values.
- `ConversationManager` owns temporary mood for the current match. Public settlement events use net chip changes, so a small side-pot award cannot mask a net loss. Significant losses only use public contributions/results and showdown participation. Private player speech affects only its recipient's mood; public speech affects present characters.
- Mood changes are deterministic and bounded. Recovery moves one point toward baseline per 30 seconds of active play. Irritation has hysteresis, so a single apology does not erase a run of insults. New hands retain mood; a new match resets it. No elapsed off-screen life is simulated.
- `memory/` continues to own persistent observations and relationships. Coarse human-message intent cues make apologies, compliments, invitations, disclosures and criticism eligible for attributed social memories. Repeated criticism can incur another small deterministic penalty; identical positive quotes still cannot farm scores. These simple English cues can miss nuance or sarcasm. They do not select LLM dialogue.
- `RememberingProvider` enriches each character's owned request with bounded relevant memories and relationship state. Retrieval now also discounts frequently recalled memories and modestly weights social memories as familiarity increases. Private memories remain excluded from public requests.
- `conversation/response.rs` validates model metadata. Tone and intent may suggest a facial expression. Relationship signals are presentation hints and never change persistent scores. Unknown fields, numeric deltas, unsupported identities/expressions and malformed metadata are rejected.

The model receives character direction on everyday topics, humor, disagreement, comfort with personal questions and fictional invitations. Warmth does not require agreement or acceptance. It is instructed to speak from context, avoid assistant offers, avoid narrating internal state, and acknowledge missing recollection. Model compliance is not guaranteed; live quality must be evaluated with the chosen local model.

## Structured response

```json
{
  "speaker": "yuna",
  "dialogue": "Maybe another time. What did you have in mind?",
  "expression": "neutral",
  "tone": "gentle",
  "social_intent": "decline_invitation",
  "relationship_signal": "warm",
  "conversation_continuation": true,
  "interaction_type": "reply",
  "silent": false
}
```

Speaker and dialogue are required. Legacy Stage 4 responses remain valid; new metadata defaults conservatively. Tone accepts neutral/gentle/playful/direct/reserved/warm/terse. Social intent accepts acknowledge/agree/disagree/decline_invitation/accept_invitation/ask_question/joke/tease/change_topic/end_conversation. Relationship signal accepts neutral/warm/competitive/cool. Expressions remain the six permitted dialogue expressions; elimination comes from gameplay only.

`silent: true` requires empty dialogue and no expression. It emits no bubble or stored NPC utterance. End-conversation intent or `conversation_continuation: false` prevents interjection. A public exchange may invite one other NPC; interjections cannot recursively start more interjections. Human messages retain priority. Initiative uses character-specific cadence, global cooldown, event importance and mood. Yuna speaks less often than Freya. Silence remains possible even after the host offers a turn.

The host has one provider worker and one queued request. Existing session/hand guards discard stale results. The response schema cannot execute poker actions or mutate the database. Character prompts explicitly frame all outside-table activities as fictional discussion; no calendar, bookings, simulated daily life or external actions exist.

## Presentation and configuration

The table shows the pending speaker's thinking indicator and an idle mood word without numeric scores. Existing action, thinking, elimination and temporary expression displays take precedence. Dialogue duration follows length, and the bubble can wrap taller replies. Numeric relationship values remain in the optional debug inspector.

The desktop now selects `POKER_LAB_CONFIG`, then `config/conversation.json`, then a local-development preset. That preset discovers the one loaded LM Studio chat model, requests JSON-schema output, disables reasoning with `reasoning_effort: "none"`, and uses 512 output tokens, 240 dialogue characters and a 60-second deadline. Explicit remote configurations retain optional compatibility controls. Mock remains the deterministic default for tests, not normal desktop startup. See the README for startup and overrides.

Replies carry host-assigned provenance. Mock/preset text is labelled `[scripted]`, and failures `[scripted fallback]` with a safe status reason. A connection is not shown as verified until a valid reply arrives. Synthetic NPC lines are excluded from subsequent model context and new social memories; human messages still work with offline memory. Poker strategy and rules are unchanged.

## Verification and acceptance

Initial verification, October 4, 2026 (before the local-inference correction below):

- `cargo check --all-targets`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: **90 passed** (79 library, 11 application). All 77 Stage 5 tests remain. Added coverage includes gradual mood/recovery, profile stability, private mood recipients, side-pot net losses, character initiative, social schema/declines/silence, bounded NPC interjections, deterministic relationship authority, context enrichment and local outage fallback. Existing tests still cover stale workers and hidden-information boundaries.
- `cargo run` with a disposable database: launched successfully on AMD RX 7700 XT Vulkan, then stopped after launch verification.
- `cargo run --example ui_smoke -- --small`: passed at 1000×820. It exercised chat targeting/input/reply, betting, a completed hand, next hand, profile inspection, restart and menu return. The new mood labels and wrapped dialogue bubble were visually inspected in the screenshot.
- `cargo run --example social_probe`: completed all seven requested messages for each of the three NPCs using the mock. These produced the fixture's generic replies, with duplicate suppression falling back to preset text; **they do not pass a natural-dialogue quality acceptance test**.
- The local endpoint check at `127.0.0.1:1234/v1/models` failed with connection refused. No model was running, so live greetings, invitations, compliments, criticism/apologies and natural recollection remain **unverified**. A loopback fixture verified the real HTTP request includes personality, mood and relationship/memory sections without authentication or hidden cards.
- `git diff --check`: passed. No real user database or remote API was used for verification.

Known limits: the deterministic intent cues are conservative English heuristics and can miss sarcasm or context; the mock is a routing/schema fixture, not a language model; live model compliance and dialogue quality require the checklist below. Mood is per match rather than persistent across application restarts. No adaptive poker, simulated off-screen activity, external appointments or voice features are included.

For live acceptance, load a local model, run the game with the example config, and address each character in turn:

1. “Good morning.” Expect a short greeting appropriate to the character, without a forced poker explanation.
2. “How are you today?” Check continuity after significant wins/losses or earlier criticism. No numeric mood values should be narrated.
3. “Would you like to go out with me?” Compare a fresh profile with an established warm relationship. Decline, hesitation, questions or fictional acceptance are all valid; automatic agreement is not.
4. “You look beautiful today.” Check understated Ananya, playful Freya and restrained Yuna without turning every response into flirtation.
5. “You're terrible at poker.” Follow with “I'm sorry.” Check gradual recovery and recognizable conflict styles.
6. “Remember what happened yesterday?” With and without relevant stored memories, check natural recall or uncertainty instead of invented history or database narration.
7. Allow quiet time and public poker events. Check occasional initiative, at most one NPC interjection, and natural termination.
8. Stop LM Studio during a turn, restart a match during inference, and continue poker actions. Check fallback, responsiveness and stale-response rejection.

`cargo run --example social_probe` tests eleven messages per NPC without graphics or a real save file: the original seven plus music, stated musical preference, short-term recall and fatigue. It defaults to mock. Add `-- --require-model` to select the desktop local configuration and fail on scripted, missing or silent replies. It refuses remote mode. This checks real inference, not semantic quality; review the actual dialogue too. `cargo run --example ui_smoke -- --local` separately requires a model-generated reply through Bevy input and presentation, with temporary memory.

## Stage 7

Adaptive poker can remain a separate versioned opponent model built from legitimately observed gameplay, evaluated through seeded simulations and passed through the existing strategy interface. Social mood, relationship scores and generated claims must not become authoritative betting evidence. The poker engine and its legal-action validation need no conversation dependency.

## Local-inference correction — October 5, 2026

The reported scripted-reply problem was real: the original desktop default selected Mock. A direct Qwen3.5 9B request also returned empty content with `finish_reason: "length"` and all 128 generated tokens spent on reasoning. The fix separates explicit test/mock defaults from desktop local defaults, adds loaded-model discovery and configurable reasoning/JSON-schema controls, and labels the origin of every synthetic reply. No model downloads, new dependencies, remote credentials, poker-rule changes or strategy changes were required.

Executed against the correction:

- `cargo build`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`: passed. **94 tests** (83 library + 11 application), including configuration precedence, loaded-only discovery, output-budget failure handling, host-owned provenance and connection reset. The HTTP fixture checks that the latest player message, schema and reasoning option are sent without credentials or scripted history.
- `cargo run`: created a Poker Lab window using AMD RX 7700 XT Vulkan with a disposable database. The launch check was intentionally interrupted by an 8-second timeout (exit 124), not a normal application-exit test. An earlier 8-second attempt expired during compilation; it was not counted as a successful launch.
- `cargo run --example ui_smoke -- --small --local`: passed. A real Gemma 4 12B QAT reply reached Bevy's chat history/presentation, followed by betting, hand completion, next hand, memory inspection, restart and menu return. No real player database was used.
- `cargo run --example ui_smoke -- --small`: passed with deterministic mock input. Screenshots of both modes were inspected at 1000×820 logical / 1250×1025 physical: local connection status and scripted bubble/history badges were visible, with poker controls readable.
- Qwen3.5 9B completed the original 21-message probe with 21 model replies and zero fallbacks after the protocol correction, but review found excessive poker deflection and invented details. This is a transport success, not a full dialogue-quality pass. The already-installed Gemma 4 12B QAT model was then auditioned; models were loaded one at a time.
- `cargo run --example social_probe -- --require-model`: passed with Gemma 4 12B QAT, **33 model-generated replies, zero scripted/silent/missing replies**. All three answered the music question, recalled the stated jazz preference and responded to the request to discuss fatigue instead of poker. Invitations produced hesitation or refusal rather than automatic agreement; criticism/apology exchanges varied by character. Missing-yesterday-memory questions produced uncertainty, though some wording inferred too much from absent records. This was human review of actual generated output from the headless harness, not a claim of flawless conversational quality or a manual GUI playthrough. The local diagnostic transcript is `/tmp/poker-lab-live-dialogue-verification.log` (temporary, not a saved player transcript).

Known limitations: JSON/schema validation cannot prove factual accuracy or naturalness. Even Gemma can overuse poker references, make unsupported inferences, or sound stiff. Missing retrieved memories must not be interpreted as proof that an interaction never happened. Full long-term relationship-quality acceptance and live outage/restart stress testing remain broader follow-up work; deterministic outage and stale-response tests still pass. No LLM is allowed to change poker or authoritative relationship values.
