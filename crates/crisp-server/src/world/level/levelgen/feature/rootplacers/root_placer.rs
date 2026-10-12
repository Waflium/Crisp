use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::WATERLOGGED;
use crisp_data::{BlockState, fluid_tags};
use serde_json::Value;

use super::above_root_placement::AboveRootPlacement;
use super::mangrove_root_placer::{self, MangroveRootPlacement};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::levelgen::feature::tree_feature::{self, TreeFeature, TreeSetter};

pub struct RootPlacer {
    trunk_offset_y: IntProvider,
    root_provider: BlockStateProviderHolder,
    above_root_placement: Option<AboveRootPlacement>,
    kind: RootPlacerType,
}

pub enum RootPlacerType {
    Mangrove(MangroveRootPlacement),
}

impl RootPlacer {
    pub fn parse(json: &Value) -> Result<RootPlacer, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("root placer without type")?;
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let kind = match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "mangrove_root_placer" => RootPlacerType::Mangrove(MangroveRootPlacement::parse(field("mangrove_root_placement")?)?),
            other => return Err(format!("unknown root placer type {other}")),
        };
        Ok(RootPlacer {
            trunk_offset_y: IntProvider::parse(field("trunk_offset_y")?)?,
            root_provider: BlockStateProviderHolder::parse(field("root_provider")?)?,
            above_root_placement: json.get("above_root_placement").map(AboveRootPlacement::parse).transpose()?,
            kind,
        })
    }

    pub fn place_roots<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        root_setter: &mut TreeSetter,
        random: &mut R,
        origin: BlockPos,
        trunk_origin: BlockPos,
        tree: &TreeFeature,
    ) -> bool {
        match &self.kind {
            RootPlacerType::Mangrove(placement) => mangrove_root_placer::place_roots(placement, self, level, root_setter, random, origin, trunk_origin, tree),
        }
    }

    pub fn can_place_root(&self, level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
        match &self.kind {
            RootPlacerType::Mangrove(placement) => tree_feature::valid_tree_pos(level, pos) || placement.can_grow_through.contains(level.get_block_state(pos).block()),
        }
    }

    pub fn place_root<R: RandomSource>(&self, level: &mut impl WorldGenLevel, root_setter: &mut TreeSetter, random: &mut R, pos: BlockPos, tree: &TreeFeature) {
        match &self.kind {
            RootPlacerType::Mangrove(placement) => mangrove_root_placer::place_root(placement, self, level, root_setter, random, pos, tree),
        }
    }

    pub fn root_placer_place_root<R: RandomSource>(&self, level: &mut impl WorldGenLevel, root_setter: &mut TreeSetter, random: &mut R, pos: BlockPos) {
        if self.can_place_root(level, pos) {
            let state = self.root_provider.value().get_state(level, random, pos);
            let state = get_potentially_waterlogged_state(level, pos, state);
            root_setter.accept(level, pos, state);
            if let Some(above_placement) = &self.above_root_placement {
                let above = pos.above();
                if random.next_float() < above_placement.above_root_placement_chance && level.get_block_state(above).is_air() {
                    let state = above_placement.above_root_provider.value().get_state(level, random, above);
                    let state = get_potentially_waterlogged_state(level, above, state);
                    root_setter.accept(level, above, state);
                }
            }
        }
    }

    pub fn get_trunk_origin(&self, origin: BlockPos, random: &mut impl RandomSource) -> BlockPos {
        origin.offset(0, self.trunk_offset_y.sample(random), 0)
    }
}

pub fn get_potentially_waterlogged_state(level: &mut impl WorldGenLevel, pos: BlockPos, state: BlockState) -> BlockState {
    if state.has(WATERLOGGED) {
        let waterlogged = level.get_fluid_state(pos).is(&fluid_tags::WATER);
        state.set(WATERLOGGED, waterlogged)
    } else {
        state
    }
}
