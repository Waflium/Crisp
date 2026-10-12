use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::speleothem_block::{self, SpeleothemBlock};
use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_SULFUR_SPIKE_LAND;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

const MAX_GROWING_LENGTH: i32 = 2;

pub struct SulfurSpikeBlock {
    block: Block,
    block_to_grow_on: BlockState,
}

impl SulfurSpikeBlock {
    pub fn new(block: Block) -> SulfurSpikeBlock {
        SulfurSpikeBlock { block, block_to_grow_on: speleothem_block::block_to_grow_on(block) }
    }
}

impl SpeleothemBlock for SulfurSpikeBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn block_to_grow_on(&self) -> BlockState {
        self.block_to_grow_on
    }

    fn get_max_growth_length(&self) -> i32 {
        MAX_GROWING_LENGTH
    }

    fn get_stalactite_landing_sound(&self) -> i32 {
        SOUND_SULFUR_SPIKE_LAND
    }
}

impl BlockBehaviour for SulfurSpikeBlock {
    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        speleothem_block::can_survive(self, state, level, pos)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        speleothem_block::update_shape(self, state, level, pos, direction)
    }

    fn get_state_for_placement(&self, _block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        speleothem_block::get_state_for_placement(self, ctx, level)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        speleothem_block::tick(self, state, level, pos);
    }

    fn on_broken_after_fall(&self, level: &mut ServerLevel, pos: BlockPos, entity: EntityId) {
        speleothem_block::on_broken_after_fall(self, level, pos, entity);
    }

    fn get_fall_damage_source(&self, entity: EntityId) -> DamageSource {
        speleothem_block::get_fall_damage_source(entity)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        speleothem_block::random_tick(self, state, level, pos);
    }
}
