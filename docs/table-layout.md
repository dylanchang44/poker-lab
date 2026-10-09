# Centered table and readable Table Talk

The felt, community cards and human seat now share the canvas centerline. The
felt spans 94% of the canvas instead of reserving a left-hand strip for chat.
The canvas scales to the window and can grow to 2,000 UI units wide.

Below the table, Table Talk occupies 60% of the width; compact betting controls
occupy a separate right-hand panel. Both panels are shorter than the previous
chat sidebar. The portrait dialogue has its own band above the human cards.

The transcript takes the remaining height inside the chat panel and preserves
its natural text height using a vertical flex layout. Previously the default
row layout could constrain the transcript node without containing its glyphs,
so the calculated scroll range did not reliably expose the final line.
Complete validated replies now wrap, and latest-follow scrolls to the bottom.
Mouse-wheel scrolling or Older pauses following; Newer or Latest returns toward
recent messages. Scrolling back to the bottom resumes following automatically.
The full 240-character input wraps across the wider input field.

`cargo run --example ui_smoke -- --small --long-chat` checks:

- Complete 360-character NPC replies and 240-character input.
- Rendered transcript glyph bounds and latest-line visibility, not just stored text.
- Private/public labels, scripted fallback badges, and multiple wrapped messages.
- Older/Newer/Latest buttons and mouse-wheel scrolling.
- Centered felt and human seat, on-screen panels, separation from human cards,
  and action buttons fitting inside their panel.
- Existing betting, hand progression, debug panels, restart and menu navigation.

Use `--hd` or `--qhd` instead of `--small` for larger-window checks. The test logs
the actual logical and physical window size, since desktop window constraints
and display scaling can change the requested size. Smoke tests use mock dialogue
and do not touch the player's saved database by default. They do not test model
inference or change NPC poker strategies.

## Verification

The revised layout passed `cargo fmt --check`, strict all-target Clippy, all 113
unit/integration tests, and `cargo build`. Rendered long-chat smoke checks passed
at 1000×820 and 1920×1052, with screenshots inspected for complete reply/input
text and separate controls. The desktop constrained the requested 1920×1080
and 2560×1440 windows to 1920×1052; exact 1080p and QHD coverage is not claimed.
After resuming on a desktop at 125% scaling, both complete smoke runs also passed
at 1000×820 logical / 1250×1025 physical and 1920×1080 logical / 2400×1350 physical.
Thus 1080p logical layout is now covered; exact QHD remains unverified.
