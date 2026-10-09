use crisp_core::BlockPos;
use crisp_core::mth;

use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::difficulty::Difficulty;
use crate::world::level::moon_phase::MOON_BRIGHTNESS_PER_PHASE;

const DIFFICULTY_TIME_GLOBAL_OFFSET: f32 = -72000.0;
const MAX_DIFFICULTY_TIME_GLOBAL: f32 = 1440000.0;
const MAX_DIFFICULTY_TIME_LOCAL: f32 = 3600000.0;

pub struct DifficultyInstance {
    pub base: Difficulty,
    pub effective_difficulty: f32,
}

impl DifficultyInstance {
    pub fn new(base: Difficulty, total_game_time: i64, local_game_time: i64, moon_brightness: f32) -> DifficultyInstance {
        DifficultyInstance { base, effective_difficulty: calculate_difficulty(base, total_game_time, local_game_time, moon_brightness) }
    }
}

fn calculate_difficulty(base: Difficulty, total_game_time: i64, local_game_time: i64, moon_brightness: f32) -> f32 {
    if base == Difficulty::Peaceful {
        return 0.0;
    }
    let is_hard = base == Difficulty::Hard;
    let mut scale = 0.75;
    let global_scale = mth::clamp_f((total_game_time as f32 + DIFFICULTY_TIME_GLOBAL_OFFSET) / MAX_DIFFICULTY_TIME_GLOBAL, 0.0, 1.0) * 0.25;
    scale += global_scale;
    let mut local_scale = 0.0;
    local_scale += mth::clamp_f(local_game_time as f32 / MAX_DIFFICULTY_TIME_LOCAL, 0.0, 1.0) * if is_hard { 1.0 } else { 0.75 };
    local_scale += mth::clamp_f(moon_brightness * 0.25, 0.0, global_scale);
    if base == Difficulty::Easy {
        local_scale *= 0.5;
    }
    scale += local_scale;
    base.id() as f32 * scale
}

impl ServerLevel {
    pub fn current_difficulty_at(&mut self, pos: BlockPos) -> DifficultyInstance {
        let (mut local_time, mut moon_brightness) = (0, 0.0);
        if let Some(chunk) = self.chunks.chunk_if_present(pos.chunk()) {
            local_time = chunk.inhabited_time;
            moon_brightness = self.moon_brightness(pos);
        }
        DifficultyInstance::new(self.difficulty, self.overworld_clock_time, local_time, moon_brightness)
    }

    pub fn moon_brightness(&mut self, pos: BlockPos) -> f32 {
        let moon_phase = self.environment_attributes().get_value_at(environment_attributes::MOON_PHASE, pos);
        MOON_BRIGHTNESS_PER_PHASE[moon_phase.index() as usize]
    }
}
