use crisp_core::BlockPos;
use crisp_data::{BlockState, Fluid, block_tags, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};

#[allow(unused_variables)]
pub trait Vegetation {
    fn may_place_on(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_VEGETATION)
    }
}

pub fn can_survive<V: Vegetation + ?Sized>(vegetation: &V, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let below = pos.below();
    let below_state = level.block_state(below);
    vegetation.may_place_on(below_state, level, below)
}

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    if !super::of(state).can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
}

pub fn block_can_be_replaced(state: BlockState, ctx: &BlockPlaceContext) -> bool {
    state.can_be_replaced() && (ctx.item.is_empty() || ctx.item.item != state.block().item())
}

pub fn schedule_water_tick(level: &mut ServerLevel, pos: BlockPos) {
    level.schedule_fluid_tick(pos, Fluid::Water, 5);
}

pub fn at_most_matched(level: &mut ServerLevel, min: BlockPos, max: BlockPos, block: crisp_data::Block, n: usize) -> bool {
    let mut count = 0;
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                if level.block_state(BlockPos::new(x, y, z)).is_block(block) {
                    count += 1;
                    if count > n {
                        return false;
                    }
                }
            }
        }
    }
    true
}

pub fn fields(block: crisp_data::Block) -> serde_json::Value {
    serde_json::from_str(block.info().fields).unwrap_or_default()
}

pub fn tag_field(fields: &serde_json::Value, name: &str) -> &'static [i32] {
    crisp_data::tag("minecraft:block", fields[name]["location"].as_str().unwrap_or_default())
}

pub fn block_field(fields: &serde_json::Value, name: &str) -> crisp_data::Block {
    fields[name].as_str().and_then(crisp_data::Block::by_name).unwrap_or(blocks::AIR)
}

pub fn is_in(state: BlockState, tag: &[i32]) -> bool {
    tag.contains(&(state.block().0 as i32))
}

pub struct VegetationBlock;

impl Vegetation for VegetationBlock {}

impl BlockBehaviour for VegetationBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: crisp_core::Direction, _: BlockPos, _: BlockState) -> BlockState {
        update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        can_survive(self, level, pos)
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn top_of(pos: BlockPos) -> BlockHitResult {
        let location = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5);
        BlockHitResult { location, direction: Direction::Up, block_pos: pos, inside: false, world_border_hit: false }
    }

    #[test]
    fn flowers_are_placed_only_on_vegetation_supports() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::POPPY, 2));
        let grass = BlockPos::new(0, -61, 0);
        level.use_item_on(player, InteractionHand::MainHand, top_of(grass));
        assert_eq!(level.block_state(grass.above()), blocks::POPPY.default_state());

        let stone = BlockPos::new(1, -61, 0);
        level.set_block(stone, blocks::STONE.default_state(), UPDATE_ALL);
        level.use_item_on(player, InteractionHand::MainHand, top_of(stone));
        assert!(level.block_state(stone.above()).is_air());
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::POPPY, 1)));
    }

    #[test]
    fn flowers_pop_off_when_their_support_changes() {
        let mut level = testing::level();
        let grass = BlockPos::new(0, -61, 0);
        level.set_block(grass.above(), blocks::DANDELION.default_state(), UPDATE_ALL);
        level.set_block(grass, blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.block_state(grass.above()).is_air());
        let drops: Vec<ItemStack> = level.entities.values().filter_map(|e| e.as_item()).map(|i| i.item.clone()).collect();
        assert_eq!(drops, vec![ItemStack::new(items::DANDELION, 1)]);
    }
}
