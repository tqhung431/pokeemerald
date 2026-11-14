use core_battle::{Action, ActionError, Battle, Battler, Event, Phase};
use core_data::GameData;
use std::sync::Arc;

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

fn data() -> Arc<GameData> {
    Arc::new(GameData::load_dir(DATA).unwrap())
}

fn mon(d: &GameData, key: &str, level: u8, moves: &[&str]) -> Battler {
    let id = d.creature_id(key).unwrap();
    if moves.is_empty() {
        Battler::new(d, id, level)
    } else {
        let ids: Vec<_> = moves.iter().map(|m| d.move_id(m).unwrap()).collect();
        Battler::with_moves(d, id, level, &ids)
    }
}

/// Plays slot 0 vs slot 0 until the battle ends, replacing with the next healthy member.
fn play_out(b: &mut Battle) -> Vec<Event> {
    let mut log = Vec::new();
    for _ in 0..500 {
        match b.phase() {
            Phase::Choose => log.extend(b.run_turn([Action::Move(0), Action::Move(0)]).unwrap()),
            Phase::Replace(need) => {
                for (side, &needed) in need.iter().enumerate() {
                    if needed {
                        let to = b
                            .side(side)
                            .party
                            .iter()
                            .position(|m| !m.fainted())
                            .unwrap();
                        log.extend(b.replace(side, to).unwrap());
                    }
                }
            }
            Phase::Finished { .. } => return log,
        }
    }
    panic!("battle did not finish");
}

#[test]
fn same_seed_same_battle() {
    let d = data();
    let run = |seed| {
        let parties = [
            vec![mon(&d, "emberkit", 20, &[]), mon(&d, "cobblet", 18, &[])],
            vec![mon(&d, "pondling", 20, &[]), mon(&d, "zapmoth", 19, &[])],
        ];
        let mut b = Battle::new(d.clone(), parties, seed).unwrap();
        play_out(&mut b)
    };
    assert_eq!(run(7), run(7));
    assert_ne!(run(7), run(8));
}

#[test]
fn battle_ends_with_winner() {
    let d = data();
    let parties = [
        vec![mon(&d, "cobblet", 30, &["pebble_toss"])],
        vec![mon(&d, "zapmoth", 5, &["gust_cutter"])],
    ];
    let mut b = Battle::new(d.clone(), parties, 1).unwrap();
    let log = play_out(&mut b);
    assert_eq!(b.phase(), Phase::Finished { winner: Some(0) });
    assert!(matches!(
        log.last(),
        Some(Event::BattleEnded { winner: Some(0) })
    ));
}

#[test]
fn immunity_deals_no_damage() {
    let d = data();
    // volt vs stone is immune
    let parties = [
        vec![mon(&d, "zapmoth", 50, &["static_jolt"])],
        vec![mon(&d, "cobblet", 50, &["harden_shell"])],
    ];
    let mut b = Battle::new(d.clone(), parties, 3).unwrap();
    let ev = b.run_turn([Action::Move(0), Action::Move(0)]).unwrap();
    assert!(ev.contains(&Event::NoEffect { side: 1 }));
    assert!(!ev
        .iter()
        .any(|e| matches!(e, Event::Damage { side: 1, .. })));
    assert_eq!(b.active(1).hp, b.active(1).max_hp);
}

#[test]
fn priority_beats_speed() {
    let d = data();
    // Cobblet is far slower but uses a priority move.
    let parties = [
        vec![mon(&d, "cobblet", 50, &["quick_dash"])],
        vec![mon(&d, "zapmoth", 50, &["gust_cutter"])],
    ];
    for seed in 0..20 {
        let mut b = Battle::new(d.clone(), parties.clone(), seed).unwrap();
        let ev = b.run_turn([Action::Move(0), Action::Move(0)]).unwrap();
        let first = ev.iter().find_map(|e| match e {
            Event::MoveUsed { side, .. } => Some(*side),
            _ => None,
        });
        assert_eq!(first, Some(0));
    }
}

#[test]
fn switch_goes_first_and_resets_stages() {
    let d = data();
    let parties = [
        vec![
            mon(&d, "cobblet", 30, &["harden_shell"]),
            mon(&d, "pondling", 30, &[]),
        ],
        vec![mon(&d, "zapmoth", 30, &["quick_dash"])],
    ];
    let mut b = Battle::new(d.clone(), parties, 5).unwrap();
    b.run_turn([Action::Move(0), Action::Move(0)]).unwrap();
    assert_eq!(b.active(0).stage(core_data::Stat::Defense), 1);
    let ev = b.run_turn([Action::Switch(1), Action::Move(0)]).unwrap();
    assert_eq!(ev[1], Event::SwitchedOut { side: 0 });
    assert_eq!(b.side(0).active, 1);
    assert_eq!(b.side(0).party[0].stage(core_data::Stat::Defense), 0);
}

#[test]
fn invalid_actions_rejected_without_side_effects() {
    let d = data();
    let parties = [
        vec![mon(&d, "emberkit", 10, &["bump"])],
        vec![mon(&d, "duskit", 10, &[])],
    ];
    let mut b = Battle::new(d.clone(), parties, 9).unwrap();
    assert_eq!(
        b.run_turn([Action::Move(3), Action::Move(0)]),
        Err(ActionError::NoSuchMove)
    );
    assert_eq!(
        b.run_turn([Action::Switch(0), Action::Move(0)]),
        Err(ActionError::InvalidSwitch)
    );
    assert_eq!(b.turn(), 0);
    assert_eq!(b.replace(0, 0), Err(ActionError::WrongPhase));
}

#[test]
fn stat_stages_cap_and_fail() {
    let d = data();
    let parties = [
        vec![mon(&d, "cobblet", 50, &["harden_shell"])],
        vec![mon(&d, "pondling", 50, &["harden_shell"])],
    ];
    let mut b = Battle::new(d.clone(), parties, 2).unwrap();
    let mut log = Vec::new();
    for _ in 0..7 {
        log.extend(b.run_turn([Action::Move(0), Action::Move(0)]).unwrap());
    }
    assert_eq!(b.active(0).stage(core_data::Stat::Defense), 6);
    assert!(log.contains(&Event::Failed { side: 0 }));
}

#[test]
fn out_of_pp_struggles_and_battle_still_ends() {
    let d = data();
    let parties = [
        vec![mon(&d, "cobblet", 50, &["harden_shell"])],
        vec![mon(&d, "pondling", 50, &["harden_shell"])],
    ];
    let mut b = Battle::new(d.clone(), parties, 4).unwrap();
    let log = play_out(&mut b);
    assert!(log.iter().any(|e| matches!(e, Event::Struggled { .. })));
    assert!(log.iter().any(|e| matches!(e, Event::Recoil { .. })));
}

#[test]
fn burn_and_poison_tick() {
    let d = data();
    let parties = [
        vec![mon(&d, "sproutail", 40, &["spore_cloud"])],
        vec![mon(&d, "cobblet", 40, &["harden_shell"])],
    ];
    // Spore Cloud has 75% accuracy; find a seed where it lands on turn 1.
    for seed in 0..50 {
        let mut b = Battle::new(d.clone(), parties.clone(), seed).unwrap();
        let ev = b.run_turn([Action::Move(0), Action::Move(0)]).unwrap();
        if ev
            .iter()
            .any(|e| matches!(e, Event::StatusApplied { side: 1, .. }))
        {
            let tick = ev.iter().find_map(|e| match e {
                Event::StatusDamage {
                    side: 1, amount, ..
                } => Some(*amount),
                _ => None,
            });
            assert_eq!(tick, Some(b.active(1).max_hp / 8));
            return;
        }
    }
    panic!("status never landed");
}

#[test]
fn events_serialize_for_the_engine_bridge() {
    let ev = Event::Fainted { side: 1 };
    assert_eq!(
        serde_json::to_string(&ev).unwrap(),
        r#"{"event":"fainted","side":1}"#
    );
}

#[test]
fn ai_prefers_super_effective_and_skips_useless_status() {
    use core_battle::ai::choose_action;
    let d = data();
    // emberkit vs sproutail: Cinder (ember -> bloom, super, STAB) beats Bump.
    let parties = [
        vec![mon(&d, "emberkit", 20, &["bump", "snarl", "cinder"])],
        vec![mon(&d, "sproutail", 20, &["bump"])],
    ];
    let b = Battle::new(d.clone(), parties, 1).unwrap();
    assert_eq!(choose_action(&b, 0), Action::Move(2));

    // A status move that can no longer help is not chosen over an attack.
    let parties = [
        vec![mon(&d, "cobblet", 20, &["harden_shell", "bump"])],
        vec![mon(&d, "pondling", 20, &["bump"])],
    ];
    let mut b = Battle::new(d.clone(), parties, 1).unwrap();
    for _ in 0..2 {
        b.run_turn([Action::Move(0), Action::Move(0)]).unwrap();
    }
    assert_eq!(choose_action(&b, 0), Action::Move(1));
}
