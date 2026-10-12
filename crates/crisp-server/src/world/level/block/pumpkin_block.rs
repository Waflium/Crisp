use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction, Vec3};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{BlockState, EquipmentSlot, blocks, items, registry, registry_id};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::block;
use crate::world::{InteractionHand, InteractionResult};

pub struct PumpkinBlock;

impl BlockBehaviour for PumpkinBlock {
    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        hit: &BlockHitResult,
    ) -> InteractionResult {
        if item.item != items::SHEARS {
            return InteractionResult::TryWithEmptyHand;
        }
        let clicked_direction = hit.direction;
        let direction = if clicked_direction.axis() == Axis::Y { Direction::from_y_rot(level.entity(player).y_rot as f64).opposite() } else { clicked_direction };
        let block_entity = level.get_block_entity(pos).cloned();
        block::drop_from_block_interact_loot_table(level, "minecraft:carve/pumpkin", pos, state, block_entity.as_ref(), Some(&item), Some(player), |level, pumpkin_seeds| {
            let position = Vec3::new(pos.x as f64 + 0.5 + direction.step_x() as f64 * 0.65, pos.y as f64 + 0.1, pos.z as f64 + 0.5 + direction.step_z() as f64 * 0.65);
            let mut entity = ItemEntity::new(position, pumpkin_seeds);
            let x = 0.05 * direction.step_x() as f64 + level.random.next_double() * 0.02;
            let z = 0.05 * direction.step_z() as f64 + level.random.next_double() * 0.02;
            entity.entity.set_delta_movement(Vec3::new(x, 0.05, z));
            level.add_fresh_entity(LevelEntity::Item(entity));
        });
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.pumpkin.carve"), SoundSource::Blocks, 1.0, 1.0);
        level.set_block(pos, blocks::CARVED_PUMPKIN.default_state().set(HORIZONTAL_FACING, direction), 11);
        let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
        level.hurt_and_break(player, slot, 1);
        InteractionResult::SUCCESS
    }
}
