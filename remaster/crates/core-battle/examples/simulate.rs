//! Runs a seeded battle between two sample parties and prints the event log.
//! `cargo run -p core-battle --example simulate -- [seed]`

use core_battle::{ai, Battle, Battler, Event, Phase};
use core_data::GameData;
use std::sync::Arc;

fn main() {
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);
    let data = Arc::new(
        GameData::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data")).expect("load data"),
    );
    let mk = |k: &str, lv| Battler::new(&data, data.creature_id(k).unwrap(), lv);
    let parties = [
        vec![mk("emberkit", 15), mk("cobblet", 14)],
        vec![mk("sproutail", 15), mk("zapmoth", 14)],
    ];
    let mut battle = Battle::new(data.clone(), parties, seed).unwrap();

    let name = |b: &Battle, side: usize| data.creature(b.active(side).creature).name.clone();
    loop {
        let events = match battle.phase() {
            Phase::Finished { .. } => break,
            Phase::Choose => battle
                .run_turn([ai::choose_action(&battle, 0), ai::choose_action(&battle, 1)])
                .unwrap(),
            Phase::Replace(need) => {
                let side = if need[0] { 0 } else { 1 };
                let to = battle
                    .side(side)
                    .party
                    .iter()
                    .position(|m| !m.fainted())
                    .unwrap();
                battle.replace(side, to).unwrap()
            }
        };
        for e in events {
            match e {
                Event::TurnStarted { turn } => println!("\n-- turn {turn} --"),
                Event::MoveUsed { side, mv } => {
                    println!("{} used {}", name(&battle, side), data.move_def(mv).name)
                }
                Event::Damage {
                    side,
                    amount,
                    hp_after,
                    crit,
                    matchup,
                } => {
                    let note = if matchup.is_super() {
                        " (super effective)"
                    } else if matchup.is_resisted() {
                        " (resisted)"
                    } else {
                        ""
                    };
                    println!(
                        "  {} took {amount}{}{note} -> {hp_after} HP",
                        name(&battle, side),
                        if crit { " crit" } else { "" }
                    );
                }
                other => println!("  {other:?}"),
            }
        }
    }
    println!("\n{:?}", battle.phase());
}
