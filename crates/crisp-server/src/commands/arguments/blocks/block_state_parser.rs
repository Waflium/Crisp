use crisp_data::{Block, BlockState};
use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::{read_identifier, registry_tag};
use crate::commands::brigadier::{
    CommandSyntaxException, Dynamic2CommandExceptionType, Dynamic3CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, StringReader,
    tr,
};
use crate::nbt;

pub static ERROR_NO_TAGS_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.block.tag.disallowed");
pub static ERROR_UNKNOWN_BLOCK: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("argument.block.id.invalid", vec![id]));
pub static ERROR_UNKNOWN_PROPERTY: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|block, property| tr("argument.block.property.unknown", vec![block, property]));
pub static ERROR_DUPLICATE_PROPERTY: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|block, property| tr("argument.block.property.duplicate", vec![property, block]));
pub static ERROR_INVALID_VALUE: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|block, property, value| tr("argument.block.property.invalid", vec![block, value, property]));
pub static ERROR_EXPECTED_VALUE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|block, property| tr("argument.block.property.novalue", vec![property, block]));
pub static ERROR_EXPECTED_END_OF_PROPERTIES: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.block.property.unclosed");
pub static ERROR_UNKNOWN_TAG: DynamicCommandExceptionType = DynamicCommandExceptionType(|tag| tr("arguments.block.tag.unknown", vec![tag]));

pub struct BlockResult {
    pub state: BlockState,
    pub properties: Vec<&'static str>,
    pub nbt: Option<Nbt>,
}

pub struct TagResult {
    pub blocks: &'static [i32],
    pub vague_properties: Vec<(String, String)>,
    pub nbt: Option<Nbt>,
}

pub enum ParseResult {
    Block(BlockResult),
    Tag(TagResult),
}

struct BlockStateParser<'r, 'a> {
    reader: &'r mut StringReader<'a>,
    for_testing: bool,
    allow_nbt: bool,
    properties: Vec<&'static str>,
    vague_properties: Vec<(String, String)>,
    id: String,
    state: Option<BlockState>,
    tag: Option<&'static [i32]>,
    nbt: Option<Nbt>,
}

pub fn parse_for_block(reader: &mut StringReader, allow_nbt: bool) -> Result<BlockResult, CommandSyntaxException> {
    let cursor = reader.cursor;
    let mut parser = BlockStateParser::new(reader, false, allow_nbt);
    match parser.parse() {
        Ok(()) => Ok(BlockResult { state: parser.state.expect("a parsed block"), properties: parser.properties, nbt: parser.nbt }),
        Err(e) => {
            reader.cursor = cursor;
            Err(e)
        }
    }
}

pub fn parse_for_testing(reader: &mut StringReader, allow_nbt: bool) -> Result<ParseResult, CommandSyntaxException> {
    let cursor = reader.cursor;
    let mut parser = BlockStateParser::new(reader, true, allow_nbt);
    match parser.parse() {
        Ok(()) => Ok(match parser.tag {
            Some(blocks) => ParseResult::Tag(TagResult { blocks, vague_properties: parser.vague_properties, nbt: parser.nbt }),
            None => ParseResult::Block(BlockResult { state: parser.state.expect("a parsed block"), properties: parser.properties, nbt: parser.nbt }),
        }),
        Err(e) => {
            reader.cursor = cursor;
            Err(e)
        }
    }
}

pub fn property_value(values: &'static [&'static str], raw: &str) -> Option<&'static str> {
    let integer = values.iter().all(|v| v.parse::<i32>().is_ok());
    if integer {
        let value = java_parse_int(raw)?;
        values.iter().copied().find(|v| v.parse::<i32>() == Ok(value))
    } else {
        values.iter().copied().find(|v| *v == raw)
    }
}

fn java_parse_int(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.bytes().try_fold(0i64, |v, c| v.checked_mul(10)?.checked_add((c - b'0') as i64).filter(|&v| v <= 1 << 31))?;
    i32::try_from(if s.starts_with('-') { -magnitude } else { magnitude }).ok()
}

impl<'r, 'a> BlockStateParser<'r, 'a> {
    fn new(reader: &'r mut StringReader<'a>, for_testing: bool, allow_nbt: bool) -> Self {
        BlockStateParser {
            reader,
            for_testing,
            allow_nbt,
            properties: Vec::new(),
            vague_properties: Vec::new(),
            id: "minecraft:".into(),
            state: None,
            tag: None,
            nbt: None,
        }
    }

    fn parse(&mut self) -> Result<(), CommandSyntaxException> {
        if self.reader.at('#') {
            self.read_tag()?;
            if self.reader.at('[') {
                self.read_vague_properties()?;
            }
        } else {
            self.read_block()?;
            if self.reader.at('[') {
                self.read_properties()?;
            }
        }
        if self.allow_nbt && self.reader.at('{') {
            self.nbt = Some(nbt::parse_compound_as_argument(self.reader)?);
        }
        Ok(())
    }

    fn read_block(&mut self) -> Result<(), CommandSyntaxException> {
        let start = self.reader.cursor;
        self.id = read_identifier(self.reader)?;
        let Some(block) = Block::by_name(&self.id) else {
            self.reader.cursor = start;
            return Err(ERROR_UNKNOWN_BLOCK.create_with_context(self.reader, self.id.as_str()));
        };
        self.state = Some(block.default_state());
        Ok(())
    }

    fn read_tag(&mut self) -> Result<(), CommandSyntaxException> {
        if !self.for_testing {
            return Err(ERROR_NO_TAGS_ALLOWED.create_with_context(self.reader));
        }
        let start = self.reader.cursor;
        self.reader.expect('#')?;
        let id = read_identifier(self.reader)?;
        match registry_tag("minecraft:block", &id) {
            Some(tag) => self.tag = Some(tag),
            None => {
                self.reader.cursor = start;
                return Err(ERROR_UNKNOWN_TAG.create_with_context(self.reader, id));
            }
        }
        Ok(())
    }

    fn read_properties(&mut self) -> Result<(), CommandSyntaxException> {
        self.reader.skip();
        self.reader.skip_whitespace();
        let block = self.state.expect("a parsed block").block();
        while self.reader.can_read() && !self.reader.at(']') {
            self.reader.skip_whitespace();
            let key_start = self.reader.cursor;
            let key = self.reader.read_string()?;
            let Some((property, values)) = block.property_by_name(&key) else {
                self.reader.cursor = key_start;
                return Err(ERROR_UNKNOWN_PROPERTY.create_with_context(self.reader, self.id.as_str(), key));
            };
            if self.properties.contains(&property) {
                self.reader.cursor = key_start;
                return Err(ERROR_DUPLICATE_PROPERTY.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip_whitespace();
            if !self.reader.at('=') {
                return Err(ERROR_EXPECTED_VALUE.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip();
            self.reader.skip_whitespace();
            let start = self.reader.cursor;
            let raw = self.reader.read_string()?;
            self.set_value(property, values, &raw, start)?;
            self.reader.skip_whitespace();
            if self.reader.can_read() {
                if !self.reader.at(',') {
                    if !self.reader.at(']') {
                        return Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader));
                    }
                    break;
                }
                self.reader.skip();
            }
        }
        if self.reader.can_read() {
            self.reader.skip();
            Ok(())
        } else {
            Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader))
        }
    }

    fn read_vague_properties(&mut self) -> Result<(), CommandSyntaxException> {
        self.reader.skip();
        let mut value_start = None;
        self.reader.skip_whitespace();
        while self.reader.can_read() && !self.reader.at(']') {
            self.reader.skip_whitespace();
            let key_start = self.reader.cursor;
            let key = self.reader.read_string()?;
            if self.vague_properties.iter().any(|(k, _)| *k == key) {
                self.reader.cursor = key_start;
                return Err(ERROR_DUPLICATE_PROPERTY.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip_whitespace();
            if !self.reader.at('=') {
                self.reader.cursor = key_start;
                return Err(ERROR_EXPECTED_VALUE.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip();
            self.reader.skip_whitespace();
            value_start = Some(self.reader.cursor);
            let value = self.reader.read_string()?;
            self.vague_properties.push((key, value));
            self.reader.skip_whitespace();
            if self.reader.can_read() {
                value_start = None;
                if !self.reader.at(',') {
                    if !self.reader.at(']') {
                        return Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader));
                    }
                    break;
                }
                self.reader.skip();
            }
        }
        if self.reader.can_read() {
            self.reader.skip();
            return Ok(());
        }
        if let Some(start) = value_start {
            self.reader.cursor = start;
        }
        Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader))
    }

    fn set_value(&mut self, property: &'static str, values: &'static [&'static str], raw: &str, start: usize) -> Result<(), CommandSyntaxException> {
        match property_value(values, raw) {
            Some(value) => {
                let state = self.state.expect("a parsed block");
                self.state = state.with_property_by_name(property, value);
                self.properties.push(property);
                Ok(())
            }
            None => {
                self.reader.cursor = start;
                Err(ERROR_INVALID_VALUE.create_with_context(self.reader, self.id.as_str(), property, raw))
            }
        }
    }
}
