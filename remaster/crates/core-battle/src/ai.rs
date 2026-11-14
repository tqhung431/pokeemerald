//! Opponent decision making.
//!
//! A deliberately simple baseline: score each usable move by expected damage
//! (power x type matchup x same-type bonus) and use a status move only when it
//! still has an effect. Later trainer tiers can layer on top of this.

use crate::{Action, Battle, SideId};
use core_data::{Category, EffectKind, Target};

/// Picks an action for `side` during the choose phase.
pub fn choose_action(battle: &Battle, side: SideId) -> Action {
    let data = battle.data();
    let user = battle.active(side);
    let foe = battle.active(1 - side);
    let own_types = &data.creature(user.creature).types;
    let foe_types = &data.creature(foe.creature).types;

    let score = |slot: usize| -> u32 {
        let mv = data.move_def(user.moves[slot].id);
        if mv.category == Category::Status {
            // Worth a little only if at least one effect would still change something.
            let useful = mv.effects.iter().any(|e| {
                let target = if e.target == Target::User { user } else { foe };
                match e.kind {
                    EffectKind::Status { .. } => target.status.is_none(),
                    EffectKind::StatStage { stat, stages } => {
                        let now = target.stage(stat);
                        if stages > 0 {
                            now < 2
                        } else {
                            now > -2
                        }
                    }
                }
            });
            return if useful { 50 } else { 0 };
        }
        let stab = if own_types.contains(&mv.ty) { 3 } else { 2 };
        let accuracy = mv.accuracy.unwrap_or(100) as u32;
        data.matchup(mv.ty, foe_types).apply(mv.power as u32 * stab) * accuracy / 100
    };

    let best = (0..user.moves.len())
        .filter(|&i| user.moves[i].pp > 0)
        .max_by_key(|&i| score(i));
    Action::Move(best.unwrap_or(0))
}
