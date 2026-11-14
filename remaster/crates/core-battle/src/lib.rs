//! Headless, deterministic singles battle engine.
//!
//! The engine owns the rules only. Each call returns the [`Event`]s that
//! happened, in order; a presentation layer replays them as animations. Given
//! the same data, parties, seed and actions, the event stream is identical.

mod battler;
mod damage;
mod rng;

pub use battler::{Battler, MoveSlot};
pub use damage::{calc_damage, DamageInput};
pub use rng::Rng;

use core_data::{Category, EffectKind, GameData, Matchup, MoveId, Stat, Status, Target};
use serde::Serialize;
use std::cmp::Ordering;
use std::fmt;

/// Index of a side: 0 or 1.
pub type SideId = usize;

/// Priority given to switching, above any move.
const SWITCH_PRIORITY: i8 = 6;
/// Power of the fallback attack used when every move is out of PP.
const STRUGGLE_POWER: u16 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Use the move in this slot of the active creature.
    Move(usize),
    /// Switch to this party index.
    Switch(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Both sides choose an action; call [`Battle::run_turn`].
    Choose,
    /// The flagged sides must call [`Battle::replace`] for a fainted creature.
    Replace([bool; 2]),
    /// The battle is over. `winner` is `None` on a draw.
    Finished { winner: Option<SideId> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionError {
    EmptyParty,
    WrongPhase,
    NoSuchMove,
    NoPp,
    InvalidSwitch,
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ActionError::EmptyParty => "a party has no creature able to battle",
            ActionError::WrongPhase => "action not allowed in the current phase",
            ActionError::NoSuchMove => "no move in that slot",
            ActionError::NoPp => "that move has no PP left",
            ActionError::InvalidSwitch => "cannot switch to that party member",
        })
    }
}

impl std::error::Error for ActionError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    TurnStarted {
        turn: u32,
    },
    SwitchedOut {
        side: SideId,
    },
    SwitchedIn {
        side: SideId,
        slot: usize,
    },
    MoveUsed {
        side: SideId,
        mv: MoveId,
    },
    /// The side had no PP anywhere and used the fallback attack.
    Struggled {
        side: SideId,
    },
    Missed {
        side: SideId,
    },
    /// The target `side` is immune to the move.
    NoEffect {
        side: SideId,
    },
    /// The move did nothing (status already present, stat already capped).
    Failed {
        side: SideId,
    },
    FullyParalyzed {
        side: SideId,
    },
    Damage {
        side: SideId,
        amount: u16,
        hp_after: u16,
        crit: bool,
        matchup: Matchup,
    },
    Recoil {
        side: SideId,
        amount: u16,
        hp_after: u16,
    },
    StatusApplied {
        side: SideId,
        status: Status,
    },
    StatusDamage {
        side: SideId,
        status: Status,
        amount: u16,
        hp_after: u16,
    },
    StatStageChanged {
        side: SideId,
        stat: Stat,
        delta: i8,
    },
    Fainted {
        side: SideId,
    },
    BattleEnded {
        winner: Option<SideId>,
    },
}

#[derive(Debug, Clone)]
pub struct Side {
    pub party: Vec<Battler>,
    pub active: usize,
}

impl Side {
    pub fn active(&self) -> &Battler {
        &self.party[self.active]
    }
    fn wiped(&self) -> bool {
        self.party.iter().all(Battler::fainted)
    }
}

#[derive(Debug, Clone)]
pub struct Battle<'a> {
    data: &'a GameData,
    sides: [Side; 2],
    rng: Rng,
    turn: u32,
    phase: Phase,
}

impl<'a> Battle<'a> {
    /// Starts a battle with each party's first healthy creature active.
    pub fn new(
        data: &'a GameData,
        parties: [Vec<Battler>; 2],
        seed: u64,
    ) -> Result<Self, ActionError> {
        let mut sides = Vec::with_capacity(2);
        for party in parties {
            let active = party
                .iter()
                .position(|b| !b.fainted())
                .ok_or(ActionError::EmptyParty)?;
            sides.push(Side { party, active });
        }
        let [a, b]: [Side; 2] = sides.try_into().expect("two sides");
        Ok(Self {
            data,
            sides: [a, b],
            rng: Rng::new(seed),
            turn: 0,
            phase: Phase::Choose,
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn turn(&self) -> u32 {
        self.turn
    }
    pub fn side(&self, side: SideId) -> &Side {
        &self.sides[side]
    }
    pub fn active(&self, side: SideId) -> &Battler {
        self.sides[side].active()
    }

    /// Whether `action` would be accepted for `side` this turn.
    pub fn validate(&self, side: SideId, action: Action) -> Result<(), ActionError> {
        if self.phase != Phase::Choose {
            return Err(ActionError::WrongPhase);
        }
        match action {
            Action::Move(slot) => {
                let user = self.active(side);
                let chosen = user.moves.get(slot).ok_or(ActionError::NoSuchMove)?;
                // With no PP anywhere, any slot is accepted and becomes the fallback attack.
                if chosen.pp == 0 && !user.out_of_pp() {
                    return Err(ActionError::NoPp);
                }
                Ok(())
            }
            Action::Switch(to) => self.validate_switch_target(side, to),
        }
    }

    fn validate_switch_target(&self, side: SideId, to: usize) -> Result<(), ActionError> {
        let s = &self.sides[side];
        match s.party.get(to) {
            Some(b) if to != s.active && !b.fainted() => Ok(()),
            _ => Err(ActionError::InvalidSwitch),
        }
    }

    /// Resolves one full turn. On error nothing has changed.
    pub fn run_turn(&mut self, actions: [Action; 2]) -> Result<Vec<Event>, ActionError> {
        for (side, &action) in actions.iter().enumerate() {
            self.validate(side, action)?;
        }
        self.turn += 1;
        let mut ev = vec![Event::TurnStarted { turn: self.turn }];

        let first = self.first_to_act(actions);
        for side in [first, 1 - first] {
            // A creature that fainted earlier this turn loses its action, and
            // nothing is aimed at an empty slot.
            if self.active(side).fainted() || self.active(1 - side).fainted() {
                continue;
            }
            match actions[side] {
                Action::Switch(to) => self.switch(side, to, &mut ev),
                Action::Move(slot) => self.use_move(side, slot, &mut ev),
            }
        }

        for side in 0..2 {
            self.end_of_turn_status(side, &mut ev);
        }
        self.resolve_phase(&mut ev);
        Ok(ev)
    }

    /// Sends in a replacement for a fainted creature during [`Phase::Replace`].
    pub fn replace(&mut self, side: SideId, to: usize) -> Result<Vec<Event>, ActionError> {
        let Phase::Replace(mut need) = self.phase else {
            return Err(ActionError::WrongPhase);
        };
        if !need[side] {
            return Err(ActionError::WrongPhase);
        }
        self.validate_switch_target(side, to)?;
        self.sides[side].active = to;
        need[side] = false;
        self.phase = if need[1 - side] {
            Phase::Replace(need)
        } else {
            Phase::Choose
        };
        Ok(vec![Event::SwitchedIn { side, slot: to }])
    }

    fn first_to_act(&mut self, actions: [Action; 2]) -> SideId {
        let priority = |side: SideId| match actions[side] {
            Action::Switch(_) => SWITCH_PRIORITY,
            Action::Move(slot) => {
                let user = self.active(side);
                if user.out_of_pp() {
                    0
                } else {
                    self.data.move_def(user.moves[slot].id).priority
                }
            }
        };
        let order = priority(0)
            .cmp(&priority(1))
            .then_with(|| self.active(0).speed().cmp(&self.active(1).speed()));
        match order {
            Ordering::Greater => 0,
            Ordering::Less => 1,
            Ordering::Equal => self.rng.below(2) as usize,
        }
    }

    fn switch(&mut self, side: SideId, to: usize, ev: &mut Vec<Event>) {
        let s = &mut self.sides[side];
        s.party[s.active].reset_stages();
        s.active = to;
        ev.push(Event::SwitchedOut { side });
        ev.push(Event::SwitchedIn { side, slot: to });
    }

    fn active_mut(&mut self, side: SideId) -> &mut Battler {
        let s = &mut self.sides[side];
        &mut s.party[s.active]
    }

    fn use_move(&mut self, side: SideId, slot: usize, ev: &mut Vec<Event>) {
        let data = self.data;
        let foe = 1 - side;

        if self.active(side).status == Some(Status::Paralysis) && self.rng.chance(25) {
            ev.push(Event::FullyParalyzed { side });
            return;
        }

        if self.active(side).out_of_pp() {
            ev.push(Event::Struggled { side });
            let neutral = Matchup { num: 1, den: 1 };
            self.attack(side, STRUGGLE_POWER, Category::Physical, false, neutral, ev);
            let user = self.active_mut(side);
            let amount = user.take_damage((user.max_hp / 4).max(1) as u32);
            let hp_after = user.hp;
            ev.push(Event::Recoil {
                side,
                amount,
                hp_after,
            });
            if hp_after == 0 {
                ev.push(Event::Fainted { side });
            }
            return;
        }

        let user = self.active_mut(side);
        user.moves[slot].pp -= 1;
        let id = user.moves[slot].id;
        let mv = data.move_def(id);
        ev.push(Event::MoveUsed { side, mv: id });

        if let Some(accuracy) = mv.accuracy {
            if !self.rng.chance(accuracy) {
                ev.push(Event::Missed { side });
                return;
            }
        }

        if mv.category != Category::Status {
            let matchup = data.matchup(mv.ty, &data.creature(self.active(foe).creature).types);
            if matchup.is_immune() {
                ev.push(Event::NoEffect { side: foe });
                return;
            }
            let stab = data
                .creature(self.active(side).creature)
                .types
                .contains(&mv.ty);
            self.attack(side, mv.power, mv.category, stab, matchup, ev);
        }

        let is_status_move = mv.category == Category::Status;
        for effect in &mv.effects {
            let target = match effect.target {
                Target::User => side,
                Target::Foe => foe,
            };
            if self.active(target).fainted() {
                continue;
            }
            if effect.chance < 100 && !self.rng.chance(effect.chance) {
                continue;
            }
            let applied = match effect.kind {
                EffectKind::Status { status } => {
                    let b = self.active_mut(target);
                    let free = b.status.is_none();
                    if free {
                        b.status = Some(status);
                        ev.push(Event::StatusApplied {
                            side: target,
                            status,
                        });
                    }
                    free
                }
                EffectKind::StatStage { stat, stages } => {
                    let delta = self.active_mut(target).change_stage(stat, stages);
                    if delta != 0 {
                        ev.push(Event::StatStageChanged {
                            side: target,
                            stat,
                            delta,
                        });
                    }
                    delta != 0
                }
            };
            // A secondary effect that cannot apply is silent; a status move's is reported.
            if !applied && is_status_move {
                ev.push(Event::Failed { side });
            }
        }
    }

    /// Rolls and deals damage from `side`'s active creature to the foe's.
    fn attack(
        &mut self,
        side: SideId,
        power: u16,
        category: Category,
        stab: bool,
        matchup: Matchup,
        ev: &mut Vec<Event>,
    ) {
        let foe = 1 - side;
        let crit = self.rng.below(16) == 0;
        let roll = 85 + self.rng.below(16);
        let (attacker, defender) = (self.active(side), self.active(foe));
        let (attack, defense) = match category {
            Category::Special => (
                attacker.stat(Stat::SpAttack),
                defender.stat(Stat::SpDefense),
            ),
            _ => {
                let burned = attacker.status == Some(Status::Burn);
                let atk = attacker.stat(Stat::Attack);
                (
                    if burned { atk / 2 } else { atk },
                    defender.stat(Stat::Defense),
                )
            }
        };
        let dealt = calc_damage(DamageInput {
            level: attacker.level,
            power,
            attack,
            defense,
            crit,
            stab,
            matchup,
            roll,
        });
        let target = self.active_mut(foe);
        let amount = target.take_damage(dealt);
        let hp_after = target.hp;
        ev.push(Event::Damage {
            side: foe,
            amount,
            hp_after,
            crit,
            matchup,
        });
        if hp_after == 0 {
            ev.push(Event::Fainted { side: foe });
        }
    }

    fn end_of_turn_status(&mut self, side: SideId, ev: &mut Vec<Event>) {
        let b = self.active_mut(side);
        let Some(status @ (Status::Burn | Status::Poison)) = b.status else {
            return;
        };
        if b.fainted() {
            return;
        }
        let amount = b.take_damage((b.max_hp / 8).max(1) as u32);
        let hp_after = b.hp;
        ev.push(Event::StatusDamage {
            side,
            status,
            amount,
            hp_after,
        });
        if hp_after == 0 {
            ev.push(Event::Fainted { side });
        }
    }

    fn resolve_phase(&mut self, ev: &mut Vec<Event>) {
        let wiped = [self.sides[0].wiped(), self.sides[1].wiped()];
        self.phase = if wiped[0] || wiped[1] {
            let winner = match wiped {
                [true, true] => None,
                [true, false] => Some(1),
                _ => Some(0),
            };
            ev.push(Event::BattleEnded { winner });
            Phase::Finished { winner }
        } else {
            let need = [self.active(0).fainted(), self.active(1).fainted()];
            if need[0] || need[1] {
                Phase::Replace(need)
            } else {
                Phase::Choose
            }
        };
    }
}
