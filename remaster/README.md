# Remaster core (Rust)

Engine-independent game logic for a 3D monster-collecting RPG modelled on the
architecture of the decompiled game in the parent directory. Rules live in
plain Rust crates with no engine dependency. A Godot 4 front end (via
godot-rust) will consume their events and the asset manifest.

| Crate | Role | Replaces in the C source |
|---|---|---|
| `core-data` | Loads and validates types, moves, creatures and the asset manifest | `src/data/*`, metatile and object-event graphics tables |
| `core-battle` | Deterministic singles battle engine that emits an event stream, plus a baseline opponent AI | `battle_main.c`, `battle_util.c`, `battle_script_commands.c`, `battle_ai_*.c` |
| `core-world` | Overworld grid built from map placements and the object manifest: movement, doors, interactions, pushing and removing objects | `fieldmap.c`, `metatile_behavior.c`, `field_control_avatar.c` |
| `godot-bridge` | godot-rust extension exposing `BattleSession` and `WorldSession` to GDScript | `battle_controller_*.c` (the rules-to-presentation seam) |

All sample content in `data/` (creatures, moves, types) is original placeholder content.

## Run

```bash
cargo test
```

```bash
cargo run -p core-battle --example simulate -- 42
```

## Godot prototype

`godot/` is a Godot 4.5 project that loads the Rust bridge. Everything on
screen is a primitive-mesh placeholder with a toon shader and outline. When a
`.glb` exists at a manifest path, it is loaded instead.

- **Battle:** stage, camera cuts, a HUD with HP bars, and move buttons coloured by type. The battle events play as move effect templates: projectile, contact strike and aura.
- **Town:** a 24x18 sample map built from `data/maps/mossvale_town.json`.
  - Grid movement with a follow camera.
  - Signs, cuttable bushes, breakable rocks, pushable boulders, item pickups, berry plants and doors (interiors are not built yet).
  - Wild battles in flower patches, which return you to the town afterwards.

Build the extension, then open or run the project:

```bash
cargo build -p godot-bridge
```

```bash
godot --path godot
```

Checks (headless):

```bash
godot --headless --path godot -s res://tests/bridge_test.gd
```

```bash
godot --headless --path godot -- --smoke
```

The smoke run plays an AI-vs-AI battle, returns to town, walks, cuts the bush at
the east exit and exits non-zero on failure. Add `--screenshots=<dir>` (with a
display, e.g. `xvfb-run`) to save frames. The data directory is found at
`res://../data`, so exported builds will need it packed in a later step.

## Data

- `data/types.json`: type list and matchup chart (only non-1x entries).
- `data/moves.json`, `data/creatures.json`: rules data referenced by string id.
- `data/assets.json`: maps ids to models, animation clips, effect templates and world objects.
- `data/maps/*.json`: map layouts as object placements on a tile grid.

See `docs/ASSET_SPEC.md` for how to build models that fit the manifest.

## Battle engine

`Battle::run_turn` takes one action per side and returns `Vec<Event>`. The
presentation layer plays each event in order: camera cue, animation, effect,
HP bar. The engine already covers:

- type matchups, including dual types and immunities;
- stat stages, critical hits and random damage spread;
- burn, poison and paralysis;
- move priority and switching;
- running out of PP, which triggers a fallback attack with recoil;
- fainting and replacement.

Not yet ported: abilities, held items, weather, double battles and experience.
