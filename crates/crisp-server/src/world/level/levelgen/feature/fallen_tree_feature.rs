use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use crisp_data::properties::AXIS;
use serde_json::Value;

use super::feature::mark_above_for_post_processing;
use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use super::tree_feature::{self, TreeSetter};
use super::treedecorators::tree_decorator::{Context, TreeDecorator};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::java_collections::JavaObjectHashSet;
use crate::util::valueproviders::IntProvider;

pub struct FallenTreeFeature {
    trunk_provider: BlockStateProviderHolder,
    log_length: IntProvider,
    stump_decorators: Vec<TreeDecorator>,
    log_decorators: Vec<TreeDecorator>,
}

impl FallenTreeFeature {
    pub fn parse(json: &Value) -> Result<FallenTreeFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let decorators = |key: &str| -> Result<Vec<TreeDecorator>, String> { field(key)?.as_array().ok_or("decorators is a list")?.iter().map(TreeDecorator::parse).collect() };
        Ok(FallenTreeFeature {
            trunk_provider: BlockStateProviderHolder::parse(field("trunk_provider")?)?,
            log_length: IntProvider::parse(field("log_length")?)?,
            stump_decorators: decorators("stump_decorators")?,
            log_decorators: decorators("log_decorators")?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        self.place_fallen_tree(origin, level, random);
        true
    }

    fn place_fallen_tree<R: RandomSource>(&self, origin: BlockPos, level: &mut WorldGenRegion, random: &mut R) {
        self.place_stump(level, random, origin);
        let direction = Direction::get_random_horizontal_direction(random);
        let log_length = self.log_length.sample(random) - 2;
        let distance = 2 + random.next_int_bounded(2);
        let mut log_start_pos = origin.relative_n(direction, distance);
        self.set_ground_height_for_fallen_log_start_pos(level, &mut log_start_pos);
        if self.can_place_entire_fallen_log(level, log_length, log_start_pos, direction) {
            self.place_fallen_log(level, random, log_length, log_start_pos, direction);
        }
    }

    fn set_ground_height_for_fallen_log_start_pos(&self, level: &mut WorldGenRegion, log_start_pos: &mut BlockPos) {
        *log_start_pos = log_start_pos.above();
        for _ in 0..6 {
            if self.may_place_on(level, *log_start_pos) {
                return;
            }
            *log_start_pos = log_start_pos.below();
        }
    }

    fn place_stump<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, stump_pos: BlockPos) {
        let stump = self.place_log_block(level, random, stump_pos, |state| state);
        let mut logs = JavaObjectHashSet::default();
        logs.insert(stump);
        decorate_logs(level, random, &logs, &self.stump_decorators);
    }

    fn can_place_entire_fallen_log(&self, level: &mut WorldGenRegion, log_length: i32, mut log_start_pos: BlockPos, direction: Direction) -> bool {
        let mut gap_in_ground = 0;
        for _ in 0..log_length {
            if !tree_feature::valid_tree_pos(level, log_start_pos) {
                return false;
            }
            if !is_over_solid_ground(level, log_start_pos) {
                gap_in_ground += 1;
                if gap_in_ground > 2 {
                    return false;
                }
            } else {
                gap_in_ground = 0;
            }
            log_start_pos = log_start_pos.relative(direction);
        }
        true
    }

    fn place_fallen_log<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, log_length: i32, mut log_start_pos: BlockPos, direction: Direction) {
        let mut fallen_log = JavaObjectHashSet::default();
        let axis = direction.axis();
        for _ in 0..log_length {
            fallen_log.insert(self.place_log_block(level, random, log_start_pos, |state| state.try_set(AXIS, axis)));
            log_start_pos = log_start_pos.relative(direction);
        }
        decorate_logs(level, random, &fallen_log, &self.log_decorators);
    }

    fn may_place_on(&self, level: &mut WorldGenRegion, block_pos: BlockPos) -> bool {
        tree_feature::valid_tree_pos(level, block_pos) && is_over_solid_ground(level, block_pos)
    }

    fn place_log_block<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, block_pos: BlockPos, sideways_state_modifier: impl Fn(BlockState) -> BlockState) -> BlockPos {
        let state = self.trunk_provider.value().get_state(level, random, block_pos);
        level.set_block_and_update(block_pos, sideways_state_modifier(state));
        mark_above_for_post_processing(level, block_pos);
        block_pos
    }
}

fn is_over_solid_ground(level: &mut WorldGenRegion, block_pos: BlockPos) -> bool {
    level.get_block_state(block_pos.below()).is_face_sturdy_full(Direction::Up)
}

fn decorate_logs<R: RandomSource>(level: &mut WorldGenRegion, random: &mut R, logs: &JavaObjectHashSet<BlockPos>, decorators: &[TreeDecorator]) {
    if decorators.is_empty() {
        return;
    }
    let mut decorations = JavaObjectHashSet::default();
    let empty = JavaObjectHashSet::default();
    let mut context = Context::new(level, TreeSetter::new(&mut decorations), random, logs, &empty, &empty);
    for decorator in decorators {
        decorator.place(&mut context);
    }
}
