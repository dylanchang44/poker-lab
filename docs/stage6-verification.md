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

The example config selects local inference at `http://127.0.0.1:1234/v1`; set its model identifier to a model loaded in LM Studio. `max_dialogue_chars` defaults to 180 and accepts 80–360. `max_output_tokens` defaults to 256. All other provider settings remain available. With no config, the deterministic mock is used. An unavailable local endpoint yields a short fallback and never blocks gameplay.

## Verification and acceptance

Executed October 4, 2026:

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

`cargo run --example social_probe` runs the first seven messages for all three characters without graphics or a real save file. Set `POKER_LAB_CONFIG` to use local inference. It refuses remote mode. This is an audition aid; its mock results test routing and schema plumbing only.

## Stage 7

Adaptive poker can remain a separate versioned opponent model built from legitimately observed gameplay, evaluated through seeded simulations and passed through the existing strategy interface. Social mood, relationship scores and generated claims must not become authoritative betting evidence. The poker engine and its legal-action validation need no conversation dependency.
