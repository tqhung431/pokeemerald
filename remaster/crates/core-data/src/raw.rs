//! On-disk JSON shapes and their conversion into [`GameData`].

use crate::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct RawTypes {
    types: Vec<String>,
    #[serde(default)]
    chart: HashMap<String, HashMap<String, f32>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMove {
    id: String,
    name: String,
    #[serde(rename = "type")]
    ty: String,
    category: Category,
    #[serde(default)]
    power: u16,
    accuracy: Option<u8>,
    pp: u8,
    #[serde(default)]
    priority: i8,
    #[serde(default)]
    effects: Vec<Effect>,
}

#[derive(Deserialize)]
struct RawLearn {
    level: u8,
    #[serde(rename = "move")]
    mv: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCreature {
    id: String,
    name: String,
    types: Vec<String>,
    base_stats: BaseStats,
    learnset: Vec<RawLearn>,
}

fn parse<'a, T: Deserialize<'a>>(what: &str, json: &'a str) -> Result<T, DataError> {
    serde_json::from_str(json).map_err(|e| DataError::Parse(what.to_string(), e))
}

fn invalid<T>(msg: String) -> Result<T, DataError> {
    Err(DataError::Invalid(msg))
}

pub(crate) fn build(types: &str, moves: &str, creatures: &str) -> Result<GameData, DataError> {
    let raw_types: RawTypes = parse("types", types)?;
    let raw_moves: Vec<RawMove> = parse("moves", moves)?;
    let raw_creatures: Vec<RawCreature> = parse("creatures", creatures)?;

    let type_names = raw_types.types;
    if type_names.is_empty() || type_names.len() > u8::MAX as usize {
        return invalid(format!("{} types (need 1..=255)", type_names.len()));
    }
    let type_id = |key: &str, ctx: &str| {
        type_names
            .iter()
            .position(|n| n == key)
            .map(|i| TypeId(i as u8))
            .ok_or_else(|| DataError::Invalid(format!("{ctx}: unknown type '{key}'")))
    };

    let n = type_names.len();
    let mut chart = vec![vec![Effectiveness::NORMAL; n]; n];
    for (atk, row) in &raw_types.chart {
        let a = type_id(atk, "type chart")?;
        for (def, &mult) in row {
            let d = type_id(def, "type chart")?;
            chart[a.0 as usize][d.0 as usize] = Effectiveness::from_f32(mult).ok_or_else(|| {
                DataError::Invalid(format!(
                    "type chart {atk}->{def}: {mult} is not 0, 0.5, 1 or 2"
                ))
            })?;
        }
    }

    let mut move_keys = HashMap::new();
    let mut move_defs = Vec::new();
    for m in raw_moves {
        let ctx = format!("move '{}'", m.id);
        if m.pp == 0 {
            return invalid(format!("{ctx}: pp must be at least 1"));
        }
        if (m.category == Category::Status) != (m.power == 0) {
            return invalid(format!(
                "{ctx}: power must be 0 for status moves and >0 otherwise"
            ));
        }
        if matches!(m.accuracy, Some(a) if a == 0 || a > 100) {
            return invalid(format!("{ctx}: accuracy must be 1..=100"));
        }
        for e in &m.effects {
            if e.chance == 0 || e.chance > 100 {
                return invalid(format!("{ctx}: effect chance must be 1..=100"));
            }
            if matches!(e.kind, EffectKind::StatStage { stages, .. } if stages == 0 || stages.abs() > 6)
            {
                return invalid(format!("{ctx}: stat stages must be -6..=6 and non-zero"));
            }
        }
        let id = MoveId(move_defs.len() as u16);
        if move_keys.insert(m.id.clone(), id).is_some() {
            return invalid(format!("{ctx}: duplicate id"));
        }
        move_defs.push(MoveDef {
            ty: type_id(&m.ty, &ctx)?,
            key: m.id,
            name: m.name,
            category: m.category,
            power: m.power,
            accuracy: m.accuracy,
            pp: m.pp,
            priority: m.priority,
            effects: m.effects,
        });
    }

    let mut creature_keys = HashMap::new();
    let mut creature_defs = Vec::new();
    for c in raw_creatures {
        let ctx = format!("creature '{}'", c.id);
        if c.types.is_empty() || c.types.len() > 2 {
            return invalid(format!("{ctx}: needs one or two types"));
        }
        if c.base_stats.hp == 0 {
            return invalid(format!("{ctx}: base hp must be at least 1"));
        }
        let types = c
            .types
            .iter()
            .map(|t| type_id(t, &ctx))
            .collect::<Result<Vec<_>, _>>()?;
        let mut learnset = Vec::new();
        for l in &c.learnset {
            let mv = *move_keys
                .get(&l.mv)
                .ok_or_else(|| DataError::Invalid(format!("{ctx}: unknown move '{}'", l.mv)))?;
            learnset.push((l.level, mv));
        }
        if !learnset.iter().any(|(level, _)| *level <= 1) {
            return invalid(format!("{ctx}: needs at least one level-1 move"));
        }
        learnset.sort_by_key(|(level, _)| *level);
        let id = CreatureId(creature_defs.len() as u16);
        if creature_keys.insert(c.id.clone(), id).is_some() {
            return invalid(format!("{ctx}: duplicate id"));
        }
        creature_defs.push(CreatureDef {
            key: c.id,
            name: c.name,
            types,
            base_stats: c.base_stats,
            learnset,
        });
    }

    Ok(GameData {
        type_names,
        chart,
        moves: move_defs,
        creatures: creature_defs,
        move_keys,
        creature_keys,
    })
}
