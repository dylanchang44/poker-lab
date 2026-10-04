# Stage 5: persistent memory and relationships

## Boundaries and architecture

`poker/` and `npc/` are unchanged. A trusted `PokerObserver` converts the privileged engine history into a small whitelist of witnessed facts. Hole-card deals, burns and seeds are ignored. Only publicly shown cards can support a high-card river-aggression observation; an all-in call is not aggression, and bluff intent is never inferred. Witness sets follow active hand participants.

`memory/models.rs` separates immutable observed facts from derived character memories and five relationship dimensions. Identity keys remain `mira`, `jax`, `nova`; profile definitions remain the display-name authority. Conversation audiences travel through both short-term history and stored memories. A private quote cannot enter another character's context, nor its owner's public comment. Public quotes propagate only to witnesses. The player-facing inspector can show all of the player's own exchanges.

`MemoryService` owns one storage worker and a 128-command channel. Bevy enqueues facts and reads a cached snapshot; SQLite never runs in the frame loop. A request worker wraps the existing provider with `RememberingProvider`, retrieves up to three permitted memories, and sends relationship/memory context separately from the current public poker snapshot. No strategy receives social scores or memories. Existing conversation session/hand guards still reject stale model replies.

The model can write dialogue only. It does not create authoritative poker facts, infer preferences, change relationship numbers, or call reset/database operations. Preferences are attributed player quotes, extracted deterministically. Model utterances may be retained as attributed social quotes, never as independently verified facts.

## Storage

Pinned `rusqlite 0.38.0` uses the existing system SQLite library. No ORM/server is involved. `PRAGMA user_version` applies transactional migrations (currently version 3), rejecting unknown future schemas. Foreign keys, WAL, synchronous FULL and a 150 ms busy timeout protect grouped writes and responsiveness.

| Table | Purpose |
| --- | --- |
| `npc_profiles` | Stable ID, compact relationship JSON, session/hand counts, last interaction |
| `sessions` | Unique match-visit key, start/end times, participants, initial relationships, aggregate statistics, deterministic summary |
| `events` | Immutable observed fact and witness list, event key, timestamp, audited relationship deltas |
| `memories` | Owner/audience, category, summary, importance, occurrence count, timestamps/recalls, first/latest source references |

A transaction inserts a unique event, updates its derived memories/relationships and session aggregate, then commits. Replaying an already committed event is a no-op. Event facts are not edited after commit. This is a bounded observation journal, not a complete event-sourced poker replay system; the engine's privileged replay stream remains separate and unsaved.

Each NPC keeps at most 96 memories, prioritizing importance and recency. Normalized repeated quotes consolidate into one memory and do not farm relationship points. Session/hand milestones and behavioral summaries consolidate by category. Raw retention keeps the latest 512 events plus first/latest sources of retained memories. Sessions keep the latest 128 plus those referenced by retained events, including interrupted sessions. Aggregate hand/pot/conversation counters survive event pruning.

Retrieval scores topic overlap, importance and recency; returns at most three 240-character excerpts; and suppresses memories recalled in the last 120 seconds. No embedding model or vector database is used. Retrieval records selection, not proof that a model actually mentioned a memory.

Relationship values are bounded integers 0–1000: familiarity, trust, respect, competitive tension and warmth. Familiarity grows slowly with sessions/hands. Deterministic social/public poker modifiers are small and character-specific. Labels are derived views rather than a rigid linear progression. Repeated identical quotes consolidate; this deliberately simple system is not a sentiment model or an anti-abuse system.

Session summaries update transactionally as facts arrive and finalize on leaving/restarting a match or normal exit. They include hands started, biggest pot, gross awards, eliminations, meaningful conversation count, observed public high-card aggression, retained new memories and relationship deltas. An interrupted session retains its latest partial summary; pending in-memory work may be lost.

## Failure behavior and privacy

An unavailable database falls back to in-memory storage and retries opening every five seconds. Failed writes have a bounded 256-entry recovery journal; retries use idempotent event keys. Prolonged failure or a full queue can lose unsaved social data and produces an inspector warning, without stopping poker. Normal exit attempts a bounded flush. No unlimited RAM journal is retained.

Storage is local but not encrypted. Selected memories are sent to whichever provider the player configures, including a remote provider. Do not put secrets in chat. No credentials or full provider prompts are stored/logged. A private message means private from the other NPCs, not from the local user or configured provider.

See the README for `POKER_LAB_DB`, named profiles and the `social` inspection/reset example. Resets require `--confirm` and should run with the game closed. They remove derived social state; bounded historical source events and summaries remain. Use a fresh profile to start with an entirely separate history.

## Manual verification checklist

1. Run `cargo run`, start a match, play several hands and inspect all three profiles using Memories.
2. Privately tell Yuna `I enjoy strategy games`; tell Everyone a different preference. Confirm only Yuna stores the private quote and all present characters store the public one.
3. Close normally and reopen. Confirm counts, memories and relationship values persist. Ask Yuna `Remember strategy games?`; mock mode supports explicit recall, and a configured model can phrase it naturally. Allow two minutes between repeated retrieval checks.
4. Ask Ananya/Freya about the private topic. They must not receive the quote. Public comments from Yuna also exclude her private context.
5. Restart while a provider request is running. Verify no old dialogue appears, short-term history clears and persistent memories remain.
6. With a disposable DB path, make storage unavailable; play and chat using mock mode. The inspector should report temporary/unavailable saving while cards, betting and navigation continue.
7. Try reset-one, reset-relationships and a fresh profile with the game closed. Confirm unrelated characters/profiles are preserved.
8. Check physical keyboard/IME behavior and your display scaling, including true 2560×1440. Natural recollection quality with LM Studio/remote models needs manual testing; automated tests do not require either.

## Future adaptive poker (now Stage 7)

Keep adaptive opponent statistics in a separate, versioned model derived from legally witnessed gameplay facts. Feed that model through the existing strategy observation/decision interface, with deterministic seeds and simulation evaluation. Do not let social relationship scores, quoted claims or generated dialogue become authoritative betting evidence. Social memory and relationship retrieval can remain unchanged.

## Verification results

Executed October 4, 2026:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: **77 passed** (66 library, 11 application), including all 61 previous-stage tests. New tests cover migration/reopen, persistence, audience isolation, public propagation, hidden-card exclusion, all-in observation semantics, relationship bounds/gradual changes, importance, bounded retrieval/retention, consolidation, summaries, resets, mock recall, unavailable storage and locked-transaction rollback/idempotent retry.
- `cargo run` with a disposable database path: launched the Poker Lab window on AMD RX 7700 XT / Vulkan (Mesa 26.2.3); stopped with Ctrl-C after checking launch. Normal orderly exit is exercised by the smoke application.
- `ui_smoke --hd` with a temporary file database: passed. Actual compositor client size was **1920×1052**, not the requested 1920×1080.
- A second process, `ui_smoke --small --restored`, reopened the same database at **1000×820**: passed. It asserted restored session counts, Ananya's retained private preference, duplicate consolidation, and absence of that quote in Freya/Yuna's memories.
- A third `ui_smoke --small` used an intentionally invalid DB path (a file used as a parent directory): passed the complete chat/betting/hand/next-hand/profile/restart/menu flow while reporting storage unavailable and retaining temporary memory.
- All smoke runs exercised actual Bevy hit testing with synthetic pointer/keyboard input, loaded portraits, completed a hand, advanced to the next, opened/cycled/closed the memory inspector and exited normally. The profile screenshot was visually inspected. This is automated graphical verification, not a claim of physical/manual play testing.
- `cargo run --example social -- inspect` against the disposable database: passed and showed the restored per-NPC state.
- `git diff --check`: passed.

No real user database was used by these tests. No live LLM request was made. Physical input, true 2560×1440 layout and natural memory use by a live provider remain manual checks. Retention is deliberately heuristic rather than semantic summarization; profile resets retain bounded historical sources; prolonged outages/overflow may lose unsaved observations; poker-hand save/resume and adaptive strategy are out of scope.
