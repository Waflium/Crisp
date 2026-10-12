use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, block_tags, blocks, fluid_tags, registry, registry_id};

use super::BlockBehaviour;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::update_flags::UPDATE_LIMIT;
use crate::world::level::{LevelAccessor, LevelReader};

const MIN_HATCH_TICK_DELAY: i32 = 3600;
const MAX_HATCH_TICK_DELAY: i32 = 12000;

fn get_frogspawn_hatch_delay(random: &mut impl RandomSource) -> i32 {
    random.next_int_range(MIN_HATCH_TICK_DELAY, MAX_HATCH_TICK_DELAY)
}

fn may_place_on(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let fluid_state = level.fluid_state(pos);
    let fluid_above = level.fluid_state(pos.above());
    (fluid_state.is(&fluid_tags::SUPPORTS_FROGSPAWN) || level.block_state(pos).is(&block_tags::SUPPORTS_FROGSPAWN)) && fluid_above.is_empty()
}

fn get_random_tadpole_position_offset(random: &mut impl RandomSource) -> f64 {
    random.next_double().clamp(0.2f32 as f64, 0.7999999970197678)
}

pub struct FrogspawnBlock {
    block: Block,
}

impl FrogspawnBlock {
    pub fn new(block: Block) -> FrogspawnBlock {
        FrogspawnBlock { block }
    }

    fn hatch_frogspawn(&self, level: &mut ServerLevel, pos: BlockPos) {
        self.destroy_block(level, pos);
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.frogspawn.hatch"), SoundSource::Blocks, 1.0, 1.0);
        self.spawn_tadpoles(level, pos);
    }

    fn destroy_block(&self, level: &mut ServerLevel, pos: BlockPos) {
        level.destroy_block(pos, false, None, UPDATE_LIMIT);
    }

    fn spawn_tadpoles(&self, level: &mut ServerLevel, pos: BlockPos) {
        let tadpole_amount = level.random.next_int_range(2, 6);
        for _ in 1..=tadpole_amount {
            if let Some(mut tadpole) = ServerLevel::create_entity("minecraft:tadpole") {
                let x_pos = pos.x as f64 + get_random_tadpole_position_offset(&mut level.random);
                let z_pos = pos.z as f64 + get_random_tadpole_position_offset(&mut level.random);
                let y_rot = level.random.next_int_range(1, 361);
                tadpole.entity_mut().snap_to(x_pos, pos.y as f64 - 0.5, z_pos, y_rot as f32, 0.0);
                if let LevelEntity::Mob(mob) = &mut tadpole {
                    mob.persistence_required = true;
                }
                level.add_fresh_entity(tadpole);
            }
        }
    }
}

impl BlockBehaviour for FrogspawnBlock {
    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        may_place_on(level, pos.below())
    }

    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        let delay = get_frogspawn_hatch_delay(&mut level.random);
        level.schedule_tick(pos, self.block, delay);
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !self.can_survive(state, level, pos) {
            self.destroy_block(level, pos);
        } else {
            self.hatch_frogspawn(level, pos);
        }
    }

    fn entity_inside(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if matches!(level.entities.get(&entity), Some(LevelEntity::FallingBlock(_))) {
            self.destroy_block(level, pos);
        }
    }
}
