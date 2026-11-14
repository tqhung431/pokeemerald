use core_data::{CreatureId, GameData, MoveId, Stat, Status};

pub const MAX_MOVES: usize = 4;
const MAX_STAGE: i8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveSlot {
    pub id: MoveId,
    pub pp: u8,
}

/// One creature's battle state.
#[derive(Debug, Clone)]
pub struct Battler {
    pub creature: CreatureId,
    pub level: u8,
    pub max_hp: u16,
    pub hp: u16,
    pub moves: Vec<MoveSlot>,
    pub status: Option<Status>,
    stats: [u16; 5],
    stages: [i8; 5],
}

impl Battler {
    /// A creature at `level` knowing the newest moves from its learnset.
    pub fn new(data: &GameData, creature: CreatureId, level: u8) -> Self {
        let moves = data.creature(creature).moves_at_level(level, MAX_MOVES);
        Self::with_moves(data, creature, level, &moves)
    }

    /// A creature with an explicit move list (at most four are kept).
    pub fn with_moves(data: &GameData, creature: CreatureId, level: u8, moves: &[MoveId]) -> Self {
        let level = level.clamp(1, 100);
        let base = data.creature(creature).base_stats;
        let scaled = |b: u16| (2 * b as u32 * level as u32 / 100) as u16;
        let max_hp = scaled(base.hp) + level as u16 + 10;
        Self {
            creature,
            level,
            max_hp,
            hp: max_hp,
            moves: moves
                .iter()
                .take(MAX_MOVES)
                .map(|&id| MoveSlot {
                    id,
                    pp: data.move_def(id).pp,
                })
                .collect(),
            status: None,
            stats: Stat::ALL.map(|s| scaled(base.get(s)) + 5),
            stages: [0; 5],
        }
    }

    pub fn fainted(&self) -> bool {
        self.hp == 0
    }

    pub fn out_of_pp(&self) -> bool {
        self.moves.iter().all(|m| m.pp == 0)
    }

    pub fn stage(&self, stat: Stat) -> i8 {
        self.stages[stat.index()]
    }

    /// The stat with its stage modifier applied.
    pub fn stat(&self, stat: Stat) -> u16 {
        let raw = self.stats[stat.index()] as u32;
        let stage = self.stage(stat) as i32;
        let (num, den) = if stage >= 0 {
            (2 + stage, 2)
        } else {
            (2, 2 - stage)
        };
        (raw * num as u32 / den as u32).clamp(1, u16::MAX as u32) as u16
    }

    /// Speed used for turn order; paralysis quarters it.
    pub fn speed(&self) -> u16 {
        let speed = self.stat(Stat::Speed);
        if self.status == Some(Status::Paralysis) {
            (speed / 4).max(1)
        } else {
            speed
        }
    }

    /// Shifts a stage, clamped to -6..=6. Returns the change actually made.
    pub(crate) fn change_stage(&mut self, stat: Stat, by: i8) -> i8 {
        let slot = &mut self.stages[stat.index()];
        let new = (*slot + by).clamp(-MAX_STAGE, MAX_STAGE);
        let delta = new - *slot;
        *slot = new;
        delta
    }

    pub(crate) fn reset_stages(&mut self) {
        self.stages = [0; 5];
    }

    /// Removes up to `amount` HP. Returns the HP actually lost.
    pub(crate) fn take_damage(&mut self, amount: u32) -> u16 {
        let lost = amount.min(self.hp as u32) as u16;
        self.hp -= lost;
        lost
    }
}
