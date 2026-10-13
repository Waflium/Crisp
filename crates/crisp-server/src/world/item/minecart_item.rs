use crisp_core::Vec3;
use crisp_data::block_tags;
use crisp_data::properties::RailShape;

use crate::server::level::ServerLevel;
use crate::world::entity::entity_save::apply_default_stack_config;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::{BlockHitResult, base_rail_block};
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let block_state = level.block_state(pos);
    if !block_state.is(&block_tags::RAILS) {
        return InteractionResult::Fail;
    }
    let shape = if block_state.block().is_instance_of("BaseRailBlock") { block_state.get(base_rail_block::shape_property(block_state.block())) } else { RailShape::NorthSouth };
    let offset = if base_rail_block::is_slope(shape) { 0.5 } else { 0.0 };
    let spawn_pos = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.0625 + offset, pos.z as f64 + 0.5);
    let Some(cart) = create_minecart(spawn_pos, item_stack.item.name(), &item_stack) else { return InteractionResult::Fail };
    level.add_fresh_entity(cart);
    item_stack.shrink(1);
    level.player_mut(player).update_item_in_hand(hand, &item_stack);
    InteractionResult::SUCCESS
}

pub fn create_minecart(spawn_pos: Vec3, entity_type: &str, item_stack: &ItemStack) -> Option<LevelEntity> {
    let mut entity = ServerLevel::create_entity(entity_type)?;
    let LevelEntity::Minecart(minecart) = &mut entity else { return None };
    minecart.set_initial_pos(spawn_pos.x, spawn_pos.y, spawn_pos.z);
    apply_default_stack_config(&mut entity, item_stack);
    Some(entity)
}
