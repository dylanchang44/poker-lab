# Stage 4 verification and design notes

Verified September 29–30, 2026:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: 61 passed (50 library, 11 application).
- `cargo run`: opened the Poker Lab window using the AMD RX 7700 XT Vulkan driver; stopped with Ctrl-C after launch verification.
- Graphical smoke checks exercised targeting, text submission, mock response, betting, hand completion, next hand, restart and menu. The small run used 1000×820. The final QHD-requested run passed at the compositor's actual 1920×1052 window size. True 2560×1440 remains unverified.
- HTTP compatibility was tested against a local test server; no live LM Studio or remote model request was made. Natural dialogue quality depends on the configured model and remains unverified here.

The conversational subsystem is plain Rust independent of Bevy. `DialogueProvider` receives an owned `TurnRequest` containing a whitelisted `PublicContext` and bounded shared dialogue. It returns text only; `parse_response` converts JSON into a limited dialogue/expression pair. The Bevy bridge never passes a strategy action to the model. The provider worker cannot access or mutate `GameSession` or `MatchEngine`.

`ConversationManager` permits one active request and one queued human-priority response, with no fan-out after a poker event. Session and hand numbers reject stale results; dropping the receiver on restart or timeout makes late worker replies harmless. It also holds global cooldown, event-ID suppression, per-NPC cadence, and a limited possibility of one NPC interjection after an everyone-addressed exchange. NPC dialogue is never a signal about hidden hand strength.

The graphical UI keeps chat entities across hand redraws. The existing `DialogueRequest` message and portrait expression timer display a validated response. Model expression `Eliminated` is forbidden; actual elimination still comes solely from poker events. Preset lines remain available when conversation is disabled or as a provider-failure fallback.

Automated tests cover config, schema rejection, identity/expression restrictions, public-context invariance under secret-card changes, real loopback HTTP request shape, missing credentials, mock routing/interjection, bounded history, cooldown/event de-duplication, timeout fallback, restart/hand stale-response rejection, and existing Stage 1–3 engine/UI behavior. No paid API or running local model is required. `ui_smoke` drives actual Bevy pointer hit testing and synthetic keyboard input to confirm chat targeting, submission, response, poker controls, complete hand, restart and menu.

Manual checks still matter for physical keyboard/IME behavior, actual LM Studio/remote model quality, provider-offline recovery, and true 2560×1440 display layout. The chat panel shows recent excerpts; full older transcript is only in current-match memory until the configured bound is reached. No persistent memory, conversation streaming, voice, or provider-side content moderation is included.
