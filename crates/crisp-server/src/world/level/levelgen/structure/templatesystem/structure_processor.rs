use std::sync::{LazyLock, OnceLock};

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, mth};
use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;
use serde_json::Value;

use super::pos_rule_test::PosRuleTest;
use super::rule_test::RuleTest;
use super::structure_place_settings::StructurePlaceSettings;
use super::structure_template::StructureBlockInfo;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockSet, resource};
use crate::world::level::levelgen::parse_block_state;

pub enum StructureProcessor {
    BlockRot { rottable_blocks: Option<BlockSet>, integrity: f32 },
    ProtectedBlocks(BlockSet),
    Rule(Vec<ProcessorRule>),
    NotImplemented,
}

pub struct ProcessorRule {
    input_predicate: RuleTest,
    loc_predicate: RuleTest,
    pos_predicate: PosRuleTest,
    output_state: BlockState,
    block_entity_modifier: RuleBlockEntityModifier,
}

enum RuleBlockEntityModifier {
    Passthrough,
    Clear,
    AppendLoot(String),
    AppendStatic(Vec<(String, Nbt)>),
}

fn put(tag: &mut Vec<(String, Nbt)>, key: &str, value: Nbt) {
    match tag.iter_mut().find(|(k, _)| k == key) {
        Some((_, existing)) => *existing = value,
        None => tag.push((key.to_owned(), value)),
    }
}

impl RuleBlockEntityModifier {
    fn parse(json: Option<&Value>) -> Result<RuleBlockEntityModifier, String> {
        let Some(json) = json else { return Ok(RuleBlockEntityModifier::Passthrough) };
        let ty = json.get("type").and_then(Value::as_str).ok_or("block entity modifier without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "clear" => RuleBlockEntityModifier::Clear,
            "passthrough" => RuleBlockEntityModifier::Passthrough,
            "append_loot" => RuleBlockEntityModifier::AppendLoot(resource(json.get("loot_table").and_then(Value::as_str).ok_or("missing loot_table")?)),
            "append_static" => match json.get("data").and_then(crate::nbt::from_json) {
                Some(Nbt::Compound(tag)) => RuleBlockEntityModifier::AppendStatic(tag),
                _ => return Err("append_static without compound data".into()),
            },
            other => return Err(format!("unknown block entity modifier {other}")),
        })
    }

    fn apply(&self, random: &mut impl RandomSource, existing_tag: Option<Nbt>) -> Option<Nbt> {
        match self {
            RuleBlockEntityModifier::Passthrough => existing_tag,
            RuleBlockEntityModifier::Clear => Some(Nbt::Compound(Vec::new())),
            RuleBlockEntityModifier::AppendLoot(loot_table) => {
                let mut result = match existing_tag {
                    Some(Nbt::Compound(tag)) => tag,
                    _ => Vec::new(),
                };
                put(&mut result, "LootTable", Nbt::String(loot_table.clone()));
                put(&mut result, "LootTableSeed", Nbt::Long(random.next_long()));
                Some(Nbt::Compound(result))
            }
            RuleBlockEntityModifier::AppendStatic(tag) => match existing_tag {
                Some(Nbt::Compound(mut existing)) => {
                    crate::nbt::merge(&mut existing, tag);
                    Some(Nbt::Compound(existing))
                }
                _ => Some(Nbt::Compound(tag.clone())),
            },
        }
    }
}

impl ProcessorRule {
    fn parse(json: &Value) -> Result<ProcessorRule, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(ProcessorRule {
            input_predicate: RuleTest::parse(field("input_predicate")?)?,
            loc_predicate: RuleTest::parse(field("location_predicate")?)?,
            pos_predicate: json.get("position_predicate").map(PosRuleTest::parse).transpose()?.unwrap_or(PosRuleTest::AlwaysTrue),
            output_state: parse_block_state(field("output_state")?)?,
            block_entity_modifier: RuleBlockEntityModifier::parse(json.get("block_entity_modifier"))?,
        })
    }

    fn test(
        &self,
        level: &mut WorldGenRegion,
        input_state: BlockState,
        in_template_pos: BlockPos,
        world_pos: BlockPos,
        reference: BlockPos,
        random: &mut impl RandomSource,
    ) -> bool {
        self.input_predicate.test(input_state, world_pos, random)
            && self.loc_predicate.test(level.get_block_state(world_pos), world_pos, random)
            && self.pos_predicate.test(in_template_pos, world_pos, reference, random)
    }
}

impl StructureProcessor {
    pub fn parse(json: &Value) -> Result<StructureProcessor, String> {
        let ty = json.get("processor_type").and_then(Value::as_str).ok_or("processor without processor_type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "block_rot" => StructureProcessor::BlockRot {
                rottable_blocks: json.get("rottable_blocks").map(BlockSet::parse).transpose()?,
                integrity: float(json.get("integrity").ok_or("missing integrity")?)?,
            },
            "protected_blocks" => StructureProcessor::ProtectedBlocks(BlockSet::parse(json.get("value").ok_or("missing value")?)?),
            "rule" => StructureProcessor::Rule(json.get("rules").and_then(Value::as_array).ok_or("missing rules")?.iter().map(ProcessorRule::parse).collect::<Result<_, _>>()?),
            _ => StructureProcessor::NotImplemented,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_block<R: RandomSource>(
        &self,
        level: &mut WorldGenRegion,
        _target_position: BlockPos,
        reference_pos: BlockPos,
        template_relative_pos: BlockPos,
        processed_block_info: StructureBlockInfo,
        settings: &StructurePlaceSettings,
        random: &mut R,
    ) -> Option<StructureBlockInfo> {
        match self {
            StructureProcessor::BlockRot { rottable_blocks, integrity } => {
                let mut random = settings.get_random(processed_block_info.pos, random);
                let rottable = rottable_blocks.as_ref().is_none_or(|blocks| blocks.contains(processed_block_info.state.block()));
                if rottable && random.next_float() > *integrity { None } else { Some(processed_block_info) }
            }
            StructureProcessor::ProtectedBlocks(cannot_replace) => {
                if cannot_replace.contains(level.get_block_state(processed_block_info.pos).block()) { None } else { Some(processed_block_info) }
            }
            StructureProcessor::Rule(rules) => {
                let pos = processed_block_info.pos;
                let mut random = LegacyRandomSource::new(mth::get_seed(pos.x, pos.y, pos.z));
                for rule in rules {
                    if rule.test(level, processed_block_info.state, template_relative_pos, pos, reference_pos, &mut random) {
                        let nbt = rule.block_entity_modifier.apply(&mut random, processed_block_info.nbt);
                        return Some(StructureBlockInfo { pos, state: rule.output_state, nbt });
                    }
                }
                Some(processed_block_info)
            }
            StructureProcessor::NotImplemented => Some(processed_block_info),
        }
    }
}

pub struct StructureProcessorList {
    pub list: Vec<StructureProcessor>,
}

impl StructureProcessorList {
    fn parse(json: &Value) -> Result<StructureProcessorList, String> {
        let processors = json.get("processors").and_then(Value::as_array).ok_or("processor list without processors")?;
        Ok(StructureProcessorList { list: processors.iter().map(StructureProcessor::parse).collect::<Result<_, _>>()? })
    }
}

static PROCESSOR_LISTS: LazyLock<Vec<OnceLock<StructureProcessorList>>> = LazyLock::new(|| (0..crisp_data::PROCESSOR_LISTS.len()).map(|_| OnceLock::new()).collect());

pub enum StructureProcessorListHolder {
    Reference(usize),
    Direct(StructureProcessorList),
}

impl StructureProcessorListHolder {
    pub fn parse(json: &Value) -> Result<StructureProcessorListHolder, String> {
        match json {
            Value::String(name) => {
                let name = resource(name);
                crisp_data::PROCESSOR_LISTS
                    .iter()
                    .position(|(n, _)| *n == name)
                    .map(StructureProcessorListHolder::Reference)
                    .ok_or_else(|| format!("unknown processor list {name}"))
            }
            Value::Array(list) => Ok(StructureProcessorListHolder::Direct(StructureProcessorList { list: list.iter().map(StructureProcessor::parse).collect::<Result<_, _>>()? })),
            other => Ok(StructureProcessorListHolder::Direct(StructureProcessorList::parse(other)?)),
        }
    }

    pub fn value(&self) -> &StructureProcessorList {
        match self {
            StructureProcessorListHolder::Reference(index) => PROCESSOR_LISTS[*index].get_or_init(|| {
                let (name, source) = crisp_data::PROCESSOR_LISTS[*index];
                let json: Value = serde_json::from_str(source).expect("processor list json");
                StructureProcessorList::parse(&json).unwrap_or_else(|e| panic!("processor list {name}: {e}"))
            }),
            StructureProcessorListHolder::Direct(list) => list,
        }
    }
}
