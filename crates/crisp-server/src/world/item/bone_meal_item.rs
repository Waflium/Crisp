use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, SupportType, blocks};

use super::ItemStack;
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_PLANT_GROWTH;
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;
use crate::world::level::block::{self, BlockHitResult};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::BlockGetter;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let relative = pos.relative(hit.direction);
    let mut bone_meal_stack = stack;
    let original_count = bone_meal_stack.count;
    let result = if grow_crop(&mut bone_meal_stack, level, pos) {
        level.level_event(None, PARTICLES_AND_SOUND_PLANT_GROWTH, pos, 15);
        InteractionResult::SUCCESS_SERVER
    } else {
        let clicked_state = level.block_state(pos);
        let solid_block_face = clicked_state.is_face_sturdy(hit.direction, SupportType::Full);
        if solid_block_face && grow_water_plant(&mut bone_meal_stack, level, relative, Some(hit.direction)) {
            level.level_event(None, PARTICLES_AND_SOUND_PLANT_GROWTH, relative, 15);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::Pass
        }
    };
    if bone_meal_stack.count != original_count && level.player(player).game_mode.game_type != GameType::Creative {
        level.player_mut(player).update_item_in_hand(hand, &bone_meal_stack);
    }
    result
}

pub fn grow_crop(item_stack: &mut ItemStack, level: &mut ServerLevel, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    let behaviour = block::of(state);
    if !behaviour.is_valid_bonemeal_target(level, pos, state) {
        return false;
    }
    if behaviour.is_bonemeal_success(level, pos, state) {
        behaviour.perform_bonemeal(level, pos, state);
    }
    item_stack.shrink(1);
    true
}

fn random_element_of(level: &mut ServerLevel, tag: &str) -> Option<Block> {
    let contents = crisp_data::tag("minecraft:block", tag);
    if contents.is_empty() { None } else { Some(Block(contents[level.random.next_int_bounded(contents.len() as i32) as usize] as u16)) }
}

pub fn grow_water_plant(item_stack: &mut ItemStack, level: &mut ServerLevel, pos: BlockPos, clicked_face: Option<Direction>) -> bool {
    if !(level.block_state(pos).is_block(blocks::WATER) && level.fluid_state(pos).amount == 8) {
        return false;
    }
    'attempts: for j in 0..128 {
        let mut test_pos = pos;
        let mut state_to_grow = blocks::SEAGRASS.default_state();
        for _ in 0..j / 16 {
            let x = level.random.next_int_bounded(3) - 1;
            let y = (level.random.next_int_bounded(3) - 1) * level.random.next_int_bounded(3) / 2;
            let z = level.random.next_int_bounded(3) - 1;
            test_pos = test_pos.offset(x, y, z);
            if level.block_state(test_pos).is_collision_shape_full_block() {
                continue 'attempts;
            }
        }
        let test_biome = level.get_biome(test_pos);
        if crisp_data::tag("minecraft:worldgen/biome", "minecraft:produces_corals_from_bonemeal").contains(&(test_biome.0 as i32)) {
            if let Some(face) = clicked_face.filter(|face| j == 0 && face.axis().is_horizontal()) {
                state_to_grow = random_element_of(level, "minecraft:wall_corals").map_or(state_to_grow, Block::default_state);
                if state_to_grow.has(HORIZONTAL_FACING) {
                    state_to_grow = state_to_grow.set(HORIZONTAL_FACING, face);
                }
            } else if level.random.next_int_bounded(4) == 0 {
                state_to_grow = random_element_of(level, "minecraft:underwater_bonemeals").map_or(state_to_grow, Block::default_state);
            }
        }
        if state_to_grow.is(&crisp_data::block_tags::WALL_CORALS) && state_to_grow.has(HORIZONTAL_FACING) {
            let mut d = 0;
            while !block::of(state_to_grow).can_survive(state_to_grow, level, test_pos) && d < 4 {
                state_to_grow = state_to_grow.set(HORIZONTAL_FACING, Direction::PLANE_HORIZONTAL[level.random.next_int_bounded(4) as usize]);
                d += 1;
            }
        }
        if block::of(state_to_grow).can_survive(state_to_grow, level, test_pos) {
            let test_state = level.block_state(test_pos);
            if test_state.is_block(blocks::WATER) && level.fluid_state(test_pos).amount == 8 {
                level.set_block(test_pos, state_to_grow, UPDATE_ALL);
            } else if test_state.is_block(blocks::SEAGRASS)
                && block::of(test_state).is_valid_bonemeal_target(level, test_pos, test_state)
                && level.random.next_int_bounded(10) == 0
            {
                block::of(test_state).perform_bonemeal(level, test_pos, test_state);
            }
        }
    }
    item_stack.shrink(1);
    true
}
