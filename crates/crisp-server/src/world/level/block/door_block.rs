use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{DOOR_HINGE, DOUBLE_BLOCK_HALF, DoorHingeSide, DoubleBlockHalf, HORIZONTAL_FACING, OPEN, POWERED};
use crisp_data::{Block, BlockState, Fluid, SupportType, blocks};

use super::block_set_type::{BlockSetType, fields};
use super::{BlockBehaviour, BlockHitResult, block_player_will_destroy};
use crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK;
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS, UPDATE_IMMEDIATE, UPDATE_SUPPRESS_DROPS};

pub struct DoorBlock {
    set_type: BlockSetType,
}

impl DoorBlock {
    pub fn new(block: Block) -> DoorBlock {
        DoorBlock { set_type: BlockSetType::from_json(&fields(block)["type"]) }
    }

    fn hinge(ctx: &BlockPlaceContext, level: &mut ServerLevel) -> DoorHingeSide {
        let pos = ctx.clicked_pos();
        let place_direction = ctx.horizontal_direction();
        let above_pos = pos.above();
        let left_direction = place_direction.counter_clockwise();
        let left_state = level.block_state(pos.relative(left_direction));
        let left_above_state = level.block_state(above_pos.relative(left_direction));
        let right_direction = place_direction.clockwise();
        let right_state = level.block_state(pos.relative(right_direction));
        let right_above_state = level.block_state(above_pos.relative(right_direction));
        let full = |state: BlockState| state.is_collision_shape_full_block();
        let solid_block_balance = (if full(left_state) { -1 } else { 0 })
            + (if full(left_above_state) { -1 } else { 0 })
            + (if full(right_state) { 1 } else { 0 })
            + (if full(right_above_state) { 1 } else { 0 });
        let door_left = is_door(left_state) && left_state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower;
        let door_right = is_door(right_state) && right_state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower;
        if (door_left && !door_right) || solid_block_balance > 0 {
            return DoorHingeSide::Right;
        }
        if (door_right && !door_left) || solid_block_balance < 0 {
            return DoorHingeSide::Left;
        }
        let step_x = place_direction.step_x();
        let step_z = place_direction.step_z();
        let click = ctx.click_location();
        let click_x = click.x - pos.x as f64;
        let click_z = click.z - pos.z as f64;
        if (step_x >= 0 || !(click_z < 0.5)) && (step_x <= 0 || !(click_z > 0.5)) && (step_z >= 0 || !(click_x > 0.5)) && (step_z <= 0 || !(click_x < 0.5)) {
            DoorHingeSide::Left
        } else {
            DoorHingeSide::Right
        }
    }

    fn play_sound(&self, entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, open: bool) {
        let pitch = level.random.next_float() * 0.1 + 0.9;
        let sound = if open { self.set_type.door_open } else { self.set_type.door_close };
        level.play_sound(entity, pos, sound, SoundSource::Blocks, 1.0, pitch);
    }
}

fn is_door(state: BlockState) -> bool {
    state.block().is_instance_of("DoorBlock")
}

fn prevent_drop_from_bottom_part(level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) {
    if state.get(DOUBLE_BLOCK_HALF) != DoubleBlockHalf::Upper {
        return;
    }
    let bottom_pos = pos.below();
    let bottom_state = level.block_state(bottom_pos);
    if bottom_state.is_block(state.block()) && bottom_state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower {
        let replacement = if bottom_state.fluid_state().fluid == Fluid::Water { blocks::WATER.default_state() } else { blocks::AIR.default_state() };
        level.set_block(bottom_pos, replacement, UPDATE_ALL | UPDATE_SUPPRESS_DROPS);
        level.level_event(Some(player), PARTICLES_AND_SOUND_DESTROY_BLOCK, bottom_pos, bottom_state.id() as i32);
    }
}

impl BlockBehaviour for DoorBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        if pos.y >= level.chunks.max_y() {
            return None;
        }
        let above = level.block_state(pos.above());
        if !block::of(above).can_be_replaced(above, ctx) {
            return None;
        }
        let powered = level.has_neighbor_signal(pos) || level.has_neighbor_signal(pos.above());
        let facing = ctx.horizontal_direction();
        let hinge = Self::hinge(ctx, level);
        Some(
            block
                .default_state()
                .set(HORIZONTAL_FACING, facing)
                .set(DOOR_HINGE, hinge)
                .set(POWERED, powered)
                .set(OPEN, powered)
                .set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower),
        )
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        level.set_block(pos.above(), state.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper), UPDATE_ALL);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        let half = state.get(DOUBLE_BLOCK_HALF);
        if direction.axis() != Axis::Y || (half == DoubleBlockHalf::Lower) != (direction == Direction::Up) {
            if half == DoubleBlockHalf::Lower && direction == Direction::Down && !block::of(state).can_survive(state, level, pos) {
                blocks::AIR.default_state()
            } else {
                state
            }
        } else if is_door(neighbour_state) && neighbour_state.get(DOUBLE_BLOCK_HALF) != half {
            neighbour_state.set(DOUBLE_BLOCK_HALF, half)
        } else {
            blocks::AIR.default_state()
        }
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        if level.player(player).game_mode.game_type == GameType::Creative || !level.has_correct_tool_for_drops(player, state) {
            prevent_drop_from_bottom_part(level, pos, state, player);
        }
        block_player_will_destroy(level, pos, state, player)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if !self.set_type.can_open_by_hand {
            return InteractionResult::Pass;
        }
        let state = state.cycle(OPEN);
        level.set_block(pos, state, UPDATE_CLIENTS | UPDATE_IMMEDIATE);
        self.play_sound(Some(player), level, pos, state.get(OPEN));
        InteractionResult::SUCCESS
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, _moved_by_piston: bool) {
        let other_half = if state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower { Direction::Up } else { Direction::Down };
        let signal = level.has_neighbor_signal(pos) || level.has_neighbor_signal(pos.relative(other_half));
        if block != state.block() && signal != state.get(POWERED) {
            if signal != state.get(OPEN) {
                self.play_sound(None, level, pos, signal);
            }
            level.set_block(pos, state.set(POWERED, signal).set(OPEN, signal), UPDATE_CLIENTS);
        }
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let below = level.block_state(pos.below());
        if state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower { below.is_face_sturdy(Direction::Up, SupportType::Full) } else { below.is_block(state.block()) }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{DOOR_HINGE, DOUBLE_BLOCK_HALF, DoorHingeSide, DoubleBlockHalf, HORIZONTAL_FACING, OPEN, POWERED};
    use crisp_data::{blocks, items};

    use crate::server::level::ServerLevel;
    use crate::server::level::testing;
    use crate::world::InteractionResult;
    use crate::world::entity::EntityId;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn setup(item: crisp_data::Item) -> (ServerLevel, EntityId) {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(item, 64));
        (level, player)
    }

    #[test]
    fn places_both_halves_with_hinge() {
        let (mut level, player) = setup(items::OAK_DOOR);
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.8, -60.0, 0.5));
        let lower = level.block_state(pos);
        let upper = level.block_state(pos.above());
        assert_eq!(lower.get(HORIZONTAL_FACING), Direction::North);
        assert_eq!(lower.get(DOUBLE_BLOCK_HALF), DoubleBlockHalf::Lower);
        assert_eq!(upper, lower.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper));
        assert_eq!(lower.get(DOOR_HINGE), DoorHingeSide::Right);

        click(&mut level, player, pos.offset(-1, -1, 0), Direction::Up, Vec3::new(-0.2, -60.0, 0.5));
        assert_eq!(level.block_state(pos.west()).get(DOOR_HINGE), DoorHingeSide::Left);

        level.set_block(pos.offset(4, 0, 0).west(), blocks::STONE.default_state(), UPDATE_ALL);
        click(&mut level, player, pos.offset(4, -1, 0), Direction::Up, Vec3::new(4.8, -60.0, 0.5));
        assert_eq!(level.block_state(pos.offset(4, 0, 0)).get(DOOR_HINGE), DoorHingeSide::Left);
    }

    #[test]
    fn opens_by_hand_unless_iron() {
        let (mut level, player) = setup(items::OAK_DOOR);
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        level.player_mut(player).inventory.slots[36] = None;
        assert_eq!(click(&mut level, player, pos.above(), Direction::South, Vec3::new(0.5, -58.5, 1.0)), InteractionResult::SUCCESS);
        assert!(level.block_state(pos).get(OPEN));
        assert!(level.block_state(pos.above()).get(OPEN));

        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::IRON_DOOR, 1));
        let iron = BlockPos::new(2, -60, 0);
        click(&mut level, player, iron.below(), Direction::Up, Vec3::new(2.5, -60.0, 0.5));
        assert!(level.block_state(iron).is_block(blocks::IRON_DOOR));
        assert_eq!(click(&mut level, player, iron, Direction::South, Vec3::new(2.5, -59.5, 1.0)), InteractionResult::Pass);
        assert!(!level.block_state(iron).get(OPEN));
    }

    #[test]
    fn redstone_opens_both_halves() {
        let (mut level, player) = setup(items::IRON_DOOR);
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        level.set_block(pos.above().east(), blocks::LIGHTNING_ROD.default_state().set(POWERED, true), UPDATE_ALL);
        for half in [pos, pos.above()] {
            let state = level.block_state(half);
            assert!(state.get(OPEN) && state.get(POWERED), "{state:?}");
        }
        level.set_block(pos.above().east(), blocks::AIR.default_state(), UPDATE_ALL);
        assert!(!level.block_state(pos).get(OPEN));
        assert!(!level.block_state(pos.above()).get(POWERED));
    }

    #[test]
    fn breaking_one_half_removes_the_other() {
        let (mut level, player) = setup(items::OAK_DOOR);
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        level.destroy_block_by_player(player, pos.above());
        assert!(level.block_state(pos).is_air());
        assert!(level.block_state(pos.above()).is_air());
        assert_eq!(level.entities.len(), 1, "one door drops");

        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        level.set_block(pos.below(), blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(pos).is_air());
        assert!(level.block_state(pos.above()).is_air());
    }

    #[test]
    fn creative_breaking_the_upper_half_drops_nothing() {
        let (mut level, player) = setup(items::OAK_DOOR);
        level.player_mut(player).game_mode.game_type = crate::server::level::game_mode::GameType::Creative;
        let pos = BlockPos::new(0, -60, 0);
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        level.destroy_block_by_player(player, pos.above());
        assert!(level.block_state(pos).is_air());
        assert!(level.entities.is_empty());
    }
}
