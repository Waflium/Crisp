use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_3;
use crisp_data::{BlockState, items, registry, registry_id};

use super::vegetation_block::{self, Vegetation};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::{InteractionHand, InteractionResult};

pub struct SweetBerryBushBlock;

impl Vegetation for SweetBerryBushBlock {}

impl BlockBehaviour for SweetBerryBushBlock {
    fn entity_inside(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        _pos: BlockPos,
        entity: EntityId,
        _effects: &mut crate::world::entity::inside_block::InsideBlockEffects,
        _is_precise: bool,
    ) {
        if !level.is_living(entity) {
            return;
        }
        level.make_stuck_in_block(entity, crisp_core::Vec3::new(0.8f32 as f64, 0.75, 0.8f32 as f64));
        if state.get(AGE_3) != 0 {
            let movement = level.player(entity).known_movement;
            if movement.horizontal_distance_sqr() > 0.0 {
                let (xs, zs) = (movement.x.abs(), movement.z.abs());
                if xs >= 0.003f32 as f64 || zs >= 0.003f32 as f64 {
                    level.hurt_server(entity, crate::world::damagesource::DamageSource::new(crisp_data::damage_types::SWEET_BERRY_BUSH), 1.0);
                }
            }
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let age = state.get(AGE_3);
        if age < 3 && level.random.next_int_bounded(5) == 0 && level.raw_brightness(pos.above(), 0) >= 9 {
            level.set_block(pos, state.set(AGE_3, age + 1), UPDATE_CLIENTS);
        }
    }

    fn use_item_on(&self, item: ItemStack, state: BlockState, _: &mut ServerLevel, _: BlockPos, _: EntityId, _: InteractionHand, _: &BlockHitResult) -> InteractionResult {
        if state.get(AGE_3) != 3 && item.item == items::BONE_MEAL && !item.is_empty() {
            InteractionResult::Pass
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if state.get(AGE_3) <= 1 {
            return InteractionResult::Pass;
        }
        let block_entity = level.get_block_entity(pos).cloned();
        block::drop_from_block_interact_loot_table(level, "minecraft:harvest/sweet_berry_bush", pos, state, block_entity.as_ref(), None, Some(player), |level, stack| {
            level.pop_resource(pos, stack)
        });
        let pitch = 0.8 + level.random.next_float() * 0.4;
        let sound = registry_id(registry::SOUND_EVENT, "minecraft:block.sweet_berry_bush.pick_berries");
        level.play_sound(None, pos, sound, SoundSource::Blocks, 1.0, pitch);
        level.set_block(pos, state.set(AGE_3, 1), UPDATE_CLIENTS);
        InteractionResult::SUCCESS
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        state.get(AGE_3) < 3
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let new_age = 3.min(state.get(AGE_3) + 1);
        level.set_block(pos, state.set(AGE_3, new_age), UPDATE_CLIENTS);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::AGE_3;
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::InteractionHand;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn picking_ripe_berries_drops_them_and_resets_the_bush() {
        let mut level = testing::level();
        let bush = BlockPos::new(0, -60, 0);
        level.set_block(bush, blocks::SWEET_BERRY_BUSH.default_state().set(AGE_3, 3), UPDATE_ALL);
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        let hit = BlockHitResult { location: Vec3::new(0.5, -59.5, 1.0), direction: Direction::South, block_pos: bush, inside: false, world_border_hit: false };
        assert!(level.use_item_on(player, InteractionHand::MainHand, hit).consumes_action());
        assert_eq!(level.block_state(bush).get(AGE_3), 1);
        let berries: i32 = level.entities.values().filter_map(|e| e.as_item()).filter(|i| i.item.item == items::SWEET_BERRIES).map(|i| i.item.count).sum();
        assert!((2..=3).contains(&berries), "{berries} berries");
    }
}
