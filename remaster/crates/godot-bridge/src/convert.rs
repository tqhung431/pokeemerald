//! Core types to Godot dictionaries.

use core_battle::{Battler, Event};
use core_data::{GameData, Stat, Status};
use core_world::Dir;
use godot::prelude::*;

pub fn parse_dir(s: &str) -> Option<Dir> {
    match s {
        "north" => Some(Dir::North),
        "south" => Some(Dir::South),
        "east" => Some(Dir::East),
        "west" => Some(Dir::West),
        _ => None,
    }
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Burn => "burn",
        Status::Poison => "poison",
        Status::Paralysis => "paralysis",
    }
}

fn stat_name(s: Stat) -> &'static str {
    match s {
        Stat::Attack => "attack",
        Stat::Defense => "defense",
        Stat::SpAttack => "sp_attack",
        Stat::SpDefense => "sp_defense",
        Stat::Speed => "speed",
    }
}

/// One battler as `{ creature, name, types, level, hp, max_hp, status, moves }`.
pub fn battler_info(data: &GameData, b: &Battler) -> VarDictionary {
    let def = data.creature(b.creature);
    let mut types = VarArray::new();
    for t in &def.types {
        types.push(&data.type_name(*t).to_variant());
    }
    let mut moves = VarArray::new();
    for slot in &b.moves {
        let m = data.move_def(slot.id);
        let d = vdict! {
            "key" => m.key.as_str(),
            "name" => m.name.as_str(),
            "type" => data.type_name(m.ty),
            "category" => format!("{:?}", m.category).to_lowercase(),
            "power" => m.power as i64,
            "pp" => slot.pp as i64,
            "max_pp" => m.pp as i64,
        };
        moves.push(&d.to_variant());
    }
    vdict! {
        "creature" => def.key.as_str(),
        "name" => def.name.as_str(),
        "types" => &types,
        "level" => b.level as i64,
        "hp" => b.hp as i64,
        "max_hp" => b.max_hp as i64,
        "status" => b.status.map_or("", status_name),
        "moves" => &moves,
    }
}

/// One battle event as a dictionary with an `"event"` key naming its kind.
pub fn event_to_dict(data: &GameData, e: &Event) -> VarDictionary {
    let side = |s: usize| s as i64;
    match *e {
        Event::TurnStarted { turn } => vdict! { "event" => "turn_started", "turn" => turn as i64 },
        Event::SwitchedOut { side: s } => vdict! { "event" => "switched_out", "side" => side(s) },
        Event::SwitchedIn { side: s, slot } => {
            vdict! { "event" => "switched_in", "side" => side(s), "slot" => slot as i64 }
        }
        Event::MoveUsed { side: s, mv } => {
            let m = data.move_def(mv);
            vdict! {
                "event" => "move_used",
                "side" => side(s),
                "move" => m.key.as_str(),
                "name" => m.name.as_str(),
                "type" => data.type_name(m.ty),
                "category" => format!("{:?}", m.category).to_lowercase(),
            }
        }
        Event::Struggled { side: s } => vdict! { "event" => "struggled", "side" => side(s) },
        Event::Missed { side: s } => vdict! { "event" => "missed", "side" => side(s) },
        Event::NoEffect { side: s } => vdict! { "event" => "no_effect", "side" => side(s) },
        Event::Failed { side: s } => vdict! { "event" => "failed", "side" => side(s) },
        Event::FullyParalyzed { side: s } => {
            vdict! { "event" => "fully_paralyzed", "side" => side(s) }
        }
        Event::Damage {
            side: s,
            amount,
            hp_after,
            crit,
            matchup,
        } => {
            let effect = if matchup.is_super() {
                "super"
            } else if matchup.is_resisted() {
                "resisted"
            } else {
                "normal"
            };
            vdict! {
                "event" => "damage",
                "side" => side(s),
                "amount" => amount as i64,
                "hp_after" => hp_after as i64,
                "crit" => crit,
                "effectiveness" => effect,
            }
        }
        Event::Recoil {
            side: s,
            amount,
            hp_after,
        } => vdict! {
            "event" => "recoil", "side" => side(s), "amount" => amount as i64, "hp_after" => hp_after as i64,
        },
        Event::StatusApplied { side: s, status } => vdict! {
            "event" => "status_applied", "side" => side(s), "status" => status_name(status),
        },
        Event::StatusDamage {
            side: s,
            status,
            amount,
            hp_after,
        } => vdict! {
            "event" => "status_damage",
            "side" => side(s),
            "status" => status_name(status),
            "amount" => amount as i64,
            "hp_after" => hp_after as i64,
        },
        Event::StatStageChanged {
            side: s,
            stat,
            delta,
        } => vdict! {
            "event" => "stat_changed", "side" => side(s), "stat" => stat_name(stat), "delta" => delta as i64,
        },
        Event::Fainted { side: s } => vdict! { "event" => "fainted", "side" => side(s) },
        Event::BattleEnded { winner } => vdict! {
            "event" => "battle_ended", "winner" => winner.map_or(-1, |w| w as i64),
        },
    }
}
