# Character artwork

Stage 3 ships original generated illustrations, not placeholder silhouettes. All three are adult fictional characters created with the built-in image-generation tool. No third-party character designs or downloaded artwork were used. See [the exact prompt set](PROMPTS.md) for provenance and art direction.

| File | Identity / age | Strategy seat / stable ID |
| --- | --- | --- |
| `ananya/expressions.png` | Ananya (Anya), India, 32 | `Seat::Npc` / `mira` |
| `freya/expressions.png` | Freya, Sweden, 24 | `Seat::Jax` / `jax` |
| `yuna/expressions.png` | Yuna, Japan, 28 | `Seat::Nova` / `nova` |

Each file is a **1536x1024 RGBA PNG**, with genuine transparent pixels, in a **4-column x 2-row** grid. Each cell is **384x512**, aspect ratio 3:4. These are upper-body illustrations with consistent framing, clothing and lighting. The delivered resolution is smaller than the suggested 768x1024 per portrait, but is sufficient for the current on-screen portrait sizes. For a higher-resolution replacement, use a 3072x2048 atlas (768x1024 per cell); the renderer calculates cell rectangles from the loaded image dimensions.

Reading order, left to right, top then bottom:

| Index | Expression | Suggested individual override |
| ---: | --- | --- |
| 0 | Neutral | `neutral.png` |
| 1 | Thinking | `thinking.png` |
| 2 | Confident | `confident.png` |
| 3 | Happy | `happy.png` |
| 4 | Surprised | `surprised.png` |
| 5 | Disappointed | `disappointed.png` |
| 6 | Eliminated | `eliminated.png` |
| 7 | Reserved neutral duplicate | Unused |

## Replacing illustrations

Replace the relevant `expressions.png`, retain the grid/order and restart the app. No poker or strategy code changes are needed. Use straight-alpha PNG with transparent surroundings; keep the same head position, scale, torso crop and outfit in every cell. Do not add labels, grid lines, gutters or watermarks. The renderer crossfades complete cells, so misaligned poses will briefly ghost during transitions. Keep each silhouette inside its own cell.

For individual replacements, put 3:4 PNGs in the same character directory and register only the overrides you actually want in the presentation loader (`src/ui/characters.rs`). For example, inside `load`, after the atlas loading loop:

```rust
assets.overrides[0][Expression::Happy.index()] =
    Some(server.load("characters/ananya/happy.png"));
```

Override row 0 is Ananya, 1 Freya, 2 Yuna. These files are optional: while an override loads, or if it fails, the matching atlas expression remains visible. If the entire atlas is unavailable too, the portrait area shows the character name and “Portrait unavailable”; betting, expressions, dialogue text and match flow continue. The fallback label is deliberately identified as missing art.

Bevy's AssetServer loads the three sheets once at startup. Strong handles in PortraitAssets keep them cached across matches and menus. The `png` feature is the only added Bevy feature; the Bevy version remains pinned to 0.18.1. No image editor or generation API is required to run the game.

Generated expressions are suitable for this stage but may benefit from an illustrator's alignment/facial-expression cleanup before a commercial release. The atlas contract makes that replacement independent of gameplay.
