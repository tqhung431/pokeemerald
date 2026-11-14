//! Engine-independent game data: types, moves, creatures and the asset manifest.
//!
//! Data is authored as JSON with string ids and resolved to dense indices at
//! load time, so the battle core never does string lookups.

mod manifest;
mod raw;

pub use manifest::{
    AssetManifest, CreatureAssets, Door, Interaction, MoveAssets, ObjectAssets, ObjectCategory,
    REQUIRED_ANIMATIONS,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeId(pub u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MoveId(pub u16);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreatureId(pub u16);

/// Type matchup multiplier, stored exactly as halves (0, 1, 2 or 4 halves).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effectiveness {
    halves: u8,
}

impl Effectiveness {
    pub const IMMUNE: Self = Self { halves: 0 };
    pub const RESISTED: Self = Self { halves: 1 };
    pub const NORMAL: Self = Self { halves: 2 };
    pub const SUPER: Self = Self { halves: 4 };

    fn from_f32(v: f32) -> Option<Self> {
        [Self::IMMUNE, Self::RESISTED, Self::NORMAL, Self::SUPER]
            .into_iter()
            .find(|e| e.halves as f32 / 2.0 == v)
    }
}

/// Combined multiplier against one or two defending types, as a fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matchup {
    pub num: u32,
    pub den: u32,
}

impl Matchup {
    pub fn is_immune(self) -> bool {
        self.num == 0
    }
    pub fn is_super(self) -> bool {
        self.num > self.den
    }
    pub fn is_resisted(self) -> bool {
        self.num != 0 && self.num < self.den
    }
    pub fn apply(self, damage: u32) -> u32 {
        damage * self.num / self.den
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Physical,
    Special,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Burn,
    Poison,
    Paralysis,
}

/// Stats that can be raised or lowered in battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Attack,
    Defense,
    SpAttack,
    SpDefense,
    Speed,
}

impl Stat {
    pub const ALL: [Stat; 5] = [
        Stat::Attack,
        Stat::Defense,
        Stat::SpAttack,
        Stat::SpDefense,
        Stat::Speed,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    #[serde(rename = "self")]
    User,
    Foe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectKind {
    Status { status: Status },
    StatStage { stat: Stat, stages: i8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    #[serde(flatten)]
    pub kind: EffectKind,
    /// Percent chance to apply once the move has hit.
    #[serde(default = "always")]
    pub chance: u8,
    pub target: Target,
}

fn always() -> u8 {
    100
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveDef {
    pub key: String,
    pub name: String,
    pub ty: TypeId,
    pub category: Category,
    pub power: u16,
    /// `None` means the move never misses.
    pub accuracy: Option<u8>,
    pub pp: u8,
    pub priority: i8,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseStats {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub sp_attack: u16,
    pub sp_defense: u16,
    pub speed: u16,
}

impl BaseStats {
    pub fn get(&self, stat: Stat) -> u16 {
        match stat {
            Stat::Attack => self.attack,
            Stat::Defense => self.defense,
            Stat::SpAttack => self.sp_attack,
            Stat::SpDefense => self.sp_defense,
            Stat::Speed => self.speed,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatureDef {
    pub key: String,
    pub name: String,
    pub types: Vec<TypeId>,
    pub base_stats: BaseStats,
    /// `(level, move)` pairs in ascending level order.
    pub learnset: Vec<(u8, MoveId)>,
}

impl CreatureDef {
    /// The most recent moves learned at or below `level`, up to `max`.
    pub fn moves_at_level(&self, level: u8, max: usize) -> Vec<MoveId> {
        let known: Vec<MoveId> = self
            .learnset
            .iter()
            .filter(|(l, _)| *l <= level)
            .map(|(_, m)| *m)
            .collect();
        known[known.len().saturating_sub(max)..].to_vec()
    }
}

#[derive(Debug)]
pub enum DataError {
    Io(std::path::PathBuf, std::io::Error),
    Parse(String, serde_json::Error),
    Invalid(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            DataError::Parse(what, e) => write!(f, "{what}: {e}"),
            DataError::Invalid(msg) => write!(f, "invalid data: {msg}"),
        }
    }
}

impl std::error::Error for DataError {}

/// All static rules data, validated and index-resolved.
#[derive(Debug, Clone)]
pub struct GameData {
    type_names: Vec<String>,
    chart: Vec<Vec<Effectiveness>>,
    moves: Vec<MoveDef>,
    creatures: Vec<CreatureDef>,
    move_keys: HashMap<String, MoveId>,
    creature_keys: HashMap<String, CreatureId>,
}

impl GameData {
    /// Loads `types.json`, `moves.json` and `creatures.json` from `dir`.
    pub fn load_dir(dir: impl AsRef<Path>) -> Result<Self, DataError> {
        let read = |name: &str| {
            let path = dir.as_ref().join(name);
            std::fs::read_to_string(&path).map_err(|e| DataError::Io(path, e))
        };
        Self::from_json(
            &read("types.json")?,
            &read("moves.json")?,
            &read("creatures.json")?,
        )
    }

    pub fn from_json(types: &str, moves: &str, creatures: &str) -> Result<Self, DataError> {
        raw::build(types, moves, creatures)
    }

    pub fn type_count(&self) -> usize {
        self.type_names.len()
    }
    pub fn type_name(&self, id: TypeId) -> &str {
        &self.type_names[id.0 as usize]
    }
    pub fn type_id(&self, key: &str) -> Option<TypeId> {
        self.type_names
            .iter()
            .position(|n| n == key)
            .map(|i| TypeId(i as u8))
    }

    pub fn moves(&self) -> &[MoveDef] {
        &self.moves
    }
    pub fn move_def(&self, id: MoveId) -> &MoveDef {
        &self.moves[id.0 as usize]
    }
    pub fn move_id(&self, key: &str) -> Option<MoveId> {
        self.move_keys.get(key).copied()
    }

    pub fn creatures(&self) -> &[CreatureDef] {
        &self.creatures
    }
    pub fn creature(&self, id: CreatureId) -> &CreatureDef {
        &self.creatures[id.0 as usize]
    }
    pub fn creature_id(&self, key: &str) -> Option<CreatureId> {
        self.creature_keys.get(key).copied()
    }

    pub fn effectiveness(&self, attack: TypeId, defend: TypeId) -> Effectiveness {
        self.chart[attack.0 as usize][defend.0 as usize]
    }

    /// Multiplier of an attacking type against all of a defender's types.
    pub fn matchup(&self, attack: TypeId, defender_types: &[TypeId]) -> Matchup {
        let mut m = Matchup { num: 1, den: 1 };
        for &t in defender_types {
            m.num *= self.effectiveness(attack, t).halves as u32;
            m.den *= 2;
        }
        m
    }
}
