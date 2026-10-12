use crisp_core::random::RandomSource;
use crisp_core::{AxisDirection, BlockPos, Direction};
use crisp_data::Fluid;
use crisp_data::properties::{PERSISTENT, WATERLOGGED};
use serde_json::Value;

use super::cherry_foliage_placer::{self, CherryFoliagePlacer};
use super::poplar_foliage_placer::{self, PoplarFoliagePlacer};
use super::random_spread_foliage_placer::{self, RandomSpreadFoliagePlacer};
use super::{acacia_foliage_placer, blob_foliage_placer, bush_foliage_placer, dark_oak_foliage_placer, fancy_foliage_placer, mega_jungle_foliage_placer, mega_pine_foliage_placer, pine_foliage_placer, spruce_foliage_placer};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::feature::tree_feature::{self, TreeFeature, TreeSetter};

#[derive(Clone, Copy)]
pub struct FoliageAttachment {
    pub pos: BlockPos,
    pub radius_offset_xz: i32,
    pub foliage_height_offset: i32,
    pub size_x: i32,
    pub size_z: i32,
}

impl FoliageAttachment {
    pub fn new(pos: BlockPos, radius_offset_xz: i32, double_trunk: bool) -> FoliageAttachment {
        let size_xz = if double_trunk { 2 } else { 1 };
        FoliageAttachment { pos, radius_offset_xz, foliage_height_offset: 0, size_x: size_xz, size_z: size_xz }
    }

    pub fn double_trunk(&self) -> bool {
        self.size_x == 2 && self.size_z == 2
    }
}

pub struct FoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub kind: FoliagePlacerType,
}

pub enum FoliagePlacerType {
    Blob { height: i32 },
    Fancy { height: i32 },
    Bush { height: i32 },
    Spruce { trunk_height: IntProvider },
    Pine { height: IntProvider },
    MegaPine { crown_height: IntProvider },
    Acacia,
    DarkOak,
    MegaJungle { height: i32 },
    RandomSpread(RandomSpreadFoliagePlacer),
    Cherry(CherryFoliagePlacer),
    Poplar(PoplarFoliagePlacer),
}

impl FoliagePlacer {
    pub fn parse(json: &Value) -> Result<FoliagePlacer, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("foliage placer without type")?;
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| field(key)?.as_i64().map(|v| v as i32).ok_or_else(|| format!("bad {key}"));
        let kind = match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "blob_foliage_placer" => FoliagePlacerType::Blob { height: int("height")? },
            "fancy_foliage_placer" => FoliagePlacerType::Fancy { height: int("height")? },
            "bush_foliage_placer" => FoliagePlacerType::Bush { height: int("height")? },
            "spruce_foliage_placer" => FoliagePlacerType::Spruce { trunk_height: IntProvider::parse(field("trunk_height")?)? },
            "pine_foliage_placer" => FoliagePlacerType::Pine { height: IntProvider::parse(field("height")?)? },
            "mega_pine_foliage_placer" => FoliagePlacerType::MegaPine { crown_height: IntProvider::parse(field("crown_height")?)? },
            "acacia_foliage_placer" => FoliagePlacerType::Acacia,
            "dark_oak_foliage_placer" => FoliagePlacerType::DarkOak,
            "jungle_foliage_placer" => FoliagePlacerType::MegaJungle { height: int("height")? },
            "random_spread_foliage_placer" => FoliagePlacerType::RandomSpread(RandomSpreadFoliagePlacer::parse(json)?),
            "cherry_foliage_placer" => FoliagePlacerType::Cherry(CherryFoliagePlacer::parse(json)?),
            "poplar_foliage_placer" => FoliagePlacerType::Poplar(PoplarFoliagePlacer::parse(json)?),
            other => return Err(format!("unknown foliage placer type {other}")),
        };
        Ok(FoliagePlacer { radius: IntProvider::parse(field("radius")?)?, offset: IntProvider::parse(field("offset")?)?, kind })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_foliage<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        foliage_setter: &mut TreeSetter,
        random: &mut R,
        tree: &TreeFeature,
        _tree_height: i32,
        foliage_attachment: &FoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
    ) {
        let offset = self.offset.sample(random);
        let attachment = foliage_attachment;
        match &self.kind {
            FoliagePlacerType::Blob { .. } => blob_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::Fancy { .. } => fancy_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::Bush { .. } => bush_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::Spruce { .. } => spruce_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::Pine { .. } => pine_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::MegaPine { .. } => mega_pine_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::Acacia => acacia_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::DarkOak => dark_oak_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, leaf_radius, offset),
            FoliagePlacerType::MegaJungle { .. } => mega_jungle_foliage_placer::create_foliage(self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::RandomSpread(placer) => random_spread_foliage_placer::create_foliage(placer, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius),
            FoliagePlacerType::Cherry(placer) => cherry_foliage_placer::create_foliage(placer, self, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
            FoliagePlacerType::Poplar(placer) => poplar_foliage_placer::create_foliage(placer, level, foliage_setter, random, tree, attachment, foliage_height, leaf_radius, offset),
        }
    }

    pub fn foliage_height<R: RandomSource>(&self, random: &mut R, tree_height: i32, _tree: &TreeFeature) -> i32 {
        match &self.kind {
            FoliagePlacerType::Blob { height } | FoliagePlacerType::Fancy { height } | FoliagePlacerType::Bush { height } | FoliagePlacerType::MegaJungle { height } => *height,
            FoliagePlacerType::Spruce { trunk_height } => 4.max(tree_height - trunk_height.sample(random)),
            FoliagePlacerType::Pine { height } => height.sample(random),
            FoliagePlacerType::MegaPine { crown_height } => crown_height.sample(random),
            FoliagePlacerType::Acacia => 0,
            FoliagePlacerType::DarkOak => 4,
            FoliagePlacerType::RandomSpread(placer) => placer.foliage_height.sample(random),
            FoliagePlacerType::Cherry(placer) => placer.height.sample(random),
            FoliagePlacerType::Poplar(placer) => placer.height.sample(random),
        }
    }

    pub fn foliage_radius<R: RandomSource>(&self, random: &mut R, trunk_height: i32) -> i32 {
        let radius = self.radius.sample(random);
        match self.kind {
            FoliagePlacerType::Pine { .. } => radius + random.next_int_bounded((trunk_height + 1).max(1)),
            _ => radius,
        }
    }

    fn should_skip_location<R: RandomSource>(&self, random: &mut R, dx: i32, y: i32, dz: i32, current_radius: i32, double_trunk: bool) -> bool {
        match &self.kind {
            FoliagePlacerType::Blob { .. } => blob_foliage_placer::should_skip_location(random, dx, y, dz, current_radius),
            FoliagePlacerType::Fancy { .. } => fancy_foliage_placer::should_skip_location(dx, dz, current_radius),
            FoliagePlacerType::Bush { .. } => bush_foliage_placer::should_skip_location(random, dx, dz, current_radius),
            FoliagePlacerType::Spruce { .. } | FoliagePlacerType::Pine { .. } => dx == current_radius && dz == current_radius && current_radius > 0,
            FoliagePlacerType::MegaPine { .. } | FoliagePlacerType::MegaJungle { .. } => {
                if dx + dz >= 7 {
                    true
                } else {
                    dx * dx + dz * dz > current_radius * current_radius
                }
            }
            FoliagePlacerType::Acacia => acacia_foliage_placer::should_skip_location(dx, y, dz, current_radius),
            FoliagePlacerType::DarkOak => dark_oak_foliage_placer::should_skip_location(dx, y, dz, current_radius, double_trunk),
            FoliagePlacerType::RandomSpread(_) => false,
            FoliagePlacerType::Cherry(placer) => placer.should_skip_location(random, dx, y, dz, current_radius),
            FoliagePlacerType::Poplar(_) => panic!("Overridden method needs more context"),
        }
    }

    pub fn should_skip_location_signed<R: RandomSource>(&self, random: &mut R, dx: i32, y: i32, dz: i32, current_radius: i32, double_trunk: bool) -> bool {
        match &self.kind {
            FoliagePlacerType::DarkOak => {
                if y != 0 || !double_trunk || dx != -current_radius && dx < current_radius || dz != -current_radius && dz < current_radius {
                    self.foliage_placer_should_skip_location_signed(random, dx, y, dz, current_radius, double_trunk)
                } else {
                    true
                }
            }
            FoliagePlacerType::Poplar(_) => panic!("Overridden method needs more context"),
            _ => self.foliage_placer_should_skip_location_signed(random, dx, y, dz, current_radius, double_trunk),
        }
    }

    fn foliage_placer_should_skip_location_signed<R: RandomSource>(&self, random: &mut R, dx: i32, y: i32, dz: i32, current_radius: i32, double_trunk: bool) -> bool {
        let (min_dx, min_dz) = if double_trunk { (dx.abs().min((dx - 1).abs()), dz.abs().min((dz - 1).abs())) } else { (dx.abs(), dz.abs()) };
        self.should_skip_location(random, min_dx, y, min_dz, current_radius, double_trunk)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn place_leaves_row<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        foliage_setter: &mut TreeSetter,
        random: &mut R,
        tree: &TreeFeature,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
    ) {
        let offset = if double_trunk { 1 } else { 0 };
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                if !self.should_skip_location_signed(random, dx, y, dz, current_radius, double_trunk) {
                    try_place_leaf(level, foliage_setter, random, tree, origin.offset(dx, y, dz));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn place_leaves_row_with_hanging_leaves_below<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        foliage_setter: &mut TreeSetter,
        random: &mut R,
        tree: &TreeFeature,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        hanging_leaves_chance: f32,
        hanging_leaves_extension_chance: f32,
    ) {
        self.place_leaves_row(level, foliage_setter, random, tree, origin, current_radius, y, double_trunk);
        let offset = if double_trunk { 1 } else { 0 };
        let log_pos = origin.below();
        for along_edge in Direction::PLANE_HORIZONTAL {
            let to_edge = along_edge.clockwise();
            let offset_to_edge = if to_edge.axis_direction() == AxisDirection::Positive { current_radius + offset } else { current_radius };
            let mut pos = origin.offset(0, y - 1, 0).relative_n(to_edge, offset_to_edge).relative_n(along_edge, -current_radius);
            let mut offset_along_edge = -current_radius;
            while offset_along_edge < current_radius + offset {
                let leaves_above = foliage_setter.is_set(pos.above());
                if leaves_above && try_place_extension(level, foliage_setter, random, tree, hanging_leaves_chance, log_pos, pos) {
                    try_place_extension(level, foliage_setter, random, tree, hanging_leaves_extension_chance, log_pos, pos.below());
                }
                offset_along_edge += 1;
                pos = pos.relative(along_edge);
            }
        }
    }
}

fn try_place_extension<R: RandomSource>(
    level: &mut impl WorldGenLevel,
    foliage_setter: &mut TreeSetter,
    random: &mut R,
    tree: &TreeFeature,
    chance: f32,
    log_pos: BlockPos,
    pos: BlockPos,
) -> bool {
    if pos.dist_manhattan(log_pos) >= 7 {
        false
    } else if random.next_float() > chance {
        false
    } else {
        try_place_leaf(level, foliage_setter, random, tree, pos)
    }
}

pub fn try_place_leaf<R: RandomSource>(level: &mut impl WorldGenLevel, foliage_setter: &mut TreeSetter, random: &mut R, tree: &TreeFeature, pos: BlockPos) -> bool {
    let is_persistent = level.get_block_state(pos).try_get(PERSISTENT).unwrap_or(false);
    if !is_persistent && tree_feature::valid_tree_pos(level, pos) {
        let mut foliage_state = tree.foliage_provider.value().get_state(level, random, pos);
        if foliage_state.has(WATERLOGGED) {
            foliage_state = foliage_state.set(WATERLOGGED, level.get_fluid_state(pos).is_source_of_type(Fluid::Water));
        }
        foliage_setter.accept(level, pos, foliage_state);
        true
    } else {
        false
    }
}
