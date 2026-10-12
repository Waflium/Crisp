use crisp_core::{AABB, BlockPos, Direction};
use crisp_core::random::RandomSource;
use crisp_data::properties::LEVEL_CAULDRON;
use crisp_data::{Block, BlockState, Fluid, blocks};

use super::{BlockBehaviour, BlockHitResult, abstract_cauldron_block, vegetation_block::fields};
use crate::core::cauldron::cauldron_interactions::{Dispatcher, lower_fill_level};
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::{InsideBlockEffectType, InsideBlockEffects};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};
use crate::server::level::ServerLevel;
use crate::server::level::events::{SOUND_DRIP_LAVA_INTO_CAULDRON, SOUND_DRIP_WATER_INTO_CAULDRON};
use crate::world::level::biome::Precipitation;
use crate::world::level::update_flags::UPDATE_ALL;

fn should_handle_precipitation(level: &mut ServerLevel, precipitation: Precipitation) -> bool {
    match precipitation {
        Precipitation::Rain => level.random.next_float() < 0.05,
        Precipitation::Snow => level.random.next_float() < 0.1,
        Precipitation::None => false,
    }
}

pub struct CauldronBlock;

impl BlockBehaviour for CauldronBlock {
    fn use_item_on(&self, item: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, _hit: &BlockHitResult) -> InteractionResult {
        use_item_on_dispatcher(Dispatcher::Empty, item, state, level, pos, player, hand)
    }

    fn handle_precipitation(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, precipitation: Precipitation) {
        if should_handle_precipitation(level, precipitation) {
            match precipitation {
                Precipitation::Rain => drop(level.set_block(pos, blocks::WATER_CAULDRON.default_state(), UPDATE_ALL)),
                Precipitation::Snow => drop(level.set_block(pos, blocks::POWDER_SNOW_CAULDRON.default_state(), UPDATE_ALL)),
                Precipitation::None => {}
            }
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        abstract_cauldron_block::tick(self, state, level, pos);
    }

    fn can_receive_stalactite_drip(&self, _fluid: Fluid) -> bool {
        true
    }

    fn receive_stalactite_drip(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, fluid: Fluid) {
        if fluid == Fluid::Water {
            let new_state = blocks::WATER_CAULDRON.default_state();
            level.set_block(pos, new_state, UPDATE_ALL);
            level.level_event(None, SOUND_DRIP_WATER_INTO_CAULDRON, pos, 0);
        } else if fluid == Fluid::Lava {
            let new_state = blocks::LAVA_CAULDRON.default_state();
            level.set_block(pos, new_state, UPDATE_ALL);
            level.level_event(None, SOUND_DRIP_LAVA_INTO_CAULDRON, pos, 0);
        }
    }
}

pub struct LayeredCauldronBlock {
    precipitation_type: Precipitation,
}

impl LayeredCauldronBlock {
    pub fn new(block: Block) -> LayeredCauldronBlock {
        let precipitation_type = match fields(block)["precipitationType"].as_str() {
            Some("rain") => Precipitation::Rain,
            Some("snow") => Precipitation::Snow,
            _ => Precipitation::None,
        };
        LayeredCauldronBlock { precipitation_type }
    }
}

impl LayeredCauldronBlock {
    fn handle_entity_on_fire_inside(precipitation_type: Precipitation, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if precipitation_type == Precipitation::Snow {
            lower_fill_level(blocks::WATER_CAULDRON.default_state().set(LEVEL_CAULDRON, state.get(LEVEL_CAULDRON)), level, pos);
        } else {
            lower_fill_level(state, level, pos);
        }
    }
}

impl BlockBehaviour for LayeredCauldronBlock {
    fn use_item_on(&self, item: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, _hit: &BlockHitResult) -> InteractionResult {
        let dispatcher = if self.precipitation_type == Precipitation::Snow { Dispatcher::PowderSnow } else { Dispatcher::Water };
        use_item_on_dispatcher(dispatcher, item, state, level, pos, player, hand)
    }

    fn entity_inside_collision_shape(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _entity: EntityId) -> Option<Vec<AABB>> {
        Some(filled_shape(state, 6.0 + state.get(LEVEL_CAULDRON) as f64 * 3.0))
    }

    fn entity_inside(&self, state: BlockState, _level: &mut ServerLevel, pos: BlockPos, _entity: EntityId, effects: &mut InsideBlockEffects, _is_precise: bool) {
        let precipitation_type = self.precipitation_type;
        effects.run_before(
            InsideBlockEffectType::Extinguish,
            Box::new(move |level, e| {
                let may_interact = !level.is_player(e) || level.may_interact(e, pos);
                if level.is_on_fire(e) && may_interact {
                    LayeredCauldronBlock::handle_entity_on_fire_inside(precipitation_type, state, level, pos);
                }
            }),
        );
        effects.apply(InsideBlockEffectType::Extinguish);
    }

    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        state.get(LEVEL_CAULDRON)
    }

    fn handle_precipitation(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, precipitation: Precipitation) {
        if should_handle_precipitation(level, precipitation) && state.get(LEVEL_CAULDRON) != 3 && precipitation == self.precipitation_type {
            level.set_block(pos, state.cycle(LEVEL_CAULDRON), UPDATE_ALL);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        abstract_cauldron_block::tick(self, state, level, pos);
    }

    fn can_receive_stalactite_drip(&self, fluid: Fluid) -> bool {
        fluid == Fluid::Water && self.precipitation_type == Precipitation::Rain
    }

    fn receive_stalactite_drip(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _fluid: Fluid) {
        if state.get(LEVEL_CAULDRON) != 3 {
            let new_state = state.set(LEVEL_CAULDRON, state.get(LEVEL_CAULDRON) + 1);
            level.set_block(pos, new_state, UPDATE_ALL);
            level.level_event(None, SOUND_DRIP_WATER_INTO_CAULDRON, pos, 0);
        }
    }
}

fn use_item_on_dispatcher(dispatcher: Dispatcher, item: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand) -> InteractionResult {
    dispatcher.interact(state, level, pos, player, hand, item)
}

fn filled_shape(state: BlockState, content_height: f64) -> Vec<AABB> {
    let mut boxes = state.voxel_shape().to_aabbs();
    boxes.push(AABB::new(2.0 / 16.0, 4.0 / 16.0, 2.0 / 16.0, 14.0 / 16.0, content_height / 16.0, 14.0 / 16.0));
    boxes
}

pub struct LavaCauldronBlock;

impl BlockBehaviour for LavaCauldronBlock {
    fn use_item_on(&self, item: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, _hit: &BlockHitResult) -> InteractionResult {
        use_item_on_dispatcher(Dispatcher::Lava, item, state, level, pos, player, hand)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        abstract_cauldron_block::tick(self, state, level, pos);
    }

    fn entity_inside_collision_shape(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _entity: EntityId) -> Option<Vec<AABB>> {
        Some(filled_shape(state, 15.0))
    }

    fn entity_inside(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _entity: EntityId, effects: &mut InsideBlockEffects, _is_precise: bool) {
        effects.apply(InsideBlockEffectType::ClearFreeze);
        effects.apply(InsideBlockEffectType::LavaIgnite);
        effects.run_after(InsideBlockEffectType::LavaIgnite, Box::new(|level, id| level.lava_hurt(id)));
    }

    fn get_analog_output_signal(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        3
    }
}
