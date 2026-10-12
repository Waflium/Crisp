use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, Fluid, SupportType, Tag, block_tags, fluid_tags};
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block;
use crate::world::level::levelgen::material::{VerticalAnchor, WorldGenerationContext};
use crate::world::level::levelgen::parse_block_state;

pub enum BlockSet {
    Tag(&'static Tag),
    Direct(Vec<Block>),
}

impl BlockSet {
    pub fn parse(json: &Value) -> Result<BlockSet, String> {
        match json {
            Value::String(s) => match s.strip_prefix('#') {
                Some(tag) => block_tags::by_name(&resource(tag)).map(BlockSet::Tag).ok_or_else(|| format!("unknown block tag {tag}")),
                None => Ok(BlockSet::Direct(vec![Block::by_name(&resource(s)).ok_or_else(|| format!("unknown block {s}"))?])),
            },
            Value::Array(list) => list
                .iter()
                .map(|v| v.as_str().and_then(|s| Block::by_name(&resource(s))).ok_or_else(|| format!("unknown block {v}")))
                .collect::<Result<_, _>>()
                .map(BlockSet::Direct),
            other => Err(format!("expected blocks, got {other}")),
        }
    }

    pub fn contains(&self, block: Block) -> bool {
        match self {
            BlockSet::Tag(tag) => tag.contains(block.0 as usize),
            BlockSet::Direct(blocks) => blocks.contains(&block),
        }
    }
}

pub enum FluidSet {
    Tag(&'static Tag),
    Direct(Vec<Fluid>),
}

impl FluidSet {
    fn parse(json: &Value) -> Result<FluidSet, String> {
        let fluid = |s: &str| match resource(s).as_str() {
            "minecraft:empty" => Ok(Fluid::Empty),
            "minecraft:flowing_water" => Ok(Fluid::FlowingWater),
            "minecraft:water" => Ok(Fluid::Water),
            "minecraft:flowing_lava" => Ok(Fluid::FlowingLava),
            "minecraft:lava" => Ok(Fluid::Lava),
            _ => Err(format!("unknown fluid {s}")),
        };
        match json {
            Value::String(s) => match s.strip_prefix('#') {
                Some(tag) => fluid_tags::by_name(&resource(tag)).map(FluidSet::Tag).ok_or_else(|| format!("unknown fluid tag {tag}")),
                None => Ok(FluidSet::Direct(vec![fluid(s)?])),
            },
            Value::Array(list) => list.iter().map(|v| fluid(v.as_str().unwrap_or_default())).collect::<Result<_, _>>().map(FluidSet::Direct),
            other => Err(format!("expected fluids, got {other}")),
        }
    }

    fn contains(&self, fluid: Fluid) -> bool {
        match self {
            FluidSet::Tag(tag) => tag.contains(fluid as usize),
            FluidSet::Direct(fluids) => fluids.contains(&fluid),
        }
    }
}

pub fn resource(name: &str) -> String {
    if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") }
}

pub fn parse_offset(json: &Value) -> Result<(i32, i32, i32), String> {
    match json.get("offset").and_then(Value::as_array) {
        Some(v) if v.len() == 3 => Ok((v[0].as_i64().unwrap_or(0) as i32, v[1].as_i64().unwrap_or(0) as i32, v[2].as_i64().unwrap_or(0) as i32)),
        Some(_) => Err("offset needs three values".into()),
        None => Ok((0, 0, 0)),
    }
}

pub enum BlockPredicate {
    MatchingBlocks { offset: (i32, i32, i32), blocks: BlockSet },
    MatchingBlockTag { offset: (i32, i32, i32), tag: &'static Tag },
    MatchingFluids { offset: (i32, i32, i32), fluids: FluidSet },
    Solid { offset: (i32, i32, i32) },
    Replaceable { offset: (i32, i32, i32) },
    WouldSurvive { offset: (i32, i32, i32), state: BlockState },
    HasSturdyFace { offset: (i32, i32, i32), direction: Direction },
    InsideWorldBounds { offset: (i32, i32, i32) },
    HeightRange { min_inclusive: VerticalAnchor, max_inclusive: VerticalAnchor },
    VolumeMatch { min: (i32, i32, i32), max: (i32, i32, i32), predicate: Box<BlockPredicate> },
    AllOf(Vec<BlockPredicate>),
    AnyOf(Vec<BlockPredicate>),
    Not(Box<BlockPredicate>),
    True,
}

impl BlockPredicate {
    pub fn parse(json: &Value) -> Result<BlockPredicate, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("block predicate without type")?;
        let list = |key: &str| -> Result<Vec<BlockPredicate>, String> {
            json.get(key).and_then(Value::as_array).ok_or_else(|| format!("missing {key}"))?.iter().map(BlockPredicate::parse).collect()
        };
        let vec3 = |key: &str| -> Result<(i32, i32, i32), String> {
            let v = json.get(key).and_then(Value::as_array).filter(|v| v.len() == 3).ok_or_else(|| format!("missing {key}"))?;
            Ok((v[0].as_i64().unwrap_or(0) as i32, v[1].as_i64().unwrap_or(0) as i32, v[2].as_i64().unwrap_or(0) as i32))
        };
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "matching_blocks" => BlockPredicate::MatchingBlocks { offset: parse_offset(json)?, blocks: BlockSet::parse(json.get("blocks").ok_or("missing blocks")?)? },
            "matching_block_tag" => {
                let tag = json.get("tag").and_then(Value::as_str).ok_or("missing tag")?;
                BlockPredicate::MatchingBlockTag { offset: parse_offset(json)?, tag: block_tags::by_name(&resource(tag)).ok_or_else(|| format!("unknown block tag {tag}"))? }
            }
            "matching_fluids" => BlockPredicate::MatchingFluids { offset: parse_offset(json)?, fluids: FluidSet::parse(json.get("fluids").ok_or("missing fluids")?)? },
            "solid" => BlockPredicate::Solid { offset: parse_offset(json)? },
            "replaceable" => BlockPredicate::Replaceable { offset: parse_offset(json)? },
            "would_survive" => BlockPredicate::WouldSurvive { offset: parse_offset(json)?, state: parse_block_state(json.get("state").ok_or("missing state")?)? },
            "has_sturdy_face" => BlockPredicate::HasSturdyFace {
                offset: parse_offset(json)?,
                direction: json.get("direction").and_then(Value::as_str).and_then(Direction::by_name).ok_or("missing direction")?,
            },
            "inside_world_bounds" => BlockPredicate::InsideWorldBounds { offset: parse_offset(json)? },
            "height_range" => BlockPredicate::HeightRange {
                min_inclusive: VerticalAnchor::parse(json.get("min_inclusive").ok_or("missing min_inclusive")?)?,
                max_inclusive: VerticalAnchor::parse(json.get("max_inclusive").ok_or("missing max_inclusive")?)?,
            },
            "volume_match" => BlockPredicate::VolumeMatch {
                min: vec3("min")?,
                max: vec3("max")?,
                predicate: Box::new(BlockPredicate::parse(json.get("match").ok_or("missing match")?)?),
            },
            "all_of" => BlockPredicate::AllOf(list("predicates")?),
            "any_of" => BlockPredicate::AnyOf(list("predicates")?),
            "not" => BlockPredicate::Not(Box::new(BlockPredicate::parse(json.get("predicate").ok_or("missing predicate")?)?)),
            "true" => BlockPredicate::True,
            other => return Err(format!("unknown block predicate {other}")),
        })
    }

    pub fn test(&self, level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
        let at = |(x, y, z): (i32, i32, i32)| origin.offset(x, y, z);
        match self {
            BlockPredicate::MatchingBlocks { offset, blocks } => blocks.contains(level.get_block_state(at(*offset)).block()),
            BlockPredicate::MatchingBlockTag { offset, tag } => level.get_block_state(at(*offset)).is(tag),
            BlockPredicate::MatchingFluids { offset, fluids } => fluids.contains(level.get_block_state(at(*offset)).fluid_state().fluid),
            BlockPredicate::Solid { offset } => level.get_block_state(at(*offset)).is_solid(),
            BlockPredicate::Replaceable { offset } => level.get_block_state(at(*offset)).can_be_replaced(),
            BlockPredicate::WouldSurvive { offset, state } => block::of(*state).can_survive(*state, level, at(*offset)),
            BlockPredicate::HasSturdyFace { offset, direction } => level.get_block_state(at(*offset)).is_face_sturdy(*direction, SupportType::Full),
            BlockPredicate::InsideWorldBounds { offset } => !level.is_outside_build_height(at(*offset).y),
            BlockPredicate::HeightRange { min_inclusive, max_inclusive } => {
                let context = WorldGenerationContext { min_gen_y: level.get_min_y(), gen_depth: level.get_height_of_level(), sea_level: level.get_sea_level() };
                let (min, max) = (min_inclusive.resolve_y(&context), max_inclusive.resolve_y(&context));
                origin.y >= min && origin.y <= max
            }
            BlockPredicate::VolumeMatch { min, max, predicate } => {
                for ox in min.0..=max.0 {
                    for oz in min.2..=max.2 {
                        for oy in min.1..=max.1 {
                            if !predicate.test(level, origin.offset(ox, oy, oz)) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
            BlockPredicate::AllOf(predicates) => predicates.iter().all(|p| p.test(level, origin)),
            BlockPredicate::AnyOf(predicates) => predicates.iter().any(|p| p.test(level, origin)),
            BlockPredicate::Not(predicate) => !predicate.test(level, origin),
            BlockPredicate::True => true,
        }
    }
}
