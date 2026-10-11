pub mod block_state_parser;

use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;

use self::block_state_parser::{ParseResult, property_value};
use crate::commands::brigadier::{CommandSyntaxException, StringReader};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_KNOWN_SHAPE;

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInput {
    pub state: BlockState,
    pub properties: Vec<&'static str>,
    pub tag: Option<Nbt>,
}

impl BlockInput {
    pub fn parse(reader: &mut StringReader) -> Result<BlockInput, CommandSyntaxException> {
        let result = block_state_parser::parse_for_block(reader, true)?;
        Ok(BlockInput { state: result.state, properties: result.properties, tag: result.nbt })
    }

    pub fn place(&self, level: &mut ServerLevel, pos: BlockPos, flags: i32) -> bool {
        let mut state = if flags & UPDATE_KNOWN_SHAPE != 0 { self.state } else { level.update_from_neighbour_shapes(self.state, pos) };
        if state.is_air() {
            state = self.state;
        }
        state = self.overwrite_with_defined_properties(state);
        let mut affected = level.set_block(pos, state, flags);
        if let Some(tag) = &self.tag
            && let Some(entity) = level.get_block_entity(pos)
        {
            let before = entity.save_without_metadata();
            entity.load_with_components(tag);
            if entity.save_without_metadata() != before {
                affected = true;
                level.block_entity_set_changed(pos);
                let state = level.block_state(pos);
                level.send_block_updated(pos, state, state);
            }
        }
        affected
    }

    fn overwrite_with_defined_properties(&self, mut state: BlockState) -> BlockState {
        if state == self.state {
            return state;
        }
        for property in &self.properties {
            let value = self.state.property_value_by_name(property).expect("a property of the input's block");
            state = state.with_property_by_name(property, value).unwrap_or(state);
        }
        state
    }
}

fn tag_matches(level: &mut ServerLevel, pos: BlockPos, tag: Option<&Nbt>) -> bool {
    let Some(tag) = tag else { return true };
    let Some(entity) = level.get_block_entity(pos) else { return false };
    crate::commands::arguments::nbt_path_argument::compare_nbt(tag, Some(&entity.save_with_full_metadata()))
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockPredicate {
    Block { state: BlockState, properties: Vec<&'static str>, nbt: Option<Nbt> },
    Tag { blocks: &'static [i32], vague_properties: Vec<(String, String)>, nbt: Option<Nbt> },
}

impl BlockPredicate {
    pub fn parse(reader: &mut StringReader) -> Result<BlockPredicate, CommandSyntaxException> {
        Ok(match block_state_parser::parse_for_testing(reader, true)? {
            ParseResult::Block(block) => BlockPredicate::Block { state: block.state, properties: block.properties, nbt: block.nbt },
            ParseResult::Tag(tag) => BlockPredicate::Tag { blocks: tag.blocks, vague_properties: tag.vague_properties, nbt: tag.nbt },
        })
    }

    pub fn test(&self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let state = level.block_state(pos);
        match self {
            BlockPredicate::Block { state: expected, properties, nbt } => {
                state.is_block(expected.block())
                    && properties.iter().all(|p| state.property_value_by_name(p) == expected.property_value_by_name(p))
                    && tag_matches(level, pos, nbt.as_ref())
            }
            BlockPredicate::Tag { blocks, vague_properties, nbt, .. } => {
                blocks.contains(&(state.block().0 as i32))
                    && vague_properties.iter().all(|(key, value)| {
                        let Some((name, values)) = state.block().property_by_name(key) else { return false };
                        property_value(values, value).is_some_and(|v| state.property_value_by_name(name) == Some(v))
                    })
                    && tag_matches(level, pos, nbt.as_ref())
            }
        }
    }
}
