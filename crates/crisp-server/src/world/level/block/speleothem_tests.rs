use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::{LEVEL_CAULDRON, SpeleothemThickness, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, blocks, items};

use super::BlockHitResult;
use super::speleothem_block::{THICKNESS, TIP_DIRECTION};
use crate::server::level::testing;
use crate::world::InteractionHand;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader, LevelWriter, ScheduledTickAccess};
use crate::world::ticks::TickPriority;

struct Column {
    states: [BlockState; 3],
    scheduled: bool,
    block_tick_delay: i32,
    fluid_tick_delay: i32,
    random: LegacyRandomSource,
}

impl Column {
    fn new(below: BlockState, centre: BlockState, above: BlockState, scheduled: bool) -> Column {
        Column { states: [below, centre, above], scheduled, block_tick_delay: 0, fluid_tick_delay: 0, random: LegacyRandomSource::new(0) }
    }
}

impl BlockGetter for Column {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        if pos.x == 0 && pos.z == 0 && (-1..=1).contains(&pos.y) { self.states[(pos.y + 1) as usize] } else { blocks::AIR.default_state() }
    }

    fn get_min_y(&self) -> i32 {
        -64
    }
}

impl LevelReader for Column {
    fn raw_brightness(&mut self, _pos: BlockPos, _darkening: i32) -> i32 {
        unreachable!()
    }

    fn get_brightness(&mut self, _layer: LightLayer, _pos: BlockPos) -> i32 {
        unreachable!()
    }

    fn get_sea_level(&self) -> i32 {
        63
    }

    fn is_inside_build_height(&self, y: i32) -> bool {
        (-64..320).contains(&y)
    }

    fn is_fast_lava(&self) -> bool {
        false
    }

    fn get_control_input_signal(&mut self, _pos: BlockPos, _direction: Direction, _only_diodes: bool) -> i32 {
        unreachable!()
    }

    fn get_block_entity(&mut self, _pos: BlockPos) -> Option<&mut BlockEntity> {
        None
    }
}

impl ScheduledTickAccess for Column {
    fn schedule_tick_with_priority(&mut self, _pos: BlockPos, _block: Block, delay: i32, _priority: TickPriority) {
        self.block_tick_delay = delay;
    }

    fn schedule_fluid_tick(&mut self, _pos: BlockPos, _fluid: Fluid, delay: i32) {
        self.fluid_tick_delay = delay;
    }

    fn has_scheduled_tick(&self, _pos: BlockPos, _block: Block) -> bool {
        self.scheduled
    }
}

impl LevelWriter for Column {
    fn set_block(&mut self, _pos: BlockPos, _state: BlockState, _flags: i32) -> bool {
        unreachable!()
    }
}

impl LevelAccessor for Column {
    fn get_random(&mut self) -> &mut dyn RandomSource {
        &mut self.random
    }

    fn mark_pos_for_post_processing(&mut self, _pos: BlockPos) {}
}

fn speleothem(block: Block, direction: Direction, thickness: SpeleothemThickness) -> BlockState {
    block.default_state().set(TIP_DIRECTION, direction).set(THICKNESS, thickness)
}

fn speleothems() -> Vec<BlockState> {
    let mut out = Vec::new();
    for block in [blocks::POINTED_DRIPSTONE, blocks::SULFUR_SPIKE] {
        for direction in [Direction::Up, Direction::Down] {
            for thickness in SpeleothemThickness::VALUES {
                out.push(speleothem(block, direction, thickness));
            }
        }
    }
    out
}

fn update_shape(column: &mut Column, direction: Direction) -> BlockState {
    let centre = column.states[1];
    let neighbour_pos = BlockPos::ZERO.relative(direction);
    let neighbour_state = column.block_state(neighbour_pos);
    super::of(centre).update_shape(centre, column, BlockPos::ZERO, direction, neighbour_pos, neighbour_state)
}

#[test]
fn stacked_sulfur_spike_tips_become_a_frustum() {
    let tip = speleothem(blocks::SULFUR_SPIKE, Direction::Up, SpeleothemThickness::Tip);
    let mut column = Column::new(blocks::STONE.default_state(), tip, tip, false);
    assert_eq!(update_shape(&mut column, Direction::Up).get(THICKNESS), SpeleothemThickness::Frustum);
    assert_eq!(update_shape(&mut column, Direction::Down).get(THICKNESS), SpeleothemThickness::Frustum);
    assert_eq!(column.block_tick_delay, 0);
}

#[test]
fn waterlogged_speleothem_schedules_a_water_tick() {
    let tip = speleothem(blocks::POINTED_DRIPSTONE, Direction::Up, SpeleothemThickness::Tip).set(WATERLOGGED, true);
    let mut column = Column::new(blocks::STONE.default_state(), tip, blocks::WATER.default_state(), false);
    assert_eq!(update_shape(&mut column, Direction::North), tip);
    assert_eq!(column.fluid_tick_delay, 5);
}

#[test]
fn update_shape_matches_vanilla() {
    let centres = speleothems();
    let mut neighbours = vec![blocks::AIR.default_state(), blocks::STONE.default_state(), blocks::OAK_FENCE.default_state(), blocks::WATER.default_state()];
    neighbours.extend(centres.iter().copied());
    let mut lines = include_str!("speleothem_oracle.txt").lines();
    let mut mismatches = 0;
    for &centre in &centres {
        for &below in &neighbours {
            let expected = lines.next().unwrap().as_bytes();
            let mut i = 0;
            for &above in &neighbours {
                for direction in [Direction::Up, Direction::Down] {
                    for scheduled in [false, true] {
                        let mut column = Column::new(below, centre, above, scheduled);
                        let result = update_shape(&mut column, direction);
                        let base = match column.block_tick_delay {
                            0 => b'a',
                            1 => b'A',
                            _ => b'k',
                        };
                        let actual = base + result.get(THICKNESS) as u8;
                        if actual != expected[i] {
                            if mismatches < 20 {
                                eprintln!("{centre:?} below {below:?} above {above:?} {direction:?} scheduled {scheduled}: crisp {} vanilla {}", actual as char, expected[i] as char);
                            }
                            mismatches += 1;
                        }
                        i += 1;
                    }
                }
            }
        }
    }
    assert!(lines.next().is_none());
    assert_eq!(mismatches, 0);
}

#[test]
fn placed_stalagmites_stack_and_reshape() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
    let ground = BlockPos::new(0, -61, 0);
    for y in 0..2 {
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::POINTED_DRIPSTONE, 1));
        let clicked = ground.offset(0, y, 0);
        let hit = BlockHitResult { location: Vec3::new(0.5, clicked.y as f64 + 1.0, 0.5), direction: Direction::Up, block_pos: clicked, inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
    }
    assert_eq!(level.block_state(ground.above()), speleothem(blocks::POINTED_DRIPSTONE, Direction::Up, SpeleothemThickness::Frustum));
    assert_eq!(level.block_state(ground.offset(0, 2, 0)), speleothem(blocks::POINTED_DRIPSTONE, Direction::Up, SpeleothemThickness::Tip));
}

#[test]
fn stalactite_drips_water_into_a_cauldron() {
    let mut level = testing::level();
    let tip_pos = BlockPos::new(0, 3, 0);
    let cauldron_pos = BlockPos::new(0, -5, 0);
    level.set_block(tip_pos.offset(0, 2, 0), blocks::WATER.default_state(), UPDATE_CLIENTS);
    level.set_block(tip_pos.above(), blocks::DRIPSTONE_BLOCK.default_state(), UPDATE_CLIENTS);
    let tip = speleothem(blocks::POINTED_DRIPSTONE, Direction::Down, SpeleothemThickness::Tip);
    level.set_block(tip_pos, tip, UPDATE_CLIENTS);
    level.set_block(cauldron_pos, blocks::CAULDRON.default_state(), UPDATE_CLIENTS);
    super::pointed_dripstone_block::maybe_transfer_fluid(tip, &mut level, tip_pos, 0.1);
    assert!(level.has_scheduled_tick(cauldron_pos, blocks::CAULDRON));
    super::of(blocks::CAULDRON.default_state()).tick(blocks::CAULDRON.default_state(), &mut level, cauldron_pos);
    assert_eq!(level.block_state(cauldron_pos), blocks::WATER_CAULDRON.default_state());
    let water_cauldron = level.block_state(cauldron_pos);
    super::of(water_cauldron).tick(water_cauldron, &mut level, cauldron_pos);
    assert_eq!(level.block_state(cauldron_pos).get(LEVEL_CAULDRON), 2);
}

#[test]
fn stalactite_turns_mud_above_into_clay() {
    let mut level = testing::level();
    let tip_pos = BlockPos::new(0, 3, 0);
    let mud_pos = tip_pos.offset(0, 2, 0);
    level.set_block(mud_pos, blocks::MUD.default_state(), UPDATE_CLIENTS);
    level.set_block(tip_pos.above(), blocks::DRIPSTONE_BLOCK.default_state(), UPDATE_CLIENTS);
    let tip = speleothem(blocks::POINTED_DRIPSTONE, Direction::Down, SpeleothemThickness::Tip);
    level.set_block(tip_pos, tip, UPDATE_CLIENTS);
    super::pointed_dripstone_block::maybe_transfer_fluid(tip, &mut level, tip_pos, 0.1);
    assert_eq!(level.block_state(mud_pos), blocks::CLAY.default_state());
}
