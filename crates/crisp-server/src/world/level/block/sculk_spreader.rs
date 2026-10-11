use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::{AxisDirection, BlockPos, Direction, Axis};
use crisp_data::{Tag, block_tags};

use super::multiface_block;
use super::sculk_behaviour::{self, SculkBehaviour};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util;

pub struct SculkSpreader {
    pub is_world_generation: bool,
    pub replaceable_blocks: &'static Tag,
    pub growth_spawn_cost: i32,
    pub no_growth_radius: i32,
    pub charge_decay_rate: i32,
    pub additional_decay_rate: i32,
    cursors: Vec<ChargeCursor>,
}

impl SculkSpreader {
    pub fn create_world_gen_spreader() -> SculkSpreader {
        SculkSpreader {
            is_world_generation: true,
            replaceable_blocks: &block_tags::SCULK_REPLACEABLE_WORLD_GEN,
            growth_spawn_cost: 50,
            no_growth_radius: 1,
            charge_decay_rate: 5,
            additional_decay_rate: 10,
            cursors: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cursors.clear();
    }

    pub fn add_cursors(&mut self, start_pos: BlockPos, mut charge: i32) {
        while charge > 0 {
            let current_charge = charge.min(1000);
            self.add_cursor(ChargeCursor::new(start_pos, current_charge));
            charge -= current_charge;
        }
    }

    fn add_cursor(&mut self, cursor: ChargeCursor) {
        if self.cursors.len() < 32 {
            self.cursors.push(cursor);
        }
    }

    pub fn update_cursors<R: RandomSource>(&mut self, level: &mut WorldGenRegion, origin_pos: BlockPos, random: &mut R, spread_veins: bool) {
        if self.cursors.is_empty() {
            return;
        }
        let mut cursors = std::mem::take(&mut self.cursors);
        let mut processed_cursors: Vec<ChargeCursor> = Vec::with_capacity(cursors.len());
        let mut mergeable_cursors: Vec<(BlockPos, usize)> = Vec::new();
        for mut cursor in cursors.drain(..) {
            if cursor.is_pos_unreasonable(origin_pos) {
                continue;
            }
            cursor.update(level, origin_pos, random, self, spread_veins);
            if cursor.charge <= 0 {
                continue;
            }
            let pos = cursor.pos;
            match mergeable_cursors.iter_mut().find(|(p, _)| *p == pos) {
                None => {
                    mergeable_cursors.push((pos, processed_cursors.len()));
                    processed_cursors.push(cursor);
                }
                Some(entry) => {
                    let existing_charge = processed_cursors[entry.1].charge;
                    if !self.is_world_generation && cursor.charge + existing_charge <= 1000 {
                        let existing = &mut processed_cursors[entry.1];
                        existing.charge += cursor.charge;
                        existing.update_delay = existing.update_delay.min(cursor.update_delay);
                    } else {
                        let less = cursor.charge < existing_charge;
                        processed_cursors.push(cursor);
                        if less {
                            entry.1 = processed_cursors.len() - 1;
                        }
                    }
                }
            }
        }
        self.cursors = processed_cursors;
    }
}

static NON_CORNER_NEIGHBOURS: LazyLock<Vec<(i32, i32, i32)>> = LazyLock::new(|| {
    let mut list = Vec::with_capacity(18);
    for z in -1..=1 {
        for y in -1..=1 {
            for x in -1..=1 {
                if (x == 0 || y == 0 || z == 0) && (x, y, z) != (0, 0, 0) {
                    list.push((x, y, z));
                }
            }
        }
    }
    list
});

pub struct ChargeCursor {
    pub pos: BlockPos,
    pub charge: i32,
    update_delay: i32,
    pub decay_delay: i32,
    facings: Option<u8>,
}

impl ChargeCursor {
    pub fn new(pos: BlockPos, charge: i32) -> ChargeCursor {
        ChargeCursor { pos, charge, update_delay: 0, decay_delay: 1, facings: None }
    }

    fn is_pos_unreasonable(&self, origin_pos: BlockPos) -> bool {
        (self.pos.x - origin_pos.x).abs().max((self.pos.y - origin_pos.y).abs()).max((self.pos.z - origin_pos.z).abs()) > 1024
    }

    fn update<R: RandomSource>(&mut self, level: &mut WorldGenRegion, origin_pos: BlockPos, random: &mut R, spreader: &SculkSpreader, spread_veins: bool) {
        if self.charge <= 0 {
            return;
        }
        if self.update_delay > 0 {
            self.update_delay -= 1;
            return;
        }
        let mut current_state = level.get_block_state(self.pos);
        let mut sculk_behaviour = SculkBehaviour::get_block_behaviour(current_state);
        if spread_veins && sculk_behaviour.attempt_spread_vein(level, self.pos, current_state, self.facings, spreader.is_world_generation) && sculk_behaviour.can_change_block_state_on_spread() {
            current_state = level.get_block_state(self.pos);
            sculk_behaviour = SculkBehaviour::get_block_behaviour(current_state);
        }
        self.charge = sculk_behaviour.attempt_use_charge(self, level, origin_pos, random, spreader, spread_veins);
        if self.charge <= 0 {
            sculk_behaviour.on_discharged(level, current_state, self.pos);
            return;
        }
        match get_valid_movement_pos(level, self.pos, random, origin_pos, spreader) {
            Some(transfer_pos) => {
                sculk_behaviour.on_discharged(level, current_state, self.pos);
                self.pos = transfer_pos;
                current_state = level.get_block_state(transfer_pos);
            }
            None if spreader.is_world_generation => {
                sculk_behaviour.on_discharged(level, current_state, self.pos);
                self.charge = 0;
                return;
            }
            None => {}
        }
        if SculkBehaviour::of(current_state.block()).is_some() {
            self.facings = Some(available_faces(current_state));
        }
        self.decay_delay = sculk_behaviour.update_decay_delay(self.decay_delay);
        self.update_delay = sculk_behaviour.get_sculk_spread_delay();
    }
}

fn available_faces(state: crisp_data::BlockState) -> u8 {
    if !state.block().is_instance_of("MultifaceBlock") {
        return 0;
    }
    Direction::VALUES.into_iter().filter(|&direction| multiface_block::has_face(state, direction)).fold(0, |faces, direction| faces | 1 << direction.ordinal())
}

fn get_valid_movement_pos(level: &mut WorldGenRegion, pos: BlockPos, random: &mut impl RandomSource, origin_pos: BlockPos, spreader: &SculkSpreader) -> Option<BlockPos> {
    let mut sculk_position = pos;
    let mut offsets = NON_CORNER_NEIGHBOURS.clone();
    util::shuffle(&mut offsets, random);
    for (x, y, z) in offsets {
        let neighbour = pos.offset(x, y, z);
        if can_move_to_pos(origin_pos, neighbour, spreader) {
            let transferee = level.get_block_state(neighbour);
            if SculkBehaviour::of(transferee.block()).is_some() && is_movement_unobstructed(level, pos, neighbour) {
                sculk_position = neighbour;
                if sculk_behaviour::has_substrate_access(level, transferee, neighbour) {
                    break;
                }
            }
        }
    }
    (sculk_position != pos).then_some(sculk_position)
}

fn can_move_to_pos(origin: BlockPos, target: BlockPos, spreader: &SculkSpreader) -> bool {
    if !spreader.is_world_generation {
        return true;
    }
    let dx = origin.x - target.x;
    let dz = origin.z - target.z;
    dx * dx + dz * dz <= 144
}

fn is_movement_unobstructed(level: &mut WorldGenRegion, from: BlockPos, to: BlockPos) -> bool {
    if from.dist_manhattan(to) == 1 {
        return true;
    }
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    let sign = |delta: i32| if delta < 0 { AxisDirection::Negative } else { AxisDirection::Positive };
    let direction_x = Direction::from_axis_and_direction(Axis::X, sign(dx));
    let direction_y = Direction::from_axis_and_direction(Axis::Y, sign(dy));
    let direction_z = Direction::from_axis_and_direction(Axis::Z, sign(dz));
    if dx == 0 {
        is_unobstructed(level, from, direction_y) || is_unobstructed(level, from, direction_z)
    } else if dy == 0 {
        is_unobstructed(level, from, direction_x) || is_unobstructed(level, from, direction_z)
    } else {
        is_unobstructed(level, from, direction_x) || is_unobstructed(level, from, direction_y)
    }
}

fn is_unobstructed(level: &mut WorldGenRegion, from: BlockPos, direction: Direction) -> bool {
    let test_pos = from.relative(direction);
    !level.get_block_state(test_pos).is_face_sturdy_full(direction.opposite())
}
