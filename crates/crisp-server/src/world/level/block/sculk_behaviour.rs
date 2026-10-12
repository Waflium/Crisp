use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::{CAN_SUMMON, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags, blocks};

use super::multiface_block::{self, get_face_property, has_any_face, has_face};
use super::multiface_spreader::{DEFAULT_SPREAD_ORDER, MultifaceSpreader, SpreadConfig, SpreadType};
use super::sculk_spreader::{ChargeCursor, SculkSpreader};
use crate::world::level::world_gen_level::WorldGenLevel;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SculkBehaviour {
    Default,
    Sculk,
    SculkVein,
}

const VEIN_SPREADER: MultifaceSpreader = MultifaceSpreader::new(SpreadConfig::SculkVein(DEFAULT_SPREAD_ORDER));
const SAME_SPACE_SPREADER: MultifaceSpreader = MultifaceSpreader::new(SpreadConfig::SculkVein(&[SpreadType::SamePosition]));

impl SculkBehaviour {
    pub fn of(block: Block) -> Option<SculkBehaviour> {
        if block == blocks::SCULK {
            Some(SculkBehaviour::Sculk)
        } else if block == blocks::SCULK_VEIN {
            Some(SculkBehaviour::SculkVein)
        } else {
            None
        }
    }

    pub fn get_block_behaviour(state: BlockState) -> SculkBehaviour {
        SculkBehaviour::of(state.block()).unwrap_or(SculkBehaviour::Default)
    }

    pub fn get_sculk_spread_delay(self) -> i32 {
        1
    }

    pub fn on_discharged(self, level: &mut impl WorldGenLevel, state: BlockState, pos: BlockPos) {
        if self == SculkBehaviour::SculkVein {
            sculk_vein_on_discharged(level, state, pos);
        }
    }

    pub fn attempt_spread_vein(self, level: &mut impl WorldGenLevel, pos: BlockPos, state: BlockState, facings: Option<u8>, post_process: bool) -> bool {
        if self == SculkBehaviour::Default {
            match facings {
                None => return SAME_SPACE_SPREADER.spread_all(level.get_block_state(pos), level, pos, post_process) > 0,
                Some(faces) if faces != 0 => {
                    return if !state.is_air() && state.fluid_state().fluid != Fluid::Water { false } else { regrow(level, pos, state, faces) };
                }
                Some(_) => {}
            }
        }
        VEIN_SPREADER.spread_all(state, level, pos, post_process) > 0
    }

    pub fn can_change_block_state_on_spread(self) -> bool {
        self != SculkBehaviour::Sculk
    }

    pub fn update_decay_delay(self, age: i32) -> i32 {
        if self == SculkBehaviour::Default { (age - 1).max(0) } else { 1 }
    }

    pub fn attempt_use_charge<R: RandomSource>(self, cursor: &ChargeCursor, level: &mut impl WorldGenLevel, origin_pos: BlockPos, random: &mut R, spreader: &SculkSpreader, spread_veins: bool) -> i32 {
        match self {
            SculkBehaviour::Default => {
                if cursor.decay_delay > 0 {
                    cursor.charge
                } else {
                    0
                }
            }
            SculkBehaviour::Sculk => sculk_attempt_use_charge(cursor, level, origin_pos, random, spreader),
            SculkBehaviour::SculkVein => {
                if spread_veins && attempt_place_sculk(spreader, level, cursor.pos, random) {
                    cursor.charge - 1
                } else if random.next_int_bounded(spreader.charge_decay_rate) == 0 {
                    mth::floor_f(cursor.charge as f32 * 0.5)
                } else {
                    cursor.charge
                }
            }
        }
    }
}

pub fn regrow(level: &mut impl WorldGenLevel, pos: BlockPos, existing: BlockState, faces: u8) -> bool {
    let mut has_at_least_one_face = false;
    let mut new_state = blocks::SCULK_VEIN.default_state();
    for face in Direction::VALUES {
        if faces & (1 << face.ordinal()) != 0 && multiface_block::can_attach_to_at(level, pos, face) {
            new_state = new_state.set(get_face_property(face), true);
            has_at_least_one_face = true;
        }
    }
    if !has_at_least_one_face {
        return false;
    }
    if !existing.fluid_state().is_empty() {
        new_state = new_state.set(WATERLOGGED, true);
    }
    level.set_block_and_update(pos, new_state);
    true
}

pub fn has_substrate_access(level: &mut impl WorldGenLevel, state: BlockState, pos: BlockPos) -> bool {
    if !state.is_block(blocks::SCULK_VEIN) {
        return false;
    }
    Direction::VALUES.into_iter().any(|direction| has_face(state, direction) && level.get_block_state(pos.relative(direction)).is(&block_tags::SCULK_REPLACEABLE))
}

fn sculk_vein_on_discharged(level: &mut impl WorldGenLevel, mut state: BlockState, pos: BlockPos) {
    if !state.is_block(blocks::SCULK_VEIN) {
        return;
    }
    for dir in Direction::VALUES {
        let side_property = get_face_property(dir);
        if state.get(side_property) && level.get_block_state(pos.relative(dir)).is_block(blocks::SCULK) {
            state = state.set(side_property, false);
        }
    }
    if !has_any_face(state) {
        let fluid_state = level.get_fluid_state(pos);
        state = if fluid_state.is_empty() { blocks::AIR } else { blocks::WATER }.default_state();
    }
    level.set_block_and_update(pos, state);
}

fn attempt_place_sculk<R: RandomSource>(spreader: &SculkSpreader, level: &mut impl WorldGenLevel, pos: BlockPos, random: &mut R) -> bool {
    let state = level.get_block_state(pos);
    for support in Direction::all_shuffled(random) {
        if has_face(state, support) {
            let support_pos = pos.relative(support);
            let support_state = level.get_block_state(support_pos);
            if support_state.is(spreader.replaceable_blocks) {
                let default_sculk = blocks::SCULK.default_state();
                level.set_block_and_update(support_pos, default_sculk);
                VEIN_SPREADER.spread_all(default_sculk, level, support_pos, spreader.is_world_generation);
                let skip = support.opposite();
                for vein_blocks in Direction::VALUES {
                    if vein_blocks != skip {
                        let vein_pos = support_pos.relative(vein_blocks);
                        let possible_vein_block = level.get_block_state(vein_pos);
                        if possible_vein_block.is_block(blocks::SCULK_VEIN) {
                            sculk_vein_on_discharged(level, possible_vein_block, vein_pos);
                        }
                    }
                }
                return true;
            }
        }
    }
    false
}

fn sculk_attempt_use_charge<R: RandomSource>(cursor: &ChargeCursor, level: &mut impl WorldGenLevel, origin_pos: BlockPos, random: &mut R, spreader: &SculkSpreader) -> i32 {
    let charge = cursor.charge;
    if charge == 0 || random.next_int_bounded(spreader.charge_decay_rate) != 0 {
        return charge;
    }
    let charge_pos = cursor.pos;
    let is_close_to_catalyst = charge_pos.closer_than(origin_pos, spreader.no_growth_radius as f64);
    if !is_close_to_catalyst && can_place_growth(level, charge_pos) {
        let xp_per_growth_spawn = spreader.growth_spawn_cost;
        if random.next_int_bounded(xp_per_growth_spawn) < charge {
            let growth_placement = charge_pos.above();
            let growth_state = get_random_growth_state(level, growth_placement, random, spreader.is_world_generation);
            level.set_block_and_update(growth_placement, growth_state);
        }
        (charge - xp_per_growth_spawn).max(0)
    } else if random.next_int_bounded(spreader.additional_decay_rate) != 0 {
        charge
    } else {
        charge - if is_close_to_catalyst { 1 } else { get_decay_penalty(spreader, charge_pos, origin_pos, charge) }
    }
}

fn get_decay_penalty(spreader: &SculkSpreader, pos: BlockPos, origin_pos: BlockPos, charge: i32) -> i32 {
    let no_growth_radius = spreader.no_growth_radius;
    let outer = pos.dist_sqr(origin_pos).sqrt() as f32 - no_growth_radius as f32;
    let outer_distance_squared = outer * outer;
    let max_reach_squared = (24 - no_growth_radius) * (24 - no_growth_radius);
    let distance_factor = 1.0f32.min(outer_distance_squared / max_reach_squared as f32);
    1.max((charge as f32 * distance_factor * 0.5) as i32)
}

fn get_random_growth_state(level: &mut impl WorldGenLevel, pos: BlockPos, random: &mut impl RandomSource, is_world_gen: bool) -> BlockState {
    let state = if random.next_int_bounded(11) == 0 { blocks::SCULK_SHRIEKER.default_state().set(CAN_SUMMON, is_world_gen) } else { blocks::SCULK_SENSOR.default_state() };
    if state.has(WATERLOGGED) && !level.get_fluid_state(pos).is_empty() { state.set(WATERLOGGED, true) } else { state }
}

fn can_place_growth(level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
    let state_above = level.get_block_state(pos.above());
    if !(state_above.is_air() || state_above.is_block(blocks::WATER) && state_above.fluid_state().fluid == Fluid::Water) {
        return false;
    }
    let mut count = 0;
    for x in pos.x - 4..=pos.x + 4 {
        for y in pos.y..=pos.y + 2 {
            for z in pos.z - 4..=pos.z + 4 {
                if level.get_block_state(BlockPos::new(x, y, z)).is(&block_tags::SCULK_GROWTH_INHIBITORS) {
                    count += 1;
                    if count > 2 {
                        return false;
                    }
                }
            }
        }
    }
    true
}
