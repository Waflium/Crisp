use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{Block, BlockState, Tag, block_tags};
use serde_json::Value;

use crate::util::valueproviders::float;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::parse_block_state;

pub enum RuleTest {
    AlwaysTrue,
    BlockMatch(Block),
    BlockStateMatch(BlockState),
    TagMatch(&'static Tag),
    RandomBlockMatch { block: Block, probability: f32 },
    RandomBlockStateMatch { block_state: BlockState, probability: f32 },
    HeightMatch { min_inclusive: i32, max_inclusive: i32 },
    AllOf(Vec<RuleTest>),
    AnyOf(Vec<RuleTest>),
    Not(Box<RuleTest>),
}

impl RuleTest {
    pub fn parse(json: &Value) -> Result<RuleTest, String> {
        let ty = json.get("predicate_type").and_then(Value::as_str).ok_or("rule test without predicate_type")?;
        let block = |key: &str| -> Result<Block, String> {
            let name = json.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing {key}"))?;
            Block::by_name(&resource(name)).ok_or_else(|| format!("unknown block {name}"))
        };
        let state = || parse_block_state(json.get("block_state").ok_or("missing block_state")?);
        let probability = || float(json.get("probability").ok_or("missing probability")?);
        let int = |key: &str| json.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("missing {key}"));
        let rules = || -> Result<Vec<RuleTest>, String> { json.get("rules").and_then(Value::as_array).ok_or("missing rules")?.iter().map(RuleTest::parse).collect() };
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "always_true" => RuleTest::AlwaysTrue,
            "block_match" => RuleTest::BlockMatch(block("block")?),
            "blockstate_match" => RuleTest::BlockStateMatch(state()?),
            "tag_match" => {
                let tag = json.get("tag").and_then(Value::as_str).ok_or("missing tag")?;
                RuleTest::TagMatch(block_tags::by_name(&resource(tag)).ok_or_else(|| format!("unknown block tag {tag}"))?)
            }
            "random_block_match" => RuleTest::RandomBlockMatch { block: block("block")?, probability: probability()? },
            "random_blockstate_match" => RuleTest::RandomBlockStateMatch { block_state: state()?, probability: probability()? },
            "height_match" => RuleTest::HeightMatch { min_inclusive: int("min_inclusive")?, max_inclusive: int("max_inclusive")? },
            "all_of" => RuleTest::AllOf(rules()?),
            "any_of" => RuleTest::AnyOf(rules()?),
            "not" => RuleTest::Not(Box::new(RuleTest::parse(json.get("rule").ok_or("missing rule")?)?)),
            other => return Err(format!("unknown rule test {other}")),
        })
    }

    pub fn test(&self, block_state: BlockState, pos: BlockPos, random: &mut impl RandomSource) -> bool {
        match self {
            RuleTest::AlwaysTrue => true,
            RuleTest::BlockMatch(block) => block_state.is_block(*block),
            RuleTest::BlockStateMatch(state) => block_state == *state,
            RuleTest::TagMatch(tag) => block_state.is(tag),
            RuleTest::RandomBlockMatch { block, probability } => block_state.is_block(*block) && random.next_float() < *probability,
            RuleTest::RandomBlockStateMatch { block_state: state, probability } => block_state == *state && random.next_float() < *probability,
            RuleTest::HeightMatch { min_inclusive, max_inclusive } => *min_inclusive <= pos.y && pos.y <= *max_inclusive,
            RuleTest::AllOf(rules) => rules.iter().all(|rule| rule.test(block_state, pos, random)),
            RuleTest::AnyOf(rules) => rules.iter().any(|rule| rule.test(block_state, pos, random)),
            RuleTest::Not(rule) => !rule.test(block_state, pos, random),
        }
    }
}
