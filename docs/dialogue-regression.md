# Stage 7 dialogue investigation

Compared the current workspace against `d108123b959f83376287fe8e658265b02819d995`. Investigation and live tests: October 8, 2026. The workspace already contained uncommitted Stage 7 work; it was preserved, not reverted.

## Findings

1. **Unavailable local inference reproduced the canned replies.** Initially, `127.0.0.1:1234` refused a connection and `lms status` reported the server off. After waking LM Studio, `lms ps` still showed no loaded models. The original 33-message probe produced **0 model replies / 33 scripted fallbacks**, with `load a model` status. The repeated sentences matched `fallback_line`, not model output.
2. **Loading the existing model restored real conversations even before the code changes below.** The installed `google/gemma-4-12b-qat` was the model documented in the successful Stage 6 acceptance run. After loading it, the original Stage 7 probe completed **33 model replies / zero fallbacks**. No model download, API key or remote service was used.
3. **A subsequent extended run exposed an intermittent LM Studio failure.** It produced **38 model replies / one fallback**. The server log at 23:25:57 identified `Error: Channel Error`, caused by `Engine protocol predict request failed: fetch failed`. Freya's music request was rejected in 0.23 seconds; requests before and after it succeeded. This was not a parsing rejection, token-limit exhaustion or a 60-second timeout.
4. **A pre-existing unnecessary fallback path was found in code.** A valid reply identical to the character's preceding line was replaced with canned text, even when answering a new direct human message. That behavior also existed in `d108123`; it was not introduced by poker learning.

The comparison found **no Stage 7 change** to provider-selection defaults, local configuration loading, character profiles, response schema, inference token/timeout settings or background-worker concurrency. Current local settings remained `provider: local`, `model: auto`, 60 seconds, 512 output tokens, 240 dialogue characters, reasoning disabled and JSON-schema output enabled. Discovery selected the single loaded Gemma instance, with an 8,192-token context.

Stage 7 had added an empty `strategic_reads` field and an unconditional strategic-instruction paragraph to the prompt, plus up to two public reads for event/idle comments. Direct player submissions already built empty reads. The successful online baseline provides no evidence that these additions caused the repeated canned text. They have nevertheless been isolated more strictly to preserve ordinary conversation. No worker congestion was observed in the sequential live probes; the SQLite adapter and strategy workers were not removed or combined with dialogue inference.

These findings identify reproducible causes in this environment, not a claim to reconstruct every earlier player session for which no diagnostic log was supplied.

## Changes made for this fix

| File | Change |
| --- | --- |
| `src/conversation/diagnostics.rs` | Host-owned failure categories and model/scripted/fallback/silence/stale counters; no prompt or credential retention |
| `src/conversation/mod.rs` | Accurate failure/status mapping; distinguish disconnected workers from timeouts; report stale/cancelled work; preserve valid repeated direct replies rather than substituting canned text |
| `src/conversation/provider.rs` | Omit strategic context from direct replies, add strategic instructions only when relevant reads exist; one local HTTP 5xx retry within the original deadline; safe HTTP status logging; preserve body-read timeout diagnostics |
| `src/conversation/tests.rs` | Deterministic diagnostic/privacy, direct-reply, optional-context, worker/stale and bounded-retry tests |
| `examples/social_probe.rs` | Thirteen messages per character, including all six exact requested scenarios; elapsed times and host-owned provenance counters |
| `examples/ui_smoke.rs` | Live-model UI check now also asserts the connected label and zero fallbacks |
| `README.md` | Explicit local server/model startup checks, safe diagnostics and probe instructions |
| `docs/dialogue-regression.md` | This evidence/report |

No poker rules, opponent-model counters, adaptive modifiers, strategic persistence, NPC profiles, social memory, relationships or moods were changed by this corrective task. Those earlier Stage 7 workspace changes remain intact. No new dependency was added.

## Failure handling

The UI and diagnostics distinguish unavailable server, no loaded model, ambiguous model selection, timeout, output-token limit, invalid JSON, unsupported response schema, rejected request, empty/overlong dialogue, worker failure and discarded stale responses. Model/scripted provenance is assigned by Rust, never trusted from generated text.

Only a **local completion HTTP 5xx** can receive one serial retry. Discovery and both completion attempts share the original deadline. HTTP 4xx, malformed output, schema violations, output limits and remote providers do not receive retries. Validation is never loosened to make a test pass. Tests exercise 503→200 recovery, repeated 503 stopping after two attempts, and 400 stopping after one attempt. Provider error bodies, credentials and private prompts are not logged; development logs contain only fixed categories and numeric HTTP status codes.

The existing scripted fallback remains available when inference genuinely fails. It stays explicitly labelled; it is not a substitute for natural conversation. Unsolicited duplicate suppression remains, while direct valid model replies keep their actual model provenance. All poker decisions still run exclusively in Rust.

## Running locally

The game discovers a loaded model; it does **not** automatically launch LM Studio or load arbitrary downloaded models. Starting the server alone is insufficient.

```sh
~/.lmstudio/bin/lms status
~/.lmstudio/bin/lms ps
# If needed; this model is already installed on the development machine:
~/.lmstudio/bin/lms server start --port 1234 --bind 127.0.0.1
~/.lmstudio/bin/lms load google/gemma-4-12b-qat --context-length 8192 --yes
cargo run --example social_probe -- --require-model
cargo run --example ui_smoke -- --small --local
cargo run
```

Only one chat model should be loaded with `model: auto`; otherwise choose an explicit model in ignored local `config/conversation.json`. Keep mock mode for deterministic/offline development. No automatic model download, remote key provisioning or persistent service configuration was introduced.

## Conversation quality and limits

The initial offline music question received the same generic fallback as unrelated messages: Ananya's “Interesting thought. Tell me more.”, Freya's “Ha! I like where this is going.”, and Yuna's “I'm listening.” These were not intelligent model answers.

After restoring real inference, the pre-code-change baseline already answered music, invitations and fatigue on topic and recalled the explicitly mentioned jazz preference. This distinguishes restoring inference availability from claiming a new model-quality breakthrough. The final acceptance results are recorded below.

The probe uses isolated temporary memory, not the player's save. Therefore “yesterday” should produce uncertainty, not invented recall. Persistent-memory/privacy integration remains covered by automated tests. Real-model answers are stochastic and sometimes still contain poker banter or unnecessary assumptions; the transport fix does not guarantee perfect fictional consistency or perfect refusal of assistant-like phrasing. The local model must remain loaded after this task; restarting LM Studio or an idle unload can make it unavailable again.

## Final verification

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: **113 passed** (101 library + 12 application), zero failures. All Stage 7 adaptation/persistence tests remain. Five new conversation tests cover the changes above; all automated tests use mock/loopback fixtures, not a live model.
- `cargo build`: passed.
- `git diff --check`: passed.
- `cargo run --example social_probe -- --require-model`: **39 real model replies, zero scripted replies, zero fallbacks, zero silent/missing/stale replies**. All six requested messages ran against all three NPCs. Replies were nonempty and passed speaker, JSON, schema, expression and length validation. Successful final-run requests took roughly 3.6–5.9 seconds, below the unchanged 60-second deadline.
- `cargo run --example ui_smoke -- --small --local`: passed, **one real model reply and zero fallbacks**. The test asserted `Local model connected`, real provenance, private input/routing, completed-hand navigation, strategy overlays, restart and return to menu. Actual window: **1000×820 logical / 1250×1025 physical** at desktop scaling. This was a real rendered/input-driven GUI check, not a manual physical-mouse session.
- Model discovery confirmed the sole loaded instance `google/gemma-4-12b-qat`, context length 8,192. No new model was downloaded. The local server/model were left running for gameplay.

Counts are per completed run: initial offline 0/33 model/fallback; restored pre-fix baseline 33/0; extended intermediate run 38/1; final probe 39/0; final GUI 1/0. An interrupted probe is excluded from these completed-run totals. The final run did not need a transient retry; deterministic HTTP tests exercised retry recovery and limits.

Examples from the final live transcript (not canned replacements):

| Player topic | Observed reply |
| --- | --- |
| Ananya, “Good morning.” | “Good morning. I hope your day is starting off well.” |
| Ananya, invitation | “That's a very direct invitation. I'm not quite ready to move from the table to a social outing just yet.” |
| Freya, music | “I love it! It's my go-to for getting into the zone. I'm a bit of a mixed bag, though—anything from upbeat synth-pop to something a bit more mellow and moody works for me. What's on your playlist?” |
| Freya, tiredness | “I hear you. Sometimes the day just drains everything out of you. Is it the kind of tired where you just want to zone out, or are you looking for something to actually distract you?” |
| Yuna, invitation | “That's a very direct question. I think I'd prefer to get to know you a bit better here first before making any plans.” |

All three answered the music/fatigue topics rather than returning the previous generic fallbacks, and the jazz short-term recall checks worked. Limitations observed during review: greetings/invitations still sometimes include poker banter; Freya invented a caffeine detail; missing-yesterday-memory answers sometimes overclaimed that the characters had “just met” or speculated that the player was confused. These are remaining model-grounding/style imperfections, **not** transport successes being passed off as perfect conversational quality. No meaningful previous-day memory was seeded in this isolated probe.

Local evidence from the final run: `/tmp/poker-lab-dialogue-final.log`, `/tmp/poker-lab-dialogue-gui.log`, `/tmp/poker-lab-dialogue-tests-final.log`, `/tmp/poker-lab-dialogue-unit-final.log`, `/tmp/poker-lab-chat.png`. The intermediate failure is in `/tmp/poker-lab-dialogue-after.log` and LM Studio's `server-logs/2026-10/2026-10-08.1.log`. Earlier temporary logs disappeared during the interrupted environment; their observed results are recorded above. Synthetic probe transcripts contain no real saved player conversations.
