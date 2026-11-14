//! Asset manifest: maps rules ids to presentation assets.
//!
//! The rules crates never read this. The engine bridge uses it to find the
//! model, animation clips and effect template for an id, so replacing art is
//! a data change only.

use crate::{DataError, GameData};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureAssets {
    /// Engine resource path of the model (glTF).
    pub model: String,
    #[serde(default = "one")]
    pub scale: f32,
    /// Logical animation name ("idle", "hit", ...) to clip name in the model.
    pub animations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveAssets {
    /// Shared effect template, e.g. "projectile" or "contact_strike".
    pub vfx: String,
    /// Tints the template; usually the move's type key.
    pub element: String,
    /// Seconds from effect start to the moment the hit lands.
    pub hit_time: f32,
}

fn one() -> f32 {
    1.0
}

/// Animations every creature model must provide.
pub const REQUIRED_ANIMATIONS: [&str; 5] = ["idle", "physical", "special", "hit", "faint"];

/// What kind of world object this is; drives editor grouping and defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectCategory {
    Building,
    Nature,
    Prop,
    Furniture,
    Obstacle,
}

/// How the player interacts with a world object, if at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Interaction {
    /// Shows text (signs, bookshelves, TVs).
    Read,
    /// Runs a named event script (PCs, counters, statues).
    Script { script: String },
    /// Removed by an overworld skill such as "cut" or "smash"; needs a `destroy` clip.
    FieldSkill { skill: String },
    /// Moved one tile when pushed with the "strength" skill.
    Push,
    /// Picked up and removed (item balls).
    Pickup,
    /// Grows and is harvested (berry trees); needs `grow` and `harvest` clips.
    Harvest,
}

impl Interaction {
    pub fn required_animations(&self) -> &'static [&'static str] {
        match self {
            Interaction::FieldSkill { .. } => &["destroy"],
            Interaction::Harvest => &["grow", "harvest"],
            _ => &[],
        }
    }
}

/// A door on a building, in tiles relative to the footprint's north-west
/// corner. Doors on animated buildings need an `open` clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Door {
    pub x: u8,
    pub y: u8,
}

/// A placeable 3D world object: building, tree, rock, sign, furniture...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectAssets {
    pub model: String,
    pub category: ObjectCategory,
    /// Size on the movement grid in tiles: `[width, depth]`.
    pub footprint: [u8; 2],
    /// Whether the footprint blocks movement. Door tiles are always walkable.
    #[serde(default = "yes")]
    pub blocking: bool,
    #[serde(default)]
    pub doors: Vec<Door>,
    #[serde(default)]
    pub interaction: Option<Interaction>,
    /// Logical clip name to clip name in the model.
    #[serde(default)]
    pub animations: BTreeMap<String, String>,
    /// Alternate models (seasonal, damaged, palette swaps) placed by id.
    #[serde(default)]
    pub variants: BTreeMap<String, String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetManifest {
    pub creatures: BTreeMap<String, CreatureAssets>,
    pub moves: BTreeMap<String, MoveAssets>,
    #[serde(default)]
    pub objects: BTreeMap<String, ObjectAssets>,
}

impl AssetManifest {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DataError> {
        let path = path.as_ref();
        let text =
            std::fs::read_to_string(path).map_err(|e| DataError::Io(path.to_path_buf(), e))?;
        Self::from_json(&text)
    }

    pub fn from_json(json: &str) -> Result<Self, DataError> {
        serde_json::from_str(json).map_err(|e| DataError::Parse("asset manifest".into(), e))
    }

    /// Returns every problem found: ids missing from the manifest, entries for
    /// ids that do not exist, and creatures missing a required animation.
    pub fn validate(&self, data: &GameData) -> Vec<String> {
        let mut problems = Vec::new();
        for c in data.creatures() {
            match self.creatures.get(&c.key) {
                None => problems.push(format!("creature '{}' has no assets", c.key)),
                Some(a) => {
                    for anim in REQUIRED_ANIMATIONS {
                        if !a.animations.contains_key(anim) {
                            problems.push(format!("creature '{}' lacks animation '{anim}'", c.key));
                        }
                    }
                }
            }
        }
        for m in data.moves() {
            if !self.moves.contains_key(&m.key) {
                problems.push(format!("move '{}' has no assets", m.key));
            }
        }
        for key in self.creatures.keys() {
            if data.creature_id(key).is_none() {
                problems.push(format!("assets for unknown creature '{key}'"));
            }
        }
        for key in self.moves.keys() {
            if data.move_id(key).is_none() {
                problems.push(format!("assets for unknown move '{key}'"));
            }
        }
        for (key, o) in &self.objects {
            problems.extend(validate_object(key, o));
        }
        problems
    }
}

fn validate_object(key: &str, o: &ObjectAssets) -> Vec<String> {
    let mut problems = Vec::new();
    let [w, d] = o.footprint;
    if w == 0 || d == 0 {
        problems.push(format!("object '{key}' has an empty footprint"));
    }
    for door in &o.doors {
        if door.x >= w || door.y >= d {
            problems.push(format!(
                "object '{key}' door ({}, {}) is outside its footprint",
                door.x, door.y
            ));
        }
    }
    if !o.doors.is_empty() && !o.blocking {
        problems.push(format!(
            "object '{key}' has doors but does not block movement"
        ));
    }
    if let Some(i) = &o.interaction {
        for anim in i.required_animations() {
            if !o.animations.contains_key(*anim) {
                problems.push(format!(
                    "object '{key}' lacks animation '{anim}' for its interaction"
                ));
            }
        }
        if matches!(
            i,
            Interaction::Push | Interaction::Pickup | Interaction::FieldSkill { .. }
        ) && o.footprint != [1, 1]
        {
            problems.push(format!(
                "object '{key}' must be 1x1 to be pushed, picked up or removed"
            ));
        }
    }
    problems
}
