use crisp_core::{Axis, BlockPos, Vec3, mth};
use crisp_data::properties::POWER;
use crisp_data::{Block, BlockState};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::LevelReader;
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_KNOWN_SHAPE};

pub struct TargetBlock {
    pub block: Block,
}

impl TargetBlock {
    fn update_redstone_output(&self, level: &mut ServerLevel, state: BlockState, hit_result: &BlockHitResult, entity: EntityId) -> i32 {
        let redstone_strength = get_redstone_strength(hit_result, hit_result.location);
        let duration = if level.as_projectile(entity).is_some_and(|projectile| projectile.abstract_arrow().is_some()) { 20 } else { 8 };
        if !level.has_scheduled_tick(hit_result.block_pos, self.block) {
            set_output_power(level, state, redstone_strength, hit_result.block_pos, duration);
        }
        redstone_strength
    }
}

fn get_redstone_strength(hit_result: &BlockHitResult, hit_location: Vec3) -> i32 {
    let hit_direction = hit_result.direction;
    let dist_x = (mth::frac(hit_location.x) - 0.5).abs();
    let dist_y = (mth::frac(hit_location.y) - 0.5).abs();
    let dist_z = (mth::frac(hit_location.z) - 0.5).abs();
    let distance = match hit_direction.axis() {
        Axis::Y => dist_x.max(dist_z),
        Axis::Z => dist_x.max(dist_y),
        Axis::X => dist_y.max(dist_z),
    };
    1.max(mth::ceil(15.0 * mth::clamp((0.5 - distance) / 0.5, 0.0, 1.0)))
}

fn set_output_power(level: &mut ServerLevel, state: BlockState, output_strength: i32, pos: BlockPos, duration: i32) {
    level.set_block_simple(pos, state.set(POWER, output_strength));
    level.schedule_tick(pos, state.block(), duration);
}

impl BlockBehaviour for TargetBlock {
    fn on_projectile_hit(&self, level: &mut ServerLevel, state: BlockState, hit_result: &BlockHitResult, projectile: EntityId) {
        let output_strength = self.update_redstone_output(level, state, hit_result, projectile);
        if let Some(player_owner) = level.projectile_get_owner(projectile).filter(|&owner| level.is_player(owner)) {
            level.award_stat(player_owner, crate::stats::stats::TARGET_HIT, 1);
            crate::advancements::triggers::criteria_triggers::TARGET_BLOCK_HIT.trigger(level, player_owner, projectile, hit_result.location, output_strength);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(POWER) != 0 {
            level.set_block_simple(pos, state.set(POWER, 0));
        }
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        state.get(POWER)
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !state.is_block(old_state.block()) && state.get(POWER) > 0 && !level.has_scheduled_tick(pos, self.block) {
            level.set_block(pos, state.set(POWER, 0), UPDATE_CLIENTS | UPDATE_KNOWN_SHAPE);
        }
    }
}
