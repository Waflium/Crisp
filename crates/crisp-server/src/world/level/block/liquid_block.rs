use crisp_core::{BlockPos, Direction};
use crisp_data::properties::LEVEL;
use crisp_data::{Block, BlockState, Fluid, block_tags, blocks, fluid_tags, items};

use super::{BlockBehaviour, bubble_column_block};
use crate::server::level::ServerLevel;
use crate::server::level::events::LAVA_FIZZ;
use crate::world::item::ItemStack;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::level::material::{self, FlowingFluid};
use crate::world::level::update_flags::UPDATE_ALL;

const POSSIBLE_FLOW_DIRECTIONS: [Direction; 5] = [Direction::Down, Direction::South, Direction::North, Direction::East, Direction::West];
const BUBBLE_COLUMN_CHECK_DELAY: i32 = 20;

pub struct LiquidBlock {
    block: Block,
    fluid: &'static dyn FlowingFluid,
}

impl LiquidBlock {
    pub fn new(block: Block) -> LiquidBlock {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let source = if fields["fluid"].as_str() == Some("minecraft:lava") { Fluid::Lava } else { Fluid::Water };
        LiquidBlock { block, fluid: material::flowing(source).expect("a flowing fluid") }
    }

    fn is_lava(&self) -> bool {
        self.fluid.source().is(&fluid_tags::LAVA)
    }

    fn should_bubble_column_occupy(state: BlockState) -> bool {
        let fluid = state.fluid_state();
        fluid.is(&fluid_tags::BUBBLE_COLUMN_CAN_OCCUPY) && fluid.is_source() && fluid.amount == 8
    }

    fn try_schedule_bubble_block_column(&self, level: &mut dyn LevelAccessor, pos: BlockPos, state_below: BlockState) {
        if state_below.is(&block_tags::ENABLES_BUBBLE_COLUMN_DRAG_DOWN) || state_below.is(&block_tags::ENABLES_BUBBLE_COLUMN_PUSH_UP) {
            level.schedule_tick(pos, self.block, BUBBLE_COLUMN_CHECK_DELAY);
        }
    }

    fn should_spread_liquid(&self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        if self.is_lava() {
            let is_over_soul_soil = level.block_state(pos.below()).is_block(blocks::SOUL_SOIL);
            for direction in POSSIBLE_FLOW_DIRECTIONS {
                let neighbour_pos = pos.relative(direction.opposite());
                if level.fluid_state(neighbour_pos).is(&fluid_tags::WATER) {
                    let convert_to = if level.fluid_state(pos).is_source() { blocks::OBSIDIAN } else { blocks::COBBLESTONE };
                    level.set_block(pos, convert_to.default_state(), UPDATE_ALL);
                    level.level_event(None, LAVA_FIZZ, pos, 0);
                    return false;
                }
                if is_over_soul_soil && level.block_state(neighbour_pos).is_block(blocks::BLUE_ICE) {
                    level.set_block(pos, blocks::BASALT.default_state(), UPDATE_ALL);
                    level.level_event(None, LAVA_FIZZ, pos, 0);
                    return false;
                }
            }
        }
        true
    }

    fn schedule_fluid_tick(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if self.should_spread_liquid(level, pos) {
            let delay = self.fluid.tick_delay(level);
            level.schedule_fluid_tick(pos, state.fluid_state().fluid, delay);
        }
        if Self::should_bubble_column_occupy(state) {
            let state_below = level.block_state(pos.below());
            self.try_schedule_bubble_block_column(level, pos, state_below);
        }
    }
}

impl BlockBehaviour for LiquidBlock {
    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        material::random_tick(level, pos, state.fluid_state());
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        self.schedule_fluid_tick(level, pos, state);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if Self::should_bubble_column_occupy(state) {
            let state_below = level.block_state(pos.below());
            bubble_column_block::update_column(level, pos, state_below);
        }
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        let fluid = state.fluid_state();
        if fluid.is_source() || neighbour_state.fluid_state().is_source() {
            let delay = self.fluid.tick_delay(level);
            level.schedule_fluid_tick(pos, fluid.fluid, delay);
        }
        if direction == Direction::Down && Self::should_bubble_column_occupy(state) {
            self.try_schedule_bubble_block_column(level, pos, neighbour_state);
        }
        state
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        self.schedule_fluid_tick(level, pos, state);
    }

    fn is_bucket_pickup(&self, _state: BlockState) -> bool {
        true
    }

    fn is_liquid_block_container(&self, _state: BlockState) -> bool {
        false
    }

    fn pickup_block(&self, _player: Option<crate::world::entity::EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Option<ItemStack> {
        if state.get(LEVEL) != 0 {
            return None;
        }
        level.set_block(pos, blocks::AIR.default_state(), 11);
        let bucket = if self.is_lava() { items::LAVA_BUCKET } else { items::WATER_BUCKET };
        Some(ItemStack::new(bucket, 1))
    }

    fn pickup_sound(&self) -> Option<i32> {
        material::pickup_sound(self.fluid.source())
    }
}
