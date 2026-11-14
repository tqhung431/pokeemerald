//! Godot bindings for the core crates.
//!
//! Exposes two GDScript classes:
//! - `BattleSession`: runs a battle and returns its events as dictionaries.
//! - `WorldSession`: loads a map layout and answers movement and interaction queries.
//!
//! The rules stay in the core crates; this layer only converts types.

mod convert;

use convert::{battler_info, event_to_dict, parse_dir};
use core_battle::{ai, Action, Battle, Battler, Phase};
use core_data::{AssetManifest, GameData, Interaction};
use core_world::{MapLayout, Step, World};
use godot::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

struct BridgeExtension;

#[gdextension]
unsafe impl ExtensionLibrary for BridgeExtension {}

fn load_data(dir: &str) -> Result<(Arc<GameData>, AssetManifest), String> {
    let dir = PathBuf::from(dir);
    let data = GameData::load_dir(&dir).map_err(|e| e.to_string())?;
    let manifest = AssetManifest::load(dir.join("assets.json")).map_err(|e| e.to_string())?;
    let problems = manifest.validate(&data);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    Ok((Arc::new(data), manifest))
}

// ---------------------------------------------------------------------------
// Battle

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct BattleSession {
    data: Option<Arc<GameData>>,
    manifest: Option<AssetManifest>,
    battle: Option<Battle>,
    error: String,
}

impl BattleSession {
    fn fail<T>(&mut self, msg: impl Into<String>, value: T) -> T {
        self.error = msg.into();
        godot_warn!("BattleSession: {}", self.error);
        value
    }

    fn parse_party(&self, party: &VarArray) -> Result<Vec<Battler>, String> {
        let data = self.data.as_ref().ok_or("data not loaded")?;
        let mut out = Vec::new();
        for entry in party.iter_shared() {
            let d: VarDictionary = entry
                .try_to()
                .map_err(|_| "party entries must be dictionaries")?;
            let key: String = d
                .get("creature")
                .ok_or("party entry lacks 'creature'")?
                .to_string();
            let level: i64 = d.get("level").and_then(|v| v.try_to().ok()).unwrap_or(5);
            let id = data
                .creature_id(&key)
                .ok_or(format!("unknown creature '{key}'"))?;
            out.push(Battler::new(data, id, level.clamp(1, 100) as u8));
        }
        Ok(out)
    }

    fn events_to_array(&self, events: &[core_battle::Event]) -> VarArray {
        let data = self.data.as_ref().expect("loaded");
        let mut arr = VarArray::new();
        for e in events {
            arr.push(&event_to_dict(data, e).to_variant());
        }
        arr
    }
}

#[godot_api]
impl BattleSession {
    /// Loads rules data and the asset manifest from an absolute directory.
    #[func]
    fn load(&mut self, data_dir: GString) -> bool {
        match load_data(&data_dir.to_string()) {
            Ok((data, manifest)) => {
                self.data = Some(data);
                self.manifest = Some(manifest);
                self.error.clear();
                true
            }
            Err(e) => self.fail(e, false),
        }
    }

    #[func]
    fn get_error(&self) -> GString {
        GString::from(self.error.as_str())
    }

    /// The asset manifest as JSON text, for the presentation layer.
    #[func]
    fn manifest_json(&self) -> GString {
        let json = self
            .manifest
            .as_ref()
            .and_then(serde_json_string)
            .unwrap_or_default();
        GString::from(json.as_str())
    }

    /// Parties are arrays of `{ "creature": key, "level": int }`.
    #[func]
    fn start(&mut self, party_a: VarArray, party_b: VarArray, seed: i64) -> bool {
        let parties = match (self.parse_party(&party_a), self.parse_party(&party_b)) {
            (Ok(a), Ok(b)) => [a, b],
            (Err(e), _) | (_, Err(e)) => return self.fail(e, false),
        };
        let data = self.data.clone().expect("checked by parse_party");
        match Battle::new(data, parties, seed as u64) {
            Ok(b) => {
                self.battle = Some(b);
                self.error.clear();
                true
            }
            Err(e) => self.fail(e.to_string(), false),
        }
    }

    /// Actions are `{ "move": slot }` or `{ "switch": party_index }`.
    /// Returns the turn's events, or an empty array and sets the error.
    #[func]
    fn run_turn(&mut self, action_a: VarDictionary, action_b: VarDictionary) -> VarArray {
        let (Some(a), Some(b)) = (parse_action(&action_a), parse_action(&action_b)) else {
            return self.fail(
                "actions must be {\"move\": n} or {\"switch\": n}",
                VarArray::new(),
            );
        };
        let Some(battle) = self.battle.as_mut() else {
            return self.fail("no battle started", VarArray::new());
        };
        match battle.run_turn([a, b]) {
            Ok(events) => self.events_to_array(&events),
            Err(e) => self.fail(e.to_string(), VarArray::new()),
        }
    }

    #[func]
    fn replace(&mut self, side: i64, party_index: i64) -> VarArray {
        let Some(battle) = self.battle.as_mut() else {
            return self.fail("no battle started", VarArray::new());
        };
        match battle.replace(side.clamp(0, 1) as usize, party_index.max(0) as usize) {
            Ok(events) => self.events_to_array(&events),
            Err(e) => self.fail(e.to_string(), VarArray::new()),
        }
    }

    /// The built-in opponent AI's choice for `side`, as an action dictionary.
    #[func]
    fn ai_action(&self, side: i64) -> VarDictionary {
        let Some(battle) = self.battle.as_ref() else {
            return VarDictionary::new();
        };
        match ai::choose_action(battle, side.clamp(0, 1) as usize) {
            Action::Move(slot) => vdict! { "move" => slot as i64 },
            Action::Switch(to) => vdict! { "switch" => to as i64 },
        }
    }

    /// `{ "kind": "choose" | "replace" | "finished", "need": [bool, bool], "winner": int }`.
    /// `winner` is -1 for a draw or an unfinished battle.
    #[func]
    fn phase(&self) -> VarDictionary {
        let Some(battle) = self.battle.as_ref() else {
            return vdict! { "kind" => "none" };
        };
        match battle.phase() {
            Phase::Choose => vdict! { "kind" => "choose", "winner" => -1 },
            Phase::Replace(need) => {
                vdict! { "kind" => "replace", "need" => &varray![need[0], need[1]], "winner" => -1 }
            }
            Phase::Finished { winner } => {
                vdict! { "kind" => "finished", "winner" => winner.map_or(-1, |w| w as i64) }
            }
        }
    }

    /// `{ "active": int, "party": [battler info...] }` for one side.
    #[func]
    fn side_info(&self, side: i64) -> VarDictionary {
        let (Some(battle), Some(data)) = (self.battle.as_ref(), self.data.as_ref()) else {
            return VarDictionary::new();
        };
        let s = battle.side(side.clamp(0, 1) as usize);
        let mut party = VarArray::new();
        for b in &s.party {
            party.push(&battler_info(data, b).to_variant());
        }
        vdict! { "active" => s.active as i64, "party" => &party }
    }
}

fn parse_action(d: &VarDictionary) -> Option<Action> {
    let get = |k: &str| {
        d.get(k)
            .and_then(|v| v.try_to::<i64>().ok())
            .filter(|n| *n >= 0)
    };
    match (get("move"), get("switch")) {
        (Some(m), None) => Some(Action::Move(m as usize)),
        (None, Some(s)) => Some(Action::Switch(s as usize)),
        _ => None,
    }
}

fn serde_json_string<T: serde::Serialize>(value: &T) -> Option<String> {
    serde_json::to_string(value).ok()
}

// ---------------------------------------------------------------------------
// World

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct WorldSession {
    world: Option<World>,
    error: String,
}

impl WorldSession {
    fn fail<T>(&mut self, msg: impl Into<String>, value: T) -> T {
        self.error = msg.into();
        godot_warn!("WorldSession: {}", self.error);
        value
    }
}

#[godot_api]
impl WorldSession {
    /// Loads `<data_dir>/maps/<map_id>.json` against the asset manifest.
    #[func]
    fn load(&mut self, data_dir: GString, map_id: GString) -> bool {
        let dir = PathBuf::from(data_dir.to_string());
        let manifest = match AssetManifest::load(dir.join("assets.json")) {
            Ok(m) => m,
            Err(e) => return self.fail(e.to_string(), false),
        };
        let layout = match MapLayout::load(dir.join("maps").join(format!("{map_id}.json"))) {
            Ok(l) => l,
            Err(e) => return self.fail(e.to_string(), false),
        };
        match World::build(layout, manifest) {
            Ok(w) => {
                self.world = Some(w);
                self.error.clear();
                true
            }
            Err(problems) => self.fail(problems.join("\n"), false),
        }
    }

    #[func]
    fn get_error(&self) -> GString {
        GString::from(self.error.as_str())
    }

    #[func]
    fn map_name(&self) -> GString {
        self.world
            .as_ref()
            .map(|w| GString::from(w.layout().name.as_str()))
            .unwrap_or_default()
    }

    #[func]
    fn size(&self) -> Vector2i {
        self.world
            .as_ref()
            .map(|w| Vector2i::new(w.layout().width, w.layout().height))
            .unwrap_or_default()
    }

    #[func]
    fn spawn(&self) -> Vector2i {
        self.world
            .as_ref()
            .map(|w| Vector2i::new(w.layout().spawn[0], w.layout().spawn[1]))
            .unwrap_or_default()
    }

    /// Every placement with its resolved model and footprint.
    #[func]
    fn placements(&self) -> VarArray {
        let mut arr = VarArray::new();
        let Some(w) = self.world.as_ref() else {
            return arr;
        };
        for (i, p) in w.layout().placements.iter().enumerate() {
            let o = &w.manifest().objects[&p.object];
            let model = p
                .variant
                .as_ref()
                .and_then(|v| o.variants.get(v))
                .unwrap_or(&o.model);
            let kind = o.interaction.as_ref().map(interaction_kind).unwrap_or("");
            let category = serde_json_string(&o.category)
                .unwrap_or_default()
                .replace('"', "");
            let mut doors = VarArray::new();
            for door in &o.doors {
                doors.push(&Vector2i::new(door.x as i32, door.y as i32).to_variant());
            }
            let d = vdict! {
                "index" => i as i64,
                "doors" => &doors,
                "object" => p.object.as_str(),
                "x" => p.x,
                "y" => p.y,
                "width" => o.footprint[0] as i64,
                "depth" => o.footprint[1] as i64,
                "model" => model.as_str(),
                "category" => category.as_str(),
                "blocking" => o.blocking,
                "interaction" => kind,
                "removed" => w.is_removed(i),
            };
            arr.push(&d.to_variant());
        }
        arr
    }

    /// `{ "result": "moved" | "blocked" | "door", "x", "y", "placement", "warp" }`.
    #[func]
    fn step(&self, x: i64, y: i64, dir: GString) -> VarDictionary {
        let (Some(w), Some(dir)) = (self.world.as_ref(), parse_dir(&dir.to_string())) else {
            return vdict! { "result" => "blocked" };
        };
        match w.step(x as i32, y as i32, dir) {
            Step::Moved { x, y } => vdict! { "result" => "moved", "x" => x, "y" => y },
            Step::Blocked => vdict! { "result" => "blocked" },
            Step::Door { placement } => {
                let warp = w.layout().placements[placement]
                    .warp
                    .clone()
                    .unwrap_or_default();
                vdict! { "result" => "door", "placement" => placement as i64, "warp" => warp.as_str() }
            }
        }
    }

    /// What is in front of the player, or an empty dictionary.
    /// `{ "placement", "object", "kind", "text", "skill", "script" }`.
    #[func]
    fn facing(&self, x: i64, y: i64, dir: GString) -> VarDictionary {
        let (Some(w), Some(dir)) = (self.world.as_ref(), parse_dir(&dir.to_string())) else {
            return VarDictionary::new();
        };
        let Some(hit) = w.facing(x as i32, y as i32, dir) else {
            return VarDictionary::new();
        };
        let mut d = vdict! {
            "placement" => hit.placement as i64,
            "object" => hit.object,
            "kind" => interaction_kind(hit.interaction),
            "text" => hit.text.unwrap_or(""),
        };
        match hit.interaction {
            Interaction::FieldSkill { skill } => d.set("skill", skill.as_str()),
            Interaction::Script { script } => d.set("script", script.as_str()),
            _ => {}
        }
        d
    }

    #[func]
    fn remove(&mut self, placement: i64) -> bool {
        match self.world.as_mut() {
            Some(w) if placement >= 0 => w.remove(placement as usize),
            _ => false,
        }
    }

    /// Pushes a boulder. Returns `{ "ok": bool, "x", "y" }`.
    #[func]
    fn push(&mut self, placement: i64, dir: GString) -> VarDictionary {
        let Some(dir) = parse_dir(&dir.to_string()) else {
            return self.fail("bad direction", vdict! { "ok" => false });
        };
        match self
            .world
            .as_mut()
            .and_then(|w| w.push(placement.max(0) as usize, dir))
        {
            Some((x, y)) => vdict! { "ok" => true, "x" => x, "y" => y },
            None => vdict! { "ok" => false },
        }
    }
}

fn interaction_kind(i: &Interaction) -> &'static str {
    match i {
        Interaction::Read => "read",
        Interaction::Script { .. } => "script",
        Interaction::FieldSkill { .. } => "field_skill",
        Interaction::Push => "push",
        Interaction::Pickup => "pickup",
        Interaction::Harvest => "harvest",
    }
}
