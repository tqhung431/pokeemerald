# 3D asset specification

Every model in the game follows these rules, so art from any source (made in
Blender, bought, AI-generated then cleaned up) drops in without code changes.
A model is only art. Its gameplay meaning (footprint, collision, interaction)
lives in `data/assets.json` and is checked by `AssetManifest::validate`.

## Conventions for all models

| Rule | Value |
|---|---|
| Format | glTF binary (`.glb`), one asset per file |
| Units | 1 unit = 1 metre = 1 grid tile |
| Axes | +Y up, model faces -Z (glTF default) |
| Pivot | Bottom centre for characters and 1x1 props; north-west corner of the footprint for multi-tile objects |
| Transforms | Applied (scale 1, rotation 0) before export |
| Textures | Hand-painted albedo, power-of-two sizes; flat colour preferred over detail maps |
| Shading | Engine applies the shared toon shader and outline; do not bake lighting into textures |
| Names | `snake_case`, matching the manifest id (`tree_round.glb`) |
| Animation clips | Named with the logical names below; the manifest can remap if a source uses other names |

## Budgets

| Asset | Triangles | Texture |
|---|---|---|
| Creature (battle and overworld) | 3k–8k | 512–1024 |
| Player | 6k–10k | 1024 |
| Trainer / NPC | 3k–6k | 512 |
| Small building | 1k–3k | 512 (shared atlas preferred) |
| Large landmark | 5k–15k | 1024 |
| Tree / bush / rock | 200–1.5k | 256 atlas |
| Small prop / furniture | 50–800 | 256 atlas |

## Creatures

Clips required by the manifest: `idle`, `physical`, `special`, `hit`, `faint`.
Recommended extras: `walk` (overworld follow), `intro` (battle entry), `status` (stat move).
Keep each creature's bounding box roughly 0.5–3 m. The `scale` field in the
manifest corrects the rest.

## Characters (player, trainers, NPCs)

Use one shared humanoid skeleton for all humans, so one animation library
(Mixamo-compatible naming works) drives every trainer.

- Player: `idle`, `walk`, `run`, `bike`, `surf`, `fish_cast`, `fish_reel`, `throw`, `celebrate`.
- Trainers and NPCs: `idle`, `walk`, `talk`, `battle_intro`, `throw`, `defeat`.
- Face the -Z axis in a neutral pose. Size to scale: adults about 1.6–1.8 m.

## World objects

Objects snap to the movement grid. `footprint` is `[width, depth]` in tiles.
Make the visual mesh fit inside it (roofs may overhang by up to half a tile).

| Category | Examples | Notes |
|---|---|---|
| `building` | houses, heal center, shop, gym, lab | Doors are walkable tiles inside the footprint; animated doors need an `open` clip. Interiors are separate scenes reached by warps. |
| `nature` | trees, flowers, berry plants | Non-blocking decoration sets `blocking: false`. Berry plants need `grow` and `harvest`. |
| `obstacle` | cuttable bush, breakable rock, boulder | Must be 1x1. Removable ones need a `destroy` clip. |
| `prop` | signs, fences, lamps, item capsules | Signs use `interaction: read`. |
| `furniture` | PC, TV, bookshelf, counters, beds | Interiors only; use a shared furniture atlas. |

Variants (`variants` in the manifest) cover seasonal trees, palette swaps and
damaged states without new ids.

## Terrain (not objects)

Ground is built from modular kit pieces placed per tile, not one big mesh, so
each tile can carry a surface type. Prepare kit pieces for:

- Ground: grass, path, sand, deep sand, snow/ash, cave floor, wood floor, tile floor.
- Elevation: cliff faces and corners at 1 m steps, ledges (one-way jumps), stairs, ladders.
- Water: shoreline pieces, shallow water, deep water (surfable), waterfall, currents.
- Encounter cover: tall grass, long grass (both with rustle animation), seaweed.
- Special: ice, cracked floor, bridges, rails, warp pads.

These surface types replace Emerald's metatile behaviours (`MB_*` in the
original source). The overworld runtime will define them in data next to the
object manifest.

## Effects and UI (2D or particle)

- Move effects: build reusable templates (`projectile`, `contact_strike`, `aura`,
  beam, area burst, weather) tinted by element; the move manifest picks one.
- Field effects: footprints, splashes, grass rustle, shadows, emotes (`!`, `?`).
- UI: item icons, type badges, status icons, menu frames, a region map and fonts.

## Checklist before adding an asset

1. Exported as `.glb` with transforms applied and the correct pivot.
2. Clip names match, or the manifest remaps them.
3. Within budget.
4. Manifest entry added and `cargo test` passes (it runs the validator over `data/assets.json`).
5. Source and licence recorded in `ASSET_LICENSES.md`.
