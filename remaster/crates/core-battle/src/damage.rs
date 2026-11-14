use core_data::Matchup;

/// Everything the damage formula depends on, with stat stages and status
/// modifiers already folded into `attack` and `defense`.
#[derive(Debug, Clone, Copy)]
pub struct DamageInput {
    pub level: u8,
    pub power: u16,
    pub attack: u16,
    pub defense: u16,
    pub crit: bool,
    /// Same-type attack bonus (x1.5).
    pub stab: bool,
    pub matchup: Matchup,
    /// Random spread in percent, normally 85..=100.
    pub roll: u32,
}

/// Integer damage formula. A move that is not blocked by immunity always
/// deals at least 1.
pub fn calc_damage(i: DamageInput) -> u32 {
    if i.matchup.is_immune() {
        return 0;
    }
    let attack = i.attack.max(1) as u32;
    let defense = i.defense.max(1) as u32;
    let mut dmg = (2 * i.level as u32 / 5 + 2) * i.power as u32 * attack / defense / 50 + 2;
    if i.crit {
        dmg *= 2;
    }
    if i.stab {
        dmg = dmg * 3 / 2;
    }
    dmg = i.matchup.apply(dmg);
    dmg = dmg * i.roll / 100;
    dmg.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> DamageInput {
        DamageInput {
            level: 50,
            power: 40,
            attack: 100,
            defense: 100,
            crit: false,
            stab: false,
            matchup: Matchup { num: 1, den: 1 },
            roll: 100,
        }
    }

    #[test]
    fn formula_values() {
        // (2*50/5 + 2) * 40 * 100/100 / 50 + 2 = 19
        assert_eq!(calc_damage(base()), 19);
        assert_eq!(
            calc_damage(DamageInput {
                stab: true,
                ..base()
            }),
            28
        );
        assert_eq!(
            calc_damage(DamageInput {
                crit: true,
                ..base()
            }),
            38
        );
        assert_eq!(calc_damage(DamageInput { roll: 85, ..base() }), 16);
        assert_eq!(
            calc_damage(DamageInput {
                attack: 200,
                ..base()
            }),
            37
        );
    }

    #[test]
    fn matchups_scale_and_floor() {
        let m = |num, den| Matchup { num, den };
        assert_eq!(
            calc_damage(DamageInput {
                matchup: m(4, 2),
                ..base()
            }),
            38
        );
        assert_eq!(
            calc_damage(DamageInput {
                matchup: m(16, 4),
                ..base()
            }),
            76
        );
        assert_eq!(
            calc_damage(DamageInput {
                matchup: m(1, 2),
                ..base()
            }),
            9
        );
        assert_eq!(
            calc_damage(DamageInput {
                matchup: m(0, 2),
                ..base()
            }),
            0
        );
        // Tiny hits still deal 1.
        let weak = DamageInput {
            level: 1,
            power: 1,
            attack: 1,
            defense: 500,
            matchup: m(1, 4),
            ..base()
        };
        assert_eq!(calc_damage(weak), 1);
    }
}
