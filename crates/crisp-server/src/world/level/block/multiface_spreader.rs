use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, Fluid, SupportType, block_tags, blocks};

use super::multiface_block;
use crate::world::level::{LevelAccessor, LevelReader};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpreadType {
    SamePosition,
    SamePlane,
    WrapAround,
}

impl SpreadType {
    pub fn get_spread_pos(self, pos: BlockPos, spread_direction: Direction, from_face: Direction) -> SpreadPos {
        match self {
            SpreadType::SamePosition => SpreadPos { pos, face: spread_direction },
            SpreadType::SamePlane => SpreadPos { pos: pos.relative(spread_direction), face: from_face },
            SpreadType::WrapAround => SpreadPos { pos: pos.relative(spread_direction).relative(from_face), face: spread_direction.opposite() },
        }
    }
}

pub const DEFAULT_SPREAD_ORDER: &[SpreadType] = &[SpreadType::SamePosition, SpreadType::SamePlane, SpreadType::WrapAround];

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SpreadPos {
    pub pos: BlockPos,
    pub face: Direction,
}

pub enum SpreadConfig {
    Default(Block),
    SculkVein(&'static [SpreadType]),
}

pub enum SpreadRandom<'r, R> {
    Own(&'r mut R),
    Level,
}

impl<R: RandomSource> SpreadRandom<'_, R> {
    fn all_shuffled(&mut self, level: &mut dyn LevelAccessor) -> [Direction; 6] {
        match self {
            SpreadRandom::Own(random) => Direction::all_shuffled(*random),
            SpreadRandom::Level => Direction::all_shuffled(&mut LevelRandom(level)),
        }
    }
}

struct LevelRandom<'l>(&'l mut dyn LevelAccessor);

impl RandomSource for LevelRandom<'_> {
    fn set_seed(&mut self, seed: i64) {
        self.0.get_random().set_seed(seed);
    }

    fn next_int(&mut self) -> i32 {
        self.0.get_random().next_int()
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        self.0.get_random().next_int_bounded(bound)
    }

    fn next_long(&mut self) -> i64 {
        self.0.get_random().next_long()
    }

    fn next_boolean(&mut self) -> bool {
        self.0.get_random().next_boolean()
    }

    fn next_float(&mut self) -> f32 {
        self.0.get_random().next_float()
    }

    fn next_double(&mut self) -> f64 {
        self.0.get_random().next_double()
    }

    fn next_gaussian(&mut self) -> f64 {
        self.0.get_random().next_gaussian()
    }
}

pub struct MultifaceSpreader {
    config: SpreadConfig,
}

impl MultifaceSpreader {
    pub const fn new(config: SpreadConfig) -> MultifaceSpreader {
        MultifaceSpreader { config }
    }

    fn block(&self) -> Block {
        match self.config {
            SpreadConfig::Default(block) => block,
            SpreadConfig::SculkVein(_) => blocks::SCULK_VEIN,
        }
    }

    fn get_spread_types(&self) -> &'static [SpreadType] {
        match self.config {
            SpreadConfig::Default(_) => DEFAULT_SPREAD_ORDER,
            SpreadConfig::SculkVein(types) => types,
        }
    }

    fn is_other_block_valid_as_source(&self, state: BlockState) -> bool {
        match self.config {
            SpreadConfig::Default(_) => false,
            SpreadConfig::SculkVein(_) => !state.is_block(blocks::SCULK_VEIN),
        }
    }

    fn can_spread_from(&self, state: BlockState, face: Direction) -> bool {
        self.is_other_block_valid_as_source(state) || multiface_block::has_face(state, face)
    }

    fn default_state_can_be_replaced(&self, existing_state: BlockState) -> bool {
        existing_state.is_air() || existing_state.is_block(self.block()) || existing_state.is_block(blocks::WATER) && existing_state.fluid_state().is_source()
    }

    fn state_can_be_replaced(&self, level: &mut dyn LevelReader, source_pos: BlockPos, placement_pos: BlockPos, placement_direction: Direction, existing_state: BlockState) -> bool {
        match self.config {
            SpreadConfig::Default(_) => self.default_state_can_be_replaced(existing_state),
            SpreadConfig::SculkVein(_) => {
                let against_state = level.block_state(placement_pos.relative(placement_direction));
                if against_state.is_block(blocks::SCULK) || against_state.is_block(blocks::SCULK_CATALYST) || against_state.is_block(blocks::MOVING_PISTON) {
                    return false;
                }
                if source_pos.dist_manhattan(placement_pos) == 2 {
                    let neighbour_pos = source_pos.relative(placement_direction.opposite());
                    if level.block_state(neighbour_pos).is_face_sturdy(placement_direction, SupportType::Full) {
                        return false;
                    }
                }
                let fluid_state = existing_state.fluid_state();
                if !fluid_state.is_empty() && fluid_state.fluid != Fluid::Water {
                    false
                } else if existing_state.is(&block_tags::FIRE) {
                    false
                } else {
                    existing_state.can_be_replaced() || self.default_state_can_be_replaced(existing_state)
                }
            }
        }
    }

    fn can_spread_into(&self, level: &mut dyn LevelReader, source_pos: BlockPos, spread_pos: SpreadPos) -> bool {
        let existing_state = level.block_state(spread_pos.pos);
        self.state_can_be_replaced(level, source_pos, spread_pos.pos, spread_pos.face, existing_state)
            && multiface_block::is_valid_state_for_placement(self.block(), level, existing_state, spread_pos.pos, spread_pos.face)
    }

    fn place_block(&self, level: &mut dyn LevelAccessor, spread_pos: SpreadPos, old_state: BlockState, post_process: bool) -> bool {
        match multiface_block::get_state_for_placement(self.block(), old_state, level, spread_pos.pos, spread_pos.face) {
            Some(spread_state) => {
                if post_process {
                    level.mark_pos_for_post_processing(spread_pos.pos);
                }
                level.set_block(spread_pos.pos, spread_state, 2)
            }
            None => false,
        }
    }

    pub fn can_spread_in_any_direction(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, starting_face: Direction) -> bool {
        Direction::VALUES.into_iter().any(|spread_direction| self.get_spread_from_face_toward_direction(state, level, pos, starting_face, spread_direction).is_some())
    }

    pub fn spread_from_random_face_toward_random_direction<R: RandomSource>(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        random: &mut SpreadRandom<R>,
    ) -> Option<SpreadPos> {
        for face_direction in random.all_shuffled(level) {
            if self.can_spread_from(state, face_direction)
                && let Some(spread_pos) = self.spread_from_face_toward_random_direction(state, level, pos, face_direction, random, false)
            {
                return Some(spread_pos);
            }
        }
        None
    }

    pub fn spread_all(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, post_process: bool) -> i64 {
        let mut total = 0;
        for face_direction in Direction::VALUES {
            if self.can_spread_from(state, face_direction) {
                total += self.spread_from_face_toward_all_directions(state, level, pos, face_direction, post_process);
            }
        }
        total
    }

    fn spread_from_face_toward_all_directions(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, starting_face: Direction, post_process: bool) -> i64 {
        Direction::VALUES.into_iter().filter(|&spread_direction| self.spread_from_face_toward_direction(state, level, pos, starting_face, spread_direction, post_process).is_some()).count() as i64
    }

    pub fn spread_from_face_toward_random_direction<R: RandomSource>(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        starting_face: Direction,
        random: &mut SpreadRandom<R>,
        post_process: bool,
    ) -> Option<SpreadPos> {
        for spread_direction in random.all_shuffled(level) {
            if let Some(spread_pos) = self.spread_from_face_toward_direction(state, level, pos, starting_face, spread_direction, post_process) {
                return Some(spread_pos);
            }
        }
        None
    }

    pub fn spread_from_face_toward_direction(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        from_face: Direction,
        spread_direction: Direction,
        post_process: bool,
    ) -> Option<SpreadPos> {
        let spread_pos = self.get_spread_from_face_toward_direction(state, level, pos, from_face, spread_direction)?;
        self.spread_to_face(level, spread_pos, post_process)
    }

    pub fn get_spread_from_face_toward_direction(
        &self,
        state: BlockState,
        level: &mut dyn LevelReader,
        pos: BlockPos,
        starting_face: Direction,
        spread_direction: Direction,
    ) -> Option<SpreadPos> {
        if spread_direction.axis() == starting_face.axis() {
            return None;
        }
        if self.is_other_block_valid_as_source(state) || multiface_block::has_face(state, starting_face) && !multiface_block::has_face(state, spread_direction) {
            for &ty in self.get_spread_types() {
                let spread_pos = ty.get_spread_pos(pos, spread_direction, starting_face);
                if self.can_spread_into(level, pos, spread_pos) {
                    return Some(spread_pos);
                }
            }
        }
        None
    }

    pub fn spread_to_face(&self, level: &mut dyn LevelAccessor, spread_pos: SpreadPos, post_process: bool) -> Option<SpreadPos> {
        let old_state = level.block_state(spread_pos.pos);
        self.place_block(level, spread_pos, old_state, post_process).then_some(spread_pos)
    }
}

pub fn spreader_for(block: Block) -> Option<MultifaceSpreader> {
    if block == blocks::SCULK_VEIN {
        Some(MultifaceSpreader::new(SpreadConfig::SculkVein(DEFAULT_SPREAD_ORDER)))
    } else if block.is_instance_of("MultifaceSpreadeableBlock") {
        Some(MultifaceSpreader::new(SpreadConfig::Default(block)))
    } else {
        None
    }
}
