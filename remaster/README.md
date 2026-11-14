# Remaster core (Rust)

Engine-independent game logic for a 3D monster-collecting RPG modelled on the
architecture of the decompiled game in the parent directory. Rules live in
plain Rust crates with no engine dependency. A Godot 4 front end (via
godot-rust) will consume their events and the asset manifest.

| Crate | Role | Replaces in the C source |
|---|---|---|
| `core-data` | Loads and validates types, moves, creatures and the asset manifest | `src/data/*`, metatile and object-event graphics tables |
| `core-battle` | Deterministic singles battle engine that emits an event stream | `battle_main.c`, `battle_util.c`, `battle_script_commands.c` |

All sample content in `data/` (creatures, moves, types) is original placeholder content.

## Run

```bash
cargo test
```

```bash
cargo run -p core-battle --example simulate -- 42
```

## Data

- `data/types.json`: type list and matchup chart (only non-1x entries).
- `data/moves.json`, `data/creatures.json`: rules data referenced by string id.
- `data/assets.json`: maps ids to models, animation clips, effect templates and world objects.

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

Not yet ported: abilities, held items, weather, double battles, experience and the opponent AI.
