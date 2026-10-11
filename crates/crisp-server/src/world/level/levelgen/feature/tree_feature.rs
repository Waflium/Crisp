use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::DISTANCE;
use crisp_data::{BlockState, block_tags, blocks};
use serde_json::Value;

use super::featuresize::feature_size::FeatureSize;
use super::foliageplacers::foliage_placer::FoliagePlacer;
use super::rootplacers::root_placer::RootPlacer;
use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use super::treedecorators::tree_decorator::{self, TreeDecorator};
use super::trunkplacers::trunk_placer::TrunkPlacer;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::java_collections::JavaObjectHashSet;
use crate::world::level::block::leaves_block;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_template;
use crisp_core::shapes::BitSetDiscreteVoxelShape;

const BLOCK_UPDATE_FLAGS: i32 = 19;

pub struct TreeSetter<'s> {
    positions: &'s mut JavaObjectHashSet<BlockPos>,
}

impl<'s> TreeSetter<'s> {
    pub fn new(positions: &'s mut JavaObjectHashSet<BlockPos>) -> TreeSetter<'s> {
        TreeSetter { positions }
    }

    pub fn accept(&mut self, level: &mut WorldGenRegion, pos: BlockPos, state: BlockState) {
        self.positions.insert(pos);
        level.set_block(pos, state, BLOCK_UPDATE_FLAGS);
    }

    pub fn is_set(&self, pos: BlockPos) -> bool {
        self.positions.contains(&pos)
    }
}

pub struct TreeFeature {
    pub trunk_provider: BlockStateProviderHolder,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: BlockStateProviderHolder,
    pub foliage_placer: FoliagePlacer,
    pub root_placer: Option<RootPlacer>,
    pub minimum_size: FeatureSize,
    pub decorators: Vec<TreeDecorator>,
    pub ignore_vines: bool,
    pub below_trunk_provider: BlockStateProviderHolder,
}

pub fn is_vine(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    level.get_block_state(pos).is_block(blocks::VINE)
}

pub fn is_air_or_leaves(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    let state = level.get_block_state(pos);
    state.is_air() || state.is(&block_tags::LEAVES)
}

fn set_block_known_shape(level: &mut WorldGenRegion, pos: BlockPos, state: BlockState) {
    level.set_block(pos, state, BLOCK_UPDATE_FLAGS);
}

pub fn valid_tree_pos(level: &mut WorldGenRegion, pos: BlockPos) -> bool {
    let state = level.get_block_state(pos);
    state.is_air() || state.is(&block_tags::REPLACEABLE_BY_TREES)
}

impl TreeFeature {
    pub fn parse(json: &Value) -> Result<TreeFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(TreeFeature {
            trunk_provider: BlockStateProviderHolder::parse(field("trunk_provider")?)?,
            trunk_placer: TrunkPlacer::parse(field("trunk_placer")?)?,
            foliage_provider: BlockStateProviderHolder::parse(field("foliage_provider")?)?,
            foliage_placer: FoliagePlacer::parse(field("foliage_placer")?)?,
            root_placer: json.get("root_placer").map(RootPlacer::parse).transpose()?,
            minimum_size: FeatureSize::parse(field("minimum_size")?)?,
            decorators: field("decorators")?.as_array().ok_or("decorators is a list")?.iter().map(TreeDecorator::parse).collect::<Result<_, _>>()?,
            ignore_vines: json.get("ignore_vines").and_then(Value::as_bool).unwrap_or(false),
            below_trunk_provider: BlockStateProviderHolder::parse(field("below_trunk_provider")?)?,
        })
    }

    fn do_place<R: RandomSource>(
        &self,
        level: &mut WorldGenRegion,
        random: &mut R,
        origin: BlockPos,
        root_setter: &mut TreeSetter,
        trunk_setter: &mut TreeSetter,
        foliage_setter: &mut TreeSetter,
    ) -> bool {
        let tree_height = self.trunk_placer.get_tree_height(random);
        let foliage_height = self.foliage_placer.foliage_height(random, tree_height, self);
        let trunk_height = tree_height - foliage_height;
        let leaf_radius = self.foliage_placer.foliage_radius(random, trunk_height);
        let trunk_origin = match &self.root_placer {
            Some(root_placer) => root_placer.get_trunk_origin(origin, random),
            None => origin,
        };
        let min_y = origin.y.min(trunk_origin.y);
        let max_y = origin.y.max(trunk_origin.y) + tree_height + 1;
        if min_y < level.get_min_y() + 1 || max_y > level.get_max_y() + 1 {
            return false;
        }
        let min_clipped_height = self.minimum_size.min_clipped_height();
        let clipped_tree_height = self.get_max_free_tree_height(level, tree_height, trunk_origin);
        if clipped_tree_height < tree_height && min_clipped_height.is_none_or(|min| clipped_tree_height < min) {
            return false;
        }
        if let Some(root_placer) = &self.root_placer
            && !root_placer.place_roots(level, root_setter, random, origin, trunk_origin, self)
        {
            return false;
        }
        let foliage_attachments = self.trunk_placer.place_trunk(level, trunk_setter, random, clipped_tree_height, trunk_origin, self);
        for foliage_attachment in &foliage_attachments {
            self.foliage_placer.create_foliage(level, foliage_setter, random, self, clipped_tree_height, foliage_attachment, foliage_height, leaf_radius);
        }
        true
    }

    fn get_max_free_tree_height(&self, level: &mut WorldGenRegion, max_tree_height: i32, tree_pos: BlockPos) -> i32 {
        for y in 0..=max_tree_height + 1 {
            let r = self.minimum_size.get_size_at_height(max_tree_height, y);
            for x in -r..=r {
                for z in -r..=r {
                    let block_pos = tree_pos.offset(x, y, z);
                    if !self.trunk_placer.is_free(level, block_pos) || !self.ignore_vines && is_vine(level, block_pos) {
                        return y - 2;
                    }
                }
            }
        }
        max_tree_height
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let mut root_positions = JavaObjectHashSet::default();
        let mut trunks = JavaObjectHashSet::default();
        let mut foliage = JavaObjectHashSet::default();
        let mut decorations = JavaObjectHashSet::default();
        let result = self.do_place(
            level,
            random,
            origin,
            &mut TreeSetter::new(&mut root_positions),
            &mut TreeSetter::new(&mut trunks),
            &mut TreeSetter::new(&mut foliage),
        );
        if !result || trunks.is_empty() && foliage.is_empty() {
            return false;
        }
        if !self.decorators.is_empty() {
            let mut context = tree_decorator::Context::new(level, TreeSetter::new(&mut decorations), random, &trunks, &foliage, &root_positions);
            for decorator in &self.decorators {
                decorator.place(&mut context);
            }
        }
        let Some(bounds) = encapsulating_positions(root_positions.iter().chain(trunks.iter()).chain(foliage.iter()).chain(decorations.iter())) else {
            return false;
        };
        let shape = update_leaves(level, &bounds, &trunks, &decorations, &root_positions);
        structure_template::update_shape_at_edge(level, 3, &shape, bounds.min.x, bounds.min.y, bounds.min.z);
        true
    }
}

fn encapsulating_positions(mut positions: impl Iterator<Item = BlockPos>) -> Option<BoundingBox> {
    let first = positions.next()?;
    let mut result = BoundingBox { min: first, max: first };
    for pos in positions {
        result.min = BlockPos::new(result.min.x.min(pos.x), result.min.y.min(pos.y), result.min.z.min(pos.z));
        result.max = BlockPos::new(result.max.x.max(pos.x), result.max.y.max(pos.y), result.max.z.max(pos.z));
    }
    Some(result)
}

fn update_leaves(
    level: &mut WorldGenRegion,
    bounds: &BoundingBox,
    logs: &JavaObjectHashSet<BlockPos>,
    decoration_set: &JavaObjectHashSet<BlockPos>,
    root_positions: &JavaObjectHashSet<BlockPos>,
) -> BitSetDiscreteVoxelShape {
    let mut shape = BitSetDiscreteVoxelShape::new(bounds.x_span(), bounds.y_span(), bounds.z_span());
    let mut to_check: Vec<JavaObjectHashSet<BlockPos>> = (0..7).map(|_| JavaObjectHashSet::default()).collect();
    for pos in decoration_set.iter().chain(root_positions.iter()) {
        if bounds.is_inside(pos) {
            shape.fill(pos.x - bounds.min.x, pos.y - bounds.min.y, pos.z - bounds.min.z);
        }
    }
    let mut smallest_distance = 0;
    for pos in logs.iter() {
        to_check[0].insert(pos);
    }
    loop {
        while smallest_distance >= 7 || !to_check[smallest_distance].is_empty() {
            if smallest_distance >= 7 {
                return shape;
            }
            let pos = to_check[smallest_distance].first().expect("a position to check");
            to_check[smallest_distance].remove(&pos);
            if bounds.is_inside(pos) {
                if smallest_distance != 0 {
                    let state = level.get_block_state(pos);
                    set_block_known_shape(level, pos, state.set(DISTANCE, smallest_distance as i32));
                }
                shape.fill(pos.x - bounds.min.x, pos.y - bounds.min.y, pos.z - bounds.min.z);
                for direction in Direction::VALUES {
                    let neighbor_pos = pos.relative(direction);
                    if bounds.is_inside(neighbor_pos) {
                        let x_in_shape = neighbor_pos.x - bounds.min.x;
                        let y_in_shape = neighbor_pos.y - bounds.min.y;
                        let z_in_shape = neighbor_pos.z - bounds.min.z;
                        if !shape.is_full(x_in_shape, y_in_shape, z_in_shape) {
                            let current_state = level.get_block_state(neighbor_pos);
                            if let Some(distance) = leaves_block::optional_distance_at(current_state) {
                                let new_distance = distance.min(smallest_distance as i32 + 1);
                                if new_distance < 7 {
                                    to_check[new_distance as usize].insert(neighbor_pos);
                                    smallest_distance = smallest_distance.min(new_distance as usize);
                                }
                            }
                        }
                    }
                }
            }
        }
        smallest_distance += 1;
    }
}

pub fn get_lowest_trunk_or_root_of_tree(roots: &[BlockPos], logs: &[BlockPos]) -> Vec<BlockPos> {
    let mut block_positions = Vec::new();
    if roots.is_empty() {
        block_positions.extend_from_slice(logs);
    } else if !logs.is_empty() && roots[0].y == logs[0].y {
        block_positions.extend_from_slice(logs);
        block_positions.extend_from_slice(roots);
    } else {
        block_positions.extend_from_slice(roots);
    }
    block_positions
}
