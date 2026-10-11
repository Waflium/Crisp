use crisp_data::BlockState;
use serde_json::Value;

use super::blockpredicates::block_predicate::BlockSet;
use super::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use super::parse_block_state;

pub struct GeodeBlockSettings {
    pub filling_provider: BlockStateProviderHolder,
    pub inner_layer_provider: BlockStateProviderHolder,
    pub alternate_inner_layer_provider: BlockStateProviderHolder,
    pub middle_layer_provider: BlockStateProviderHolder,
    pub outer_layer_provider: BlockStateProviderHolder,
    pub inner_placements: Vec<BlockState>,
    pub cannot_replace: BlockSet,
    pub invalid_blocks: BlockSet,
}

impl GeodeBlockSettings {
    pub fn parse(json: &Value) -> Result<GeodeBlockSettings, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let provider = |key: &str| BlockStateProviderHolder::parse(field(key)?);
        Ok(GeodeBlockSettings {
            filling_provider: provider("filling_provider")?,
            inner_layer_provider: provider("inner_layer_provider")?,
            alternate_inner_layer_provider: provider("alternate_inner_layer_provider")?,
            middle_layer_provider: provider("middle_layer_provider")?,
            outer_layer_provider: provider("outer_layer_provider")?,
            inner_placements: field("inner_placements")?.as_array().ok_or("inner_placements is a list")?.iter().map(parse_block_state).collect::<Result<_, _>>()?,
            cannot_replace: BlockSet::parse(field("cannot_replace")?)?,
            invalid_blocks: BlockSet::parse(field("invalid_blocks")?)?,
        })
    }
}
