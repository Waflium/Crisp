use std::sync::LazyLock;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::abstract_huge_mushroom_feature::{AbstractHugeMushroomFeature, HugeMushroomType};
use super::bamboo_feature::BambooFeature;
use super::disk_feature::DiskFeature;
use super::iceberg_feature::IcebergFeature;
use super::spike_feature::SpikeFeature;
use super::spring_feature::SpringFeature;
use super::{blue_ice_feature, snow_and_freeze_feature};
use super::block_column_feature::BlockColumnFeature;
use super::block_pile_feature::BlockPileFeature;
use super::fallen_tree_feature::FallenTreeFeature;
use super::multiface_growth_feature::MultifaceGrowthFeature;
use super::ore_feature::OreFeature;
use super::random_boolean_selector_feature::RandomBooleanSelectorFeature;
use super::random_selector_feature::RandomSelectorFeature;
use super::scattered_ore_feature::ScatteredOreFeature;
use super::simple_block_feature::SimpleBlockFeature;
use super::simple_random_selector_feature::SimpleRandomSelectorFeature;
use super::tree_feature::TreeFeature;
use super::vegetation_patch_feature::VegetationPatchFeature;
use super::vines_feature;
use super::weighted_random_selector_feature::WeightedRandomSelectorFeature;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;

pub enum Feature {
    Tree(Box<TreeFeature>),
    FallenTree(FallenTreeFeature),
    SimpleBlock(SimpleBlockFeature),
    BlockColumn(BlockColumnFeature),
    BlockPile(BlockPileFeature),
    VegetationPatch(Box<VegetationPatchFeature>),
    Vines,
    Disk(DiskFeature),
    Spring(SpringFeature),
    FreezeTopLayer,
    Iceberg(IcebergFeature),
    Spike(Box<SpikeFeature>),
    BlueIce,
    MultifaceGrowth(MultifaceGrowthFeature),
    Bamboo(BambooFeature),
    HugeMushroom(Box<AbstractHugeMushroomFeature>),
    Ore(OreFeature),
    ScatteredOre(ScatteredOreFeature),
    RandomSelector(RandomSelectorFeature),
    SimpleRandomSelector(SimpleRandomSelectorFeature),
    RandomBooleanSelector(Box<RandomBooleanSelectorFeature>),
    WeightedRandomSelector(WeightedRandomSelectorFeature),
    NotImplemented,
}

static FEATURES: LazyLock<Vec<Feature>> = LazyLock::new(|| {
    crisp_data::FEATURES
        .iter()
        .map(|(name, json)| {
            let json: Value = serde_json::from_str(json).expect("feature json");
            Feature::parse(&json).unwrap_or_else(|e| panic!("feature {name}: {e}"))
        })
        .collect()
});

pub fn feature_index(name: &str) -> Option<u32> {
    let name = resource(name);
    crisp_data::FEATURES.iter().position(|(n, _)| *n == name).map(|i| i as u32)
}

impl Feature {
    pub fn parse(json: &Value) -> Result<Feature, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("feature without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "tree" => Feature::Tree(Box::new(TreeFeature::parse(json)?)),
            "fallen_tree" => Feature::FallenTree(FallenTreeFeature::parse(json)?),
            "simple_block" => Feature::SimpleBlock(SimpleBlockFeature::parse(json)?),
            "block_column" => Feature::BlockColumn(BlockColumnFeature::parse(json)?),
            "block_pile" => Feature::BlockPile(BlockPileFeature::parse(json)?),
            "vegetation_patch" => Feature::VegetationPatch(Box::new(VegetationPatchFeature::parse(json, false)?)),
            "waterlogged_vegetation_patch" => Feature::VegetationPatch(Box::new(VegetationPatchFeature::parse(json, true)?)),
            "vines" => Feature::Vines,
            "disk" => Feature::Disk(DiskFeature::parse(json)?),
            "spring_feature" => Feature::Spring(SpringFeature::parse(json)?),
            "freeze_top_layer" => Feature::FreezeTopLayer,
            "iceberg" => Feature::Iceberg(IcebergFeature::parse(json)?),
            "spike" => Feature::Spike(Box::new(SpikeFeature::parse(json)?)),
            "blue_ice" => Feature::BlueIce,
            "multiface_growth" => Feature::MultifaceGrowth(MultifaceGrowthFeature::parse(json)?),
            "bamboo" => Feature::Bamboo(BambooFeature::parse(json)?),
            "huge_brown_mushroom" => Feature::HugeMushroom(Box::new(AbstractHugeMushroomFeature::parse(json, HugeMushroomType::Brown)?)),
            "huge_red_mushroom" => Feature::HugeMushroom(Box::new(AbstractHugeMushroomFeature::parse(json, HugeMushroomType::Red)?)),
            "ore" => Feature::Ore(OreFeature::parse(json)?),
            "scattered_ore" => Feature::ScatteredOre(ScatteredOreFeature::parse(json)?),
            "random_selector" => Feature::RandomSelector(RandomSelectorFeature::parse(json)?),
            "simple_random_selector" => Feature::SimpleRandomSelector(SimpleRandomSelectorFeature::parse(json)?),
            "random_boolean_selector" => Feature::RandomBooleanSelector(Box::new(RandomBooleanSelectorFeature::parse(json)?)),
            "weighted_random_selector" => Feature::WeightedRandomSelector(WeightedRandomSelectorFeature::parse(json)?),
            _ => Feature::NotImplemented,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        match self {
            Feature::Tree(feature) => feature.place(level, random, origin),
            Feature::FallenTree(feature) => feature.place(level, random, origin),
            Feature::SimpleBlock(feature) => feature.place(level, random, origin),
            Feature::BlockColumn(feature) => feature.place(level, random, origin),
            Feature::BlockPile(feature) => feature.place(level, random, origin),
            Feature::VegetationPatch(feature) => feature.place(level, random, origin),
            Feature::Vines => vines_feature::place(level, origin),
            Feature::Disk(feature) => feature.place(level, random, origin),
            Feature::Spring(feature) => feature.place(level, origin),
            Feature::FreezeTopLayer => snow_and_freeze_feature::place(level, origin),
            Feature::Iceberg(feature) => feature.place(level, random, origin),
            Feature::Spike(feature) => feature.place(level, random, origin),
            Feature::BlueIce => blue_ice_feature::place(level, random, origin),
            Feature::MultifaceGrowth(feature) => feature.place(level, random, origin),
            Feature::Bamboo(feature) => feature.place(level, random, origin),
            Feature::HugeMushroom(feature) => feature.place(level, random, origin),
            Feature::Ore(feature) => feature.place(level, random, origin),
            Feature::ScatteredOre(feature) => feature.place(level, random, origin),
            Feature::RandomSelector(feature) => feature.place(level, random, origin),
            Feature::SimpleRandomSelector(feature) => feature.place(level, random, origin),
            Feature::RandomBooleanSelector(feature) => feature.place(level, random, origin),
            Feature::WeightedRandomSelector(feature) => feature.place(level, random, origin),
            Feature::NotImplemented => false,
        }
    }
}

pub fn mark_above_for_post_processing(level: &mut WorldGenRegion, place_pos: BlockPos) {
    let mut pos = place_pos;
    for _ in 0..2 {
        pos = pos.above();
        if level.get_block_state(pos).is_air() {
            return;
        }
        level.mark_pos_for_post_processing(pos);
    }
}

pub enum FeatureHolder {
    Reference(u32),
    Direct(Box<Feature>),
}

impl FeatureHolder {
    pub fn parse(json: &Value) -> Result<FeatureHolder, String> {
        match json {
            Value::String(name) => feature_index(name).map(FeatureHolder::Reference).ok_or_else(|| format!("unknown feature {name}")),
            other => Ok(FeatureHolder::Direct(Box::new(Feature::parse(other)?))),
        }
    }

    pub fn value(&self) -> &Feature {
        match self {
            FeatureHolder::Reference(index) => &FEATURES[*index as usize],
            FeatureHolder::Direct(feature) => feature,
        }
    }
}
