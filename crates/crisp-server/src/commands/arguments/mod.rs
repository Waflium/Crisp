pub mod blocks;
pub mod component_argument;
pub mod coordinates;
pub mod dimension_argument;
pub mod item;
pub mod nbt_path_argument;
pub mod entity_argument;
pub mod function_argument;
pub mod game_profile_argument;
pub mod message_argument;
pub mod objective_argument;
pub mod objective_criteria_argument;
pub mod operation_argument;
pub mod particle_argument;
pub mod resource_or_id_argument;
pub mod resource_key_argument;
pub mod score_holder_argument;
pub mod scoreboard_slot_argument;
pub mod selector;
pub mod slot_argument;
pub mod slot_source_argument;

use std::sync::LazyLock;

use crisp_data::commands::{CommandNode, CommandNodeKind};
use crisp_protocol::nbt::Nbt;
use serde_json::Value;

use self::blocks::{BlockInput, BlockPredicate};
use self::coordinates::Coordinates;
use self::item::ItemInput;
use self::item::item_predicate_argument::ItemPredicate;
use self::nbt_path_argument::NbtPath;
use self::operation_argument::Operation;
use self::resource_or_id_argument::ResourceOrId;
use self::slot_source_argument::SlotSourceResult;
use crate::server::reloadable_server_registries;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;
use self::score_holder_argument::ScoreHolderResult;
use self::game_profile_argument::GameProfileResult;
use self::message_argument::Message;
use self::selector::entity_selector::EntitySelector;
use self::selector::entity_selector_parser::allow_selectors;
use crate::advancements::predicates::min_max_bounds::{Doubles, Ints};
use super::brigadier::{
    CommandContext, CommandSyntaxException, DOUBLE_TOO_BIG, DOUBLE_TOO_SMALL, DynamicCommandExceptionType, Dynamic2CommandExceptionType, FLOAT_TOO_BIG,
    FLOAT_TOO_SMALL, INTEGER_TOO_BIG, INTEGER_TOO_SMALL, LONG_TOO_BIG, LONG_TOO_SMALL, SimpleCommandExceptionType, StringReader, tr,
};
use super::shared_suggestion_provider;
use super::source::{Anchor, CommandSourceStack};
use super::suggestion::{Suggestions, SuggestionsBuilder};
use crate::network::chat::{Component, Style};
use crate::server::level::game_mode::GameType;
use crate::world::item::component::swing_animation::SwingAnimationType;
use crate::world::scores::criteria::ObjectiveCriteria;
use crate::world::scores::display_slot::DisplaySlot;
use crate::world::scores::team_color::TeamColor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StringType {
    SingleWord,
    QuotablePhrase,
    GreedyPhrase,
}

#[derive(Clone, Debug)]
pub enum ArgumentType {
    Bool,
    Double { min: f64, max: f64 },
    Float { min: f32, max: f32 },
    Integer { min: i32, max: i32 },
    Long { min: i64, max: i64 },
    String(StringType),
    BlockPos,
    ColumnPos,
    Vec3 { center_correct: bool },
    Vec2 { center_correct: bool },
    Rotation,
    Swizzle,
    EntityAnchor,
    GameMode,
    SwingAnimation,
    Time { min: i32 },
    Identifier,
    Dimension,
    Uuid,
    Resource { registry: String },
    ResourceKey { registry: String },
    ResourceOrTag { registry: String },
    ResourceOrTagKey { registry: String },
    CompoundTag,
    NbtTag,
    Function,
    NbtPath,
    BlockState,
    BlockPredicate,
    ItemStack,
    ItemPredicate,
    Entity { single: bool, players_only: bool },
    GameProfile,
    Message,
    IntRange,
    FloatRange,
    ScoreHolder { multiple: bool },
    Objective,
    ObjectiveCriteria,
    Operation,
    ScoreboardSlot,
    Team,
    TeamColor,
    Component,
    Style,
    LootTable,
    LootPredicate,
    LootModifier,
    ContextFloatProvider,
    ContextIntProvider,
    SlotSource,
    ItemSlot,
    Particle,
    Feature,
    Dialog,
    TemplateRotation,
    TemplateMirror,
    Unsupported(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArgumentValue {
    Bool(bool),
    Double(f64),
    Float(f32),
    Int(i32),
    Long(i64),
    String(String),
    Coordinates(Coordinates),
    Axes(u8),
    Anchor(Anchor),
    GameMode(GameType),
    SwingAnimation(SwingAnimationType),
    Identifier(String),
    Uuid(u128),
    Resource { registry: String, id: String },
    ResourceOrTag { tag: bool, id: String },
    Nbt(Nbt),
    NbtPath(NbtPath),
    Function { tag: bool, id: String },
    BlockInput(BlockInput),
    BlockPredicate(BlockPredicate),
    ItemInput(ItemInput),
    ItemPredicate(ItemPredicate),
    EntitySelector(Box<EntitySelector>),
    GameProfile(GameProfileResult),
    Message(Message),
    IntRange(Ints),
    FloatRange(Doubles),
    ScoreHolder(ScoreHolderResult),
    ObjectiveCriteria(ObjectiveCriteria),
    Operation(Operation),
    DisplaySlot(DisplaySlot),
    TeamColor(TeamColor),
    Component(Component),
    Style(Style),
    LootTable(ResourceOrId<LootTable>),
    LootPredicate(ResourceOrId<LootItemCondition>),
    LootModifier(ResourceOrId<LootItemFunction>),
    FloatProvider(ResourceOrId<ContextFloatProvider>),
    IntProvider(ResourceOrId<ContextIntProvider>),
    SlotSource(SlotSourceResult),
    Particle(crate::core::particles::particle_options::ParticleOptions),
    ResourceOrId(resource_or_id_argument::RawResourceOrId),
    Rotation(crate::world::level::block::rotation::Rotation),
    Mirror(crate::world::level::block::mirror::Mirror),
}

impl ArgumentType {
    fn of(parser: &'static str, properties: &Value) -> ArgumentType {
        let f64_prop = |key: &str, default: f64| properties.get(key).and_then(Value::as_f64).unwrap_or(default);
        let i64_prop = |key: &str, default: i64| properties.get(key).and_then(Value::as_i64).unwrap_or(default);
        let registry = || properties["registry"].as_str().unwrap_or_default().to_string();
        match parser {
            "brigadier:bool" => ArgumentType::Bool,
            "brigadier:double" => ArgumentType::Double { min: f64_prop("min", -f64::MAX), max: f64_prop("max", f64::MAX) },
            "brigadier:float" => ArgumentType::Float { min: f64_prop("min", -f32::MAX as f64) as f32, max: f64_prop("max", f32::MAX as f64) as f32 },
            "brigadier:integer" => ArgumentType::Integer { min: i64_prop("min", i32::MIN as i64) as i32, max: i64_prop("max", i32::MAX as i64) as i32 },
            "brigadier:long" => ArgumentType::Long { min: i64_prop("min", i64::MIN), max: i64_prop("max", i64::MAX) },
            "brigadier:string" => ArgumentType::String(match properties["type"].as_str() {
                Some("word") => StringType::SingleWord,
                Some("greedy") => StringType::GreedyPhrase,
                _ => StringType::QuotablePhrase,
            }),
            "minecraft:block_pos" => ArgumentType::BlockPos,
            "minecraft:column_pos" => ArgumentType::ColumnPos,
            "minecraft:vec3" => ArgumentType::Vec3 { center_correct: true },
            "minecraft:vec2" => ArgumentType::Vec2 { center_correct: true },
            "minecraft:rotation" => ArgumentType::Rotation,
            "minecraft:swizzle" => ArgumentType::Swizzle,
            "minecraft:entity_anchor" => ArgumentType::EntityAnchor,
            "minecraft:gamemode" => ArgumentType::GameMode,
            "minecraft:swing_animation" => ArgumentType::SwingAnimation,
            "minecraft:time" => ArgumentType::Time { min: i64_prop("min", 0) as i32 },
            "minecraft:resource_location" => ArgumentType::Identifier,
            "minecraft:dimension" => ArgumentType::Dimension,
            "minecraft:uuid" => ArgumentType::Uuid,
            "minecraft:resource" => ArgumentType::Resource { registry: registry() },
            "minecraft:resource_key" => ArgumentType::ResourceKey { registry: registry() },
            "minecraft:resource_or_tag" => ArgumentType::ResourceOrTag { registry: registry() },
            "minecraft:resource_or_tag_key" => ArgumentType::ResourceOrTagKey { registry: registry() },
            "minecraft:nbt_compound_tag" => ArgumentType::CompoundTag,
            "minecraft:nbt_tag" => ArgumentType::NbtTag,
            "minecraft:function" => ArgumentType::Function,
            "minecraft:nbt_path" => ArgumentType::NbtPath,
            "minecraft:block_state" => ArgumentType::BlockState,
            "minecraft:block_predicate" => ArgumentType::BlockPredicate,
            "minecraft:item_stack" => ArgumentType::ItemStack,
            "minecraft:item_predicate" => ArgumentType::ItemPredicate,
            "minecraft:entity" => ArgumentType::Entity { single: properties["amount"] == "single", players_only: properties["type"] == "players" },
            "minecraft:game_profile" => ArgumentType::GameProfile,
            "minecraft:message" => ArgumentType::Message,
            "minecraft:int_range" => ArgumentType::IntRange,
            "minecraft:float_range" => ArgumentType::FloatRange,
            "minecraft:score_holder" => ArgumentType::ScoreHolder { multiple: properties["amount"] == "multiple" },
            "minecraft:objective" => ArgumentType::Objective,
            "minecraft:objective_criteria" => ArgumentType::ObjectiveCriteria,
            "minecraft:operation" => ArgumentType::Operation,
            "minecraft:scoreboard_slot" => ArgumentType::ScoreboardSlot,
            "minecraft:team" => ArgumentType::Team,
            "minecraft:team_color" => ArgumentType::TeamColor,
            "minecraft:component" => ArgumentType::Component,
            "minecraft:style" => ArgumentType::Style,
            "minecraft:loot_table" => ArgumentType::LootTable,
            "minecraft:loot_predicate" => ArgumentType::LootPredicate,
            "minecraft:loot_modifier" => ArgumentType::LootModifier,
            "minecraft:context_float_provider" => ArgumentType::ContextFloatProvider,
            "minecraft:context_int_provider" => ArgumentType::ContextIntProvider,
            "minecraft:slot_source" => ArgumentType::SlotSource,
            "minecraft:item_slot" => ArgumentType::ItemSlot,
            "minecraft:particle" => ArgumentType::Particle,
            "minecraft:feature" => ArgumentType::Feature,
            "minecraft:dialog" => ArgumentType::Dialog,
            "minecraft:template_rotation" => ArgumentType::TemplateRotation,
            "minecraft:template_mirror" => ArgumentType::TemplateMirror,
            other => ArgumentType::Unsupported(other),
        }
    }

    fn parse(&self, reader: &mut StringReader, source: &CommandSourceStack) -> Result<ArgumentValue, CommandSyntaxException> {
        Ok(match *self {
            ArgumentType::Bool => ArgumentValue::Bool(reader.read_boolean()?),
            ArgumentType::Double { min, max } => {
                let start = reader.cursor;
                let result = reader.read_double()?;
                bounded(reader, start, result, min, max, &DOUBLE_TOO_SMALL, &DOUBLE_TOO_BIG)?;
                ArgumentValue::Double(result)
            }
            ArgumentType::Float { min, max } => {
                let start = reader.cursor;
                let result = reader.read_float()?;
                bounded(reader, start, result, min, max, &FLOAT_TOO_SMALL, &FLOAT_TOO_BIG)?;
                ArgumentValue::Float(result)
            }
            ArgumentType::Integer { min, max } => {
                let start = reader.cursor;
                let result = reader.read_int()?;
                bounded(reader, start, result, min, max, &INTEGER_TOO_SMALL, &INTEGER_TOO_BIG)?;
                ArgumentValue::Int(result)
            }
            ArgumentType::Long { min, max } => {
                let start = reader.cursor;
                let result = reader.read_long()?;
                bounded(reader, start, result, min, max, &LONG_TOO_SMALL, &LONG_TOO_BIG)?;
                ArgumentValue::Long(result)
            }
            ArgumentType::String(StringType::SingleWord) => ArgumentValue::String(reader.read_unquoted_string()),
            ArgumentType::String(StringType::QuotablePhrase) => ArgumentValue::String(reader.read_string()?),
            ArgumentType::String(StringType::GreedyPhrase) => {
                let text = reader.remaining();
                reader.cursor = reader.total_length();
                ArgumentValue::String(text)
            }
            ArgumentType::BlockPos => ArgumentValue::Coordinates(coordinates::parse_block_pos(reader)?),
            ArgumentType::ColumnPos => ArgumentValue::Coordinates(coordinates::parse_column_pos(reader)?),
            ArgumentType::Vec3 { center_correct } => ArgumentValue::Coordinates(coordinates::parse_vec3(reader, center_correct)?),
            ArgumentType::Vec2 { center_correct } => ArgumentValue::Coordinates(coordinates::parse_vec2(reader, center_correct)?),
            ArgumentType::Rotation => ArgumentValue::Coordinates(coordinates::parse_rotation(reader)?),
            ArgumentType::Swizzle => ArgumentValue::Axes(coordinates::parse_swizzle(reader)?),
            ArgumentType::EntityAnchor => {
                let start = reader.cursor;
                let name = reader.read_unquoted_string();
                ArgumentValue::Anchor(match name.as_str() {
                    "feet" => Anchor::Feet,
                    "eyes" => Anchor::Eyes,
                    _ => {
                        reader.cursor = start;
                        return Err(ERROR_INVALID_ANCHOR.create_with_context(reader, name));
                    }
                })
            }
            ArgumentType::GameMode => {
                let name = reader.read_unquoted_string();
                match GameType::by_name(&name) {
                    Some(mode) => ArgumentValue::GameMode(mode),
                    None => return Err(ERROR_INVALID_GAME_MODE.create_with_context(reader, name)),
                }
            }
            ArgumentType::SwingAnimation => {
                let name = reader.read_unquoted_string();
                match SwingAnimationType::by_name(&name) {
                    Some(ty) => ArgumentValue::SwingAnimation(ty),
                    None => return Err(ERROR_INVALID_SWING_ANIMATION.create_with_context(reader, name)),
                }
            }
            ArgumentType::Time { min } => ArgumentValue::Int(parse_time(reader, min)?),
            ArgumentType::Identifier | ArgumentType::Dimension => ArgumentValue::Identifier(read_identifier(reader)?),
            ArgumentType::Uuid => ArgumentValue::Uuid(parse_uuid(reader)?),
            ArgumentType::Resource { ref registry } => {
                let id = read_identifier(reader)?;
                if !crisp_data::registry_entries(registry).is_some_and(|entries| entries.contains(&id.as_str())) {
                    return Err(ERROR_UNKNOWN_RESOURCE.create_with_context(reader, id, registry.as_str()));
                }
                ArgumentValue::Resource { registry: registry.clone(), id }
            }
            ArgumentType::ResourceKey { ref registry } => ArgumentValue::Resource { registry: registry.clone(), id: read_identifier(reader)? },
            ArgumentType::ResourceOrTag { ref registry } => {
                if reader.can_read() && reader.peek() == b'#' as u16 {
                    let cursor = reader.cursor;
                    reader.skip();
                    let tag_id = read_identifier(reader).inspect_err(|_| reader.cursor = cursor)?;
                    if registry_tag(registry, &tag_id).is_none() {
                        let error = ERROR_UNKNOWN_TAG.create_with_context(reader, tag_id, registry.as_str());
                        reader.cursor = cursor;
                        return Err(error);
                    }
                    ArgumentValue::ResourceOrTag { tag: true, id: tag_id }
                } else {
                    let id = read_identifier(reader)?;
                    if !crisp_data::registry_entries(registry).is_some_and(|entries| entries.contains(&id.as_str())) {
                        return Err(ERROR_UNKNOWN_RESOURCE.create_with_context(reader, id, registry.as_str()));
                    }
                    ArgumentValue::ResourceOrTag { tag: false, id }
                }
            }
            ArgumentType::ResourceOrTagKey { .. } => {
                if reader.can_read() && reader.peek() == b'#' as u16 {
                    let cursor = reader.cursor;
                    reader.skip();
                    let tag_id = read_identifier(reader).inspect_err(|_| reader.cursor = cursor)?;
                    ArgumentValue::ResourceOrTag { tag: true, id: tag_id }
                } else {
                    ArgumentValue::ResourceOrTag { tag: false, id: read_identifier(reader)? }
                }
            }
            ArgumentType::CompoundTag => ArgumentValue::Nbt(crate::nbt::parse_compound_as_argument(reader)?),
            ArgumentType::NbtTag => ArgumentValue::Nbt(crate::nbt::parse_as_argument(reader)?),
            ArgumentType::Function => function_argument::parse(reader)?,
            ArgumentType::NbtPath => ArgumentValue::NbtPath(nbt_path_argument::parse(reader)?),
            ArgumentType::BlockState => ArgumentValue::BlockInput(BlockInput::parse(reader)?),
            ArgumentType::BlockPredicate => ArgumentValue::BlockPredicate(BlockPredicate::parse(reader)?),
            ArgumentType::ItemStack => ArgumentValue::ItemInput(item::item_parser::parse(reader)?),
            ArgumentType::ItemPredicate => ArgumentValue::ItemPredicate(item::item_predicate_argument::parse(reader)?),
            ArgumentType::Entity { single, players_only } => {
                ArgumentValue::EntitySelector(Box::new(entity_argument::parse(reader, single, players_only, allow_selectors(source))?))
            }
            ArgumentType::GameProfile => ArgumentValue::GameProfile(game_profile_argument::parse(reader, allow_selectors(source))?),
            ArgumentType::Message => ArgumentValue::Message(Message::parse_text(reader, allow_selectors(source))?),
            ArgumentType::IntRange => ArgumentValue::IntRange(Ints::from_reader(reader)?),
            ArgumentType::FloatRange => ArgumentValue::FloatRange(Doubles::from_reader(reader)?),
            ArgumentType::ScoreHolder { multiple } => ArgumentValue::ScoreHolder(score_holder_argument::parse(reader, multiple, allow_selectors(source))?),
            ArgumentType::Objective | ArgumentType::Team => ArgumentValue::String(reader.read_unquoted_string()),
            ArgumentType::ObjectiveCriteria => ArgumentValue::ObjectiveCriteria(objective_criteria_argument::parse(reader)?),
            ArgumentType::Operation => ArgumentValue::Operation(operation_argument::parse(reader)?),
            ArgumentType::ScoreboardSlot => ArgumentValue::DisplaySlot(scoreboard_slot_argument::parse_display_slot(reader)?),
            ArgumentType::TeamColor => ArgumentValue::TeamColor(scoreboard_slot_argument::parse_team_color(reader)?),
            ArgumentType::Component => ArgumentValue::Component(component_argument::parse_component(reader)?),
            ArgumentType::Style => ArgumentValue::Style(component_argument::parse_style(reader)?),
            ArgumentType::LootTable => ArgumentValue::LootTable(resource_or_id_argument::loot_table(reader)?),
            ArgumentType::LootPredicate => ArgumentValue::LootPredicate(resource_or_id_argument::loot_predicate(reader)?),
            ArgumentType::LootModifier => ArgumentValue::LootModifier(resource_or_id_argument::loot_modifier(reader)?),
            ArgumentType::ContextFloatProvider => ArgumentValue::FloatProvider(resource_or_id_argument::float_provider(reader)?),
            ArgumentType::ContextIntProvider => ArgumentValue::IntProvider(resource_or_id_argument::int_provider(reader)?),
            ArgumentType::SlotSource => ArgumentValue::SlotSource(slot_source_argument::parse(reader)?),
            ArgumentType::ItemSlot => ArgumentValue::Int(slot_argument::parse(reader)?),
            ArgumentType::Particle => ArgumentValue::Particle(particle_argument::read_particle(reader)?),
            ArgumentType::Feature => ArgumentValue::ResourceOrId(resource_or_id_argument::parse_raw(
                reader,
                "minecraft:worldgen/feature",
                |id| crate::world::level::levelgen::feature::feature::feature_index(id).is_some(),
                |value| crate::world::level::levelgen::feature::feature::Feature::parse_nbt(value).map(drop),
            )?),
            ArgumentType::Dialog => ArgumentValue::ResourceOrId(resource_or_id_argument::parse_raw(
                reader,
                "minecraft:dialog",
                |id| crisp_data::SyncedRegistry::get("minecraft:dialog").id_of(id).is_some(),
                crate::server::dialog::validate,
            )?),
            ArgumentType::TemplateRotation => {
                let id = reader.read_unquoted_string();
                match crate::world::level::block::rotation::Rotation::by_name(&id) {
                    Some(rotation) => ArgumentValue::Rotation(rotation),
                    None => return Err(ERROR_INVALID_ENUM_VALUE.create_with_context(reader, id)),
                }
            }
            ArgumentType::TemplateMirror => {
                let id = reader.read_unquoted_string();
                match crate::world::level::block::mirror::Mirror::by_name(&id) {
                    Some(mirror) => ArgumentValue::Mirror(mirror),
                    None => return Err(ERROR_INVALID_ENUM_VALUE.create_with_context(reader, id)),
                }
            }
            ArgumentType::Unsupported(parser) => {
                return Err(super::brigadier::DISPATCHER_PARSE_EXCEPTION.create_with_context(reader, format!("{parser} arguments are not supported yet")));
            }
        })
    }
}

fn bounded<T: PartialOrd + Copy + Into<crate::network::chat::Arg>>(
    reader: &mut StringReader,
    start: usize,
    value: T,
    min: T,
    max: T,
    too_small: &Dynamic2CommandExceptionType,
    too_big: &Dynamic2CommandExceptionType,
) -> Result<(), CommandSyntaxException> {
    if value < min {
        reader.cursor = start;
        return Err(too_small.create_with_context(reader, value, min));
    }
    if value > max {
        reader.cursor = start;
        return Err(too_big.create_with_context(reader, value, max));
    }
    Ok(())
}

pub static ERROR_INVALID_ANCHOR: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("argument.anchor.invalid", vec![name]));
pub static ERROR_INVALID_GAME_MODE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.gamemode.invalid", vec![value]));
static ERROR_INVALID_SWING_ANIMATION: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.swing_animation.invalid", vec![value]));
pub static ERROR_INVALID_UNIT: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.time.invalid_unit");
pub static ERROR_TICK_COUNT_TOO_LOW: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|value, limit| tr("argument.time.tick_count_too_low", vec![limit, value]));
pub static ERROR_INVALID_ID: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.id.invalid");
pub static ERROR_INVALID_UUID: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.uuid.invalid");
pub static ERROR_UNKNOWN_RESOURCE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|id, registry| tr("argument.resource.not_found", vec![id, registry]));
pub static ERROR_UNKNOWN_TAG: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|id, registry| tr("argument.resource_tag.not_found", vec![id, registry]));
pub static ERROR_INVALID_ENUM_VALUE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.enum.invalid", vec![value]));
pub static ERROR_INVALID_DIMENSION: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.dimension.invalid", vec![value]));

fn parse_time(reader: &mut StringReader, minimum: i32) -> Result<i32, CommandSyntaxException> {
    let value = reader.read_float()?;
    let unit = reader.read_unquoted_string();
    let factor = match unit.as_str() {
        "d" => 24000,
        "s" => 20,
        "t" | "" => 1,
        _ => return Err(ERROR_INVALID_UNIT.create_with_context(reader)),
    };
    let ticks = crisp_core::mth::java_round_f32(value * factor as f32);
    if ticks < minimum {
        return Err(ERROR_TICK_COUNT_TOO_LOW.create_with_context(reader, ticks, minimum));
    }
    Ok(ticks)
}

pub fn is_allowed_in_identifier(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x61..=0x7A) || [b'_', b':', b'/', b'.', b'-'].iter().any(|&b| c == b as u16)
}

pub fn read_identifier(reader: &mut StringReader) -> Result<String, CommandSyntaxException> {
    let start = reader.cursor;
    while reader.can_read() && is_allowed_in_identifier(reader.peek()) {
        reader.skip();
    }
    let raw = reader.substring(start, reader.cursor);
    parse_identifier(&raw).ok_or_else(|| {
        reader.cursor = start;
        ERROR_INVALID_ID.create_with_context(reader)
    })
}

pub fn parse_identifier(raw: &str) -> Option<String> {
    let valid_path = |p: &str| p.bytes().all(|c| matches!(c, b'_' | b'-' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.'));
    let valid_namespace = |n: &str| n != ".." && n.bytes().all(|c| matches!(c, b'_' | b'-' | b'a'..=b'z' | b'0'..=b'9' | b'.'));
    match raw.split_once(':') {
        Some(("", path)) => valid_path(path).then(|| format!("minecraft:{path}")),
        Some((namespace, path)) => (valid_namespace(namespace) && valid_path(path)).then(|| raw.to_string()),
        None => valid_path(raw).then(|| format!("minecraft:{raw}")),
    }
}

pub fn registry_tag(registry: &str, tag: &str) -> Option<&'static [i32]> {
    let tags = crisp_data::tags().iter().find(|r| r.registry == registry)?;
    tags.tags.iter().find(|t| t.0 == tag).map(|t| t.1)
}

fn parse_uuid(reader: &mut StringReader) -> Result<u128, CommandSyntaxException> {
    let start = reader.cursor;
    let mut end = start;
    while end < reader.total_length() {
        let c = reader.string()[end];
        if !(c == b'-' as u16 || char::from_u32(c as u32).is_some_and(|ch| ch.is_ascii_hexdigit())) {
            break;
        }
        end += 1;
    }
    if end > start
        && let Some(uuid) = java_uuid_from_string(&reader.substring(start, end))
    {
        reader.cursor = end;
        return Ok(uuid);
    }
    Err(ERROR_INVALID_UUID.create_with_context(reader))
}

pub fn java_uuid_from_string(s: &str) -> Option<u128> {
    if s.len() > 36 {
        return None;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let parse = |p: &str| i64::from_str_radix(p, 16).ok().map(|v| v as u64);
    let (a, b, c, d, e) = (parse(parts[0])?, parse(parts[1])?, parse(parts[2])?, parse(parts[3])?, parse(parts[4])?);
    let most = (a & 0xFFFF_FFFF) << 32 | (b & 0xFFFF) << 16 | (c & 0xFFFF);
    let least = (d & 0xFFFF) << 48 | (e & 0xFFFF_FFFF_FFFF);
    Some((most as u128) << 64 | least as u128)
}

static TYPES: LazyLock<Vec<Option<ArgumentType>>> = LazyLock::new(|| {
    let mut types: Vec<Option<ArgumentType>> = crisp_data::commands::CommandNode::all()
        .iter()
        .map(|n| {
            (n.kind == CommandNodeKind::Argument)
                .then(|| ArgumentType::of(n.parser.unwrap(), &serde_json::from_str(n.properties).unwrap_or(Value::Null)))
        })
        .collect();
    types[super::node_at("particle name pos delta").id as usize] = Some(ArgumentType::Vec3 { center_correct: false });
    types
});

pub fn type_of(node: &CommandNode) -> &'static ArgumentType {
    TYPES[node.id as usize].as_ref().expect("an argument node")
}

pub fn parse(node: &CommandNode, reader: &mut StringReader, source: &CommandSourceStack) -> Result<ArgumentValue, CommandSyntaxException> {
    type_of(node).parse(reader, source)
}

impl CommandContext {
    pub fn get_bool(&self, name: &str) -> bool {
        match self.argument(name) {
            ArgumentValue::Bool(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_int(&self, name: &str) -> i32 {
        match self.argument(name) {
            ArgumentValue::Int(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_long(&self, name: &str) -> i64 {
        match self.argument(name) {
            ArgumentValue::Long(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_float(&self, name: &str) -> f32 {
        match self.argument(name) {
            ArgumentValue::Float(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_double(&self, name: &str) -> f64 {
        match self.argument(name) {
            ArgumentValue::Double(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_string(&self, name: &str) -> &str {
        match self.argument(name) {
            ArgumentValue::String(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_coordinates(&self, name: &str) -> &Coordinates {
        match self.argument(name) {
            ArgumentValue::Coordinates(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_anchor(&self, name: &str) -> Anchor {
        match self.argument(name) {
            ArgumentValue::Anchor(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_swing_animation_type(&self, name: &str) -> SwingAnimationType {
        match self.argument(name) {
            ArgumentValue::SwingAnimation(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_int_range(&self, name: &str) -> Ints {
        match self.argument(name) {
            ArgumentValue::IntRange(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_float_range(&self, name: &str) -> Doubles {
        match self.argument(name) {
            ArgumentValue::FloatRange(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_game_mode(&self, name: &str) -> GameType {
        match self.argument(name) {
            ArgumentValue::GameMode(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_identifier(&self, name: &str) -> &str {
        match self.argument(name) {
            ArgumentValue::Identifier(v) => v,
            ArgumentValue::Resource { id, .. } => id,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_resource_or_tag(&self, name: &str) -> (bool, &str) {
        match self.argument(name) {
            ArgumentValue::ResourceOrTag { tag, id } => (*tag, id),
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_nbt(&self, name: &str) -> &Nbt {
        match self.argument(name) {
            ArgumentValue::Nbt(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_nbt_path(&self, name: &str) -> &NbtPath {
        match self.argument(name) {
            ArgumentValue::NbtPath(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_block(&self, name: &str) -> &BlockInput {
        match self.argument(name) {
            ArgumentValue::BlockInput(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_block_predicate(&self, name: &str) -> &BlockPredicate {
        match self.argument(name) {
            ArgumentValue::BlockPredicate(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_item(&self, name: &str) -> &ItemInput {
        match self.argument(name) {
            ArgumentValue::ItemInput(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_item_predicate(&self, name: &str) -> &ItemPredicate {
        match self.argument(name) {
            ArgumentValue::ItemPredicate(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_loot_table(&self, name: &str) -> &ResourceOrId<LootTable> {
        match self.argument(name) {
            ArgumentValue::LootTable(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_resource_or_id(&self, name: &str) -> &resource_or_id_argument::RawResourceOrId {
        match self.argument(name) {
            ArgumentValue::ResourceOrId(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_loot_predicate(&self, name: &str) -> &ResourceOrId<LootItemCondition> {
        match self.argument(name) {
            ArgumentValue::LootPredicate(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_rotation(&self, name: &str) -> crate::world::level::block::rotation::Rotation {
        match self.argument(name) {
            ArgumentValue::Rotation(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_loot_modifier(&self, name: &str) -> &ResourceOrId<LootItemFunction> {
        match self.argument(name) {
            ArgumentValue::LootModifier(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_mirror(&self, name: &str) -> crate::world::level::block::mirror::Mirror {
        match self.argument(name) {
            ArgumentValue::Mirror(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_float_provider(&self, name: &str) -> &ResourceOrId<ContextFloatProvider> {
        match self.argument(name) {
            ArgumentValue::FloatProvider(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_uuid(&self, name: &str) -> u128 {
        match self.argument(name) {
            ArgumentValue::Uuid(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_int_provider(&self, name: &str) -> &ResourceOrId<ContextIntProvider> {
        match self.argument(name) {
            ArgumentValue::IntProvider(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_slot_source(&self, name: &str) -> &SlotSourceResult {
        match self.argument(name) {
            ArgumentValue::SlotSource(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_particle(&self, name: &str) -> &crate::core::particles::particle_options::ParticleOptions {
        match self.argument(name) {
            ArgumentValue::Particle(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }
}

pub fn list_suggestions(
    server: &crate::server::Server,
    node: &CommandNode,
    source: &CommandSourceStack,
    mut builder: SuggestionsBuilder,
) -> Suggestions {
    let player_names = || server.players().map(|p| p.profile.name.clone()).collect::<Vec<String>>();
    let validator = |input: &str| {
        let units: Vec<u16> = input.encode_utf16().collect();
        type_of(node).parse(&mut StringReader::new(&units), source).is_ok()
    };
    let coordinates = |remaining: &str| {
        if remaining.starts_with('^') { [shared_suggestion_provider::DEFAULT_LOCAL] } else { [shared_suggestion_provider::DEFAULT_GLOBAL] }
    };
    match type_of(node) {
        ArgumentType::Bool => {
            let remaining = builder.remaining_lower_case();
            for value in ["true", "false"] {
                if value.starts_with(&remaining) {
                    builder.suggest(value);
                }
            }
        }
        ArgumentType::BlockPos | ArgumentType::Vec3 { .. } => {
            let remaining = builder.remaining();
            shared_suggestion_provider::suggest_coordinates(&remaining, &coordinates(&remaining), &mut builder, validator);
        }
        ArgumentType::ColumnPos | ArgumentType::Vec2 { .. } => {
            let remaining = builder.remaining();
            shared_suggestion_provider::suggest_2d_coordinates(&remaining, &coordinates(&remaining), &mut builder, validator);
        }
        ArgumentType::EntityAnchor => shared_suggestion_provider::suggest(["feet", "eyes"], &mut builder),
        ArgumentType::GameMode => shared_suggestion_provider::suggest(GameType::VALUES.map(GameType::name), &mut builder),
        ArgumentType::SwingAnimation => shared_suggestion_provider::suggest(["none", "whack", "stab"], &mut builder),
        ArgumentType::Time { .. } => {
            let remaining: Vec<u16> = builder.remaining().encode_utf16().collect();
            let mut reader = StringReader::new(&remaining);
            if reader.read_float().is_err() {
                return builder.build();
            }
            let mut units = builder.create_offset(builder.start() + reader.cursor);
            shared_suggestion_provider::suggest(["d", "s", "t", ""], &mut units);
            return units.build();
        }
        ArgumentType::Dimension => shared_suggestion_provider::suggest_resource(server.levels.iter().map(|level| level.dimension), &mut builder),
        ArgumentType::Resource { registry } => {
            shared_suggestion_provider::suggest_resource(crisp_data::registry_entries(registry).unwrap_or_default().iter(), &mut builder)
        }
        ArgumentType::ResourceKey { registry } => match registry.as_str() {
            "minecraft:recipe" => resource_key_argument::suggest_recipes(&mut builder),
            _ => shared_suggestion_provider::suggest_resource(crisp_data::registry_entries(registry).unwrap_or_default().iter(), &mut builder),
        },
        ArgumentType::ResourceOrTag { registry } | ArgumentType::ResourceOrTagKey { registry } => {
            let tags = crisp_data::tags().iter().find(|r| r.registry == registry).map_or(&[][..], |r| r.tags);
            shared_suggestion_provider::suggest_resource_with_prefix(tags.iter().map(|t| t.0), &mut builder, "#");
            match registry.as_str() {
                "minecraft:worldgen/structure" => shared_suggestion_provider::suggest_resource(crisp_data::STRUCTURES.iter().map(|s| s.0), &mut builder),
                _ => shared_suggestion_provider::suggest_resource(crisp_data::registry_entries(registry).unwrap_or_default().iter(), &mut builder),
            }
        }
        ArgumentType::Entity { .. } | ArgumentType::GameProfile => {
            return selector_suggestions(&builder, source, |b| shared_suggestion_provider::suggest(player_names(), b));
        }
        ArgumentType::Objective => shared_suggestion_provider::suggest(server.scoreboard.borrow().objectives().map(|o| o.name().to_string()), &mut builder),
        ArgumentType::ObjectiveCriteria => shared_suggestion_provider::suggest(crate::world::scores::criteria::all_names(), &mut builder),
        ArgumentType::Operation => shared_suggestion_provider::suggest(["=", "+=", "-=", "*=", "/=", "%=", "<", ">", "><"], &mut builder),
        ArgumentType::ScoreboardSlot => shared_suggestion_provider::suggest(DisplaySlot::VALUES.map(DisplaySlot::serialized_name), &mut builder),
        ArgumentType::Team => shared_suggestion_provider::suggest(server.scoreboard.borrow().player_teams().map(|t| t.name().to_string()), &mut builder),
        ArgumentType::TeamColor => shared_suggestion_provider::suggest(TeamColor::VALUES.map(TeamColor::serialized_name), &mut builder),
        ArgumentType::LootTable => resource_or_id_argument::list_suggestions(reloadable_server_registries::LOOT_TABLE, &mut builder),
        ArgumentType::LootPredicate => resource_or_id_argument::list_suggestions(reloadable_server_registries::PREDICATE, &mut builder),
        ArgumentType::LootModifier => resource_or_id_argument::list_suggestions(reloadable_server_registries::ITEM_MODIFIER, &mut builder),
        ArgumentType::ContextFloatProvider => resource_or_id_argument::list_suggestions(reloadable_server_registries::CONTEXT_FLOAT_PROVIDER, &mut builder),
        ArgumentType::ContextIntProvider => resource_or_id_argument::list_suggestions(reloadable_server_registries::CONTEXT_INT_PROVIDER, &mut builder),
        ArgumentType::SlotSource => slot_source_argument::list_suggestions(&mut builder),
        ArgumentType::ItemSlot => slot_argument::list_suggestions(&mut builder),
        ArgumentType::Particle => shared_suggestion_provider::suggest_resource(crisp_data::registry::PARTICLE_TYPE.iter(), &mut builder),
        ArgumentType::Feature => shared_suggestion_provider::suggest_resource(crisp_data::FEATURES.iter().map(|f| f.0), &mut builder),
        ArgumentType::Dialog => shared_suggestion_provider::suggest_resource(crisp_data::SyncedRegistry::get("minecraft:dialog").entries.iter(), &mut builder),
        ArgumentType::TemplateRotation => {
            shared_suggestion_provider::suggest(["none", "clockwise_90", "180", "counterclockwise_90"], &mut builder)
        }
        ArgumentType::TemplateMirror => shared_suggestion_provider::suggest(["none", "left_right", "front_back"], &mut builder),
        _ => {}
    }
    builder.build()
}

pub fn selector_suggestions(builder: &SuggestionsBuilder, source: &CommandSourceStack, names: impl FnOnce(&mut SuggestionsBuilder)) -> Suggestions {
    let input = builder.input().to_vec();
    let mut reader = StringReader::new(&input);
    reader.cursor = builder.start();
    let mut parser = selector::entity_selector_parser::EntitySelectorParser::new(&mut reader, allow_selectors(source));
    let _ = parser.parse();
    parser.fill_suggestions(builder, names)
}
