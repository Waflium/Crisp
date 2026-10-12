use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::{Block, BlockState, blocks, registry, registry_id};

use super::BlockBehaviour;
use super::flower_block::FlowerBlock;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes::EYEBLOSSOM_OPEN;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{LevelAccessor, LevelReader};

const EYEBLOSSOM_XZ_RANGE: i32 = 3;
const EYEBLOSSOM_Y_RANGE: i32 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Open,
    Closed,
}

impl Type {
    fn open(self) -> bool {
        self == Type::Open
    }

    fn long_switch_sound(self) -> &'static str {
        match self {
            Type::Open => "minecraft:block.eyeblossom.open_long",
            Type::Closed => "minecraft:block.eyeblossom.close_long",
        }
    }

    fn short_switch_sound(self) -> &'static str {
        match self {
            Type::Open => "minecraft:block.eyeblossom.open",
            Type::Closed => "minecraft:block.eyeblossom.close",
        }
    }

    fn particle_color(self) -> i32 {
        match self {
            Type::Open => 16545810,
            Type::Closed => 6250335,
        }
    }

    pub fn block(self) -> Block {
        if self.open() { blocks::OPEN_EYEBLOSSOM } else { blocks::CLOSED_EYEBLOSSOM }
    }

    pub fn state(self) -> BlockState {
        self.block().default_state()
    }

    pub fn transform(self) -> Type {
        Type::from_boolean(!self.open())
    }

    pub fn from_boolean(open: bool) -> Type {
        if open { Type::Open } else { Type::Closed }
    }

    pub fn spawn_transform_particle(self, level: &mut ServerLevel, pos: BlockPos) {
        let start = Vec3::at_center_of(pos);
        let lifetime = 0.5 + level.random.next_double();
        let velocity = Vec3::new(level.random.next_double() - 0.5, level.random.next_double() + 1.0, level.random.next_double() - 0.5);
        let target = start.add_vec(velocity.scale(lifetime));
        let (color, duration) = (self.particle_color(), (20.0 * lifetime) as i32);
        level.send_particles(
            "minecraft:trail",
            |w| {
                w.f64(target.x);
                w.f64(target.y);
                w.f64(target.z);
                w.i32(color);
                w.varint(duration);
            },
            start.x,
            start.y,
            start.z,
            1,
            (0.0, 0.0, 0.0),
            0.0,
        );
    }
}

pub struct EyeblossomBlock {
    ty: Type,
}

impl EyeblossomBlock {
    pub fn new(block: Block) -> EyeblossomBlock {
        EyeblossomBlock { ty: Type::from_boolean(block == blocks::OPEN_EYEBLOSSOM) }
    }

    fn try_changing_state(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let should_be_open = level.environment_attributes().get_value_at(EYEBLOSSOM_OPEN, pos).to_boolean(self.ty.open());
        if should_be_open == self.ty.open() {
            return false;
        }
        let new_type = self.ty.transform();
        level.set_block(pos, new_type.state(), UPDATE_ALL);
        new_type.spawn_transform_particle(level, pos);
        let min_pos = pos.offset(-EYEBLOSSOM_XZ_RANGE, -EYEBLOSSOM_Y_RANGE, -EYEBLOSSOM_XZ_RANGE);
        let max_pos = pos.offset(EYEBLOSSOM_XZ_RANGE, EYEBLOSSOM_Y_RANGE, EYEBLOSSOM_XZ_RANGE);
        level.find_blocks_in(min_pos, max_pos).filter_state(|nearby_state| nearby_state == state).for_each(level, |level, nearby_pos, _| {
            let distance = pos.dist_sqr(nearby_pos).sqrt();
            let delay = level.random.next_int_between_inclusive((distance * 5.0) as i32, (distance * 10.0) as i32);
            level.schedule_tick(nearby_pos, state.block(), delay);
        });
        true
    }
}

impl BlockBehaviour for EyeblossomBlock {
    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if self.try_changing_state(state, level, pos) {
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, self.ty.transform().long_switch_sound()), SoundSource::Blocks, 1.0, 1.0);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if self.try_changing_state(state, level, pos) {
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, self.ty.transform().short_switch_sound()), SoundSource::Blocks, 1.0, 1.0);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        FlowerBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        FlowerBlock.can_survive(state, level, pos)
    }
}
