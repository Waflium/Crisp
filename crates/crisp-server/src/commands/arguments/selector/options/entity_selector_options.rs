use crisp_data::registry;

use crate::advancements::predicates::min_max_bounds::{Doubles, FloatDegrees, Ints};
use crate::commands::arguments::read_identifier;
use crate::commands::arguments::selector::entity_selector::{AdvancementSelectorPredicate, EntityPredicate, Order, player_type};
use crate::commands::arguments::selector::entity_selector_parser::{EntitySelectorParser, SelectorSuggestions};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::SuggestionsBuilder;
use crate::network::chat::Component;
use crate::commands::brigadier::{CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::server::level::game_mode::GameType;

pub static ERROR_UNKNOWN_OPTION: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("argument.entity.options.unknown", vec![name]));
pub static ERROR_INAPPLICABLE_OPTION: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("argument.entity.options.inapplicable", vec![name]));
pub static ERROR_RANGE_NEGATIVE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.options.distance.negative");
pub static ERROR_LEVEL_NEGATIVE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.options.level.negative");
pub static ERROR_LIMIT_TOO_SMALL: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.options.limit.toosmall");
pub static ERROR_SORT_UNKNOWN: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("argument.entity.options.sort.irreversible", vec![name]));
pub static ERROR_GAME_MODE_INVALID: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("argument.entity.options.mode.invalid", vec![name]));
pub static ERROR_ENTITY_TYPE_INVALID: DynamicCommandExceptionType = DynamicCommandExceptionType(|t| tr("argument.entity.options.type.invalid", vec![t]));

pub type Modifier = fn(&mut EntitySelectorParser) -> Result<()>;

type Result<T> = std::result::Result<T, CommandSyntaxException>;

fn rollback(parser: &mut EntitySelectorParser, start: usize, error: &SimpleCommandExceptionType) -> CommandSyntaxException {
    parser.reader.cursor = start;
    error.create_with_context(parser.reader)
}

fn rollback_with(parser: &mut EntitySelectorParser, start: usize, error: &DynamicCommandExceptionType, argument: &str) -> CommandSyntaxException {
    parser.reader.cursor = start;
    error.create_with_context(parser.reader, argument)
}

const OPTION_NAMES: [&str; 21] = [
    "name", "distance", "level", "x", "y", "z", "dx", "dy", "dz", "x_rotation", "y_rotation", "limit", "sort", "gamemode", "team", "type", "tag", "nbt", "scores",
    "advancements", "predicate",
];

fn option(parser: &EntitySelectorParser, key: &str) -> Option<(Modifier, bool)> {
    Some(match key {
        "name" => (name, parser.name_option.can_parse_any()),
        "distance" => (distance, parser.distance.is_none()),
        "level" => (level, parser.level.is_none()),
        "x" => (x, parser.x.is_none()),
        "y" => (y, parser.y.is_none()),
        "z" => (z, parser.z.is_none()),
        "dx" => (dx, parser.delta_x.is_none()),
        "dy" => (dy, parser.delta_y.is_none()),
        "dz" => (dz, parser.delta_z.is_none()),
        "x_rotation" => (x_rotation, parser.rot_x.is_none()),
        "y_rotation" => (y_rotation, parser.rot_y.is_none()),
        "limit" => (limit, !parser.current_entity && parser.limited_option.can_parse()),
        "sort" => (sort, !parser.current_entity && parser.sorted_option.can_parse()),
        "gamemode" => (gamemode, parser.gamemode_option.can_parse_any()),
        "team" => (team, parser.team_option.can_parse_any()),
        "type" => (entity_type, parser.type_option.can_parse_any()),
        "tag" => (tag, true),
        "nbt" => (nbt, true),
        "scores" => (scores, parser.scores_option.can_parse()),
        "advancements" => (advancements, parser.advancements_option.can_parse()),
        "predicate" => (predicate, true),
        _ => return None,
    })
}

pub fn get(parser: &mut EntitySelectorParser, key: &str, start: usize) -> Result<Modifier> {
    let Some((modifier, can_use)) = option(parser, key) else {
        return Err(rollback_with(parser, start, &ERROR_UNKNOWN_OPTION, key));
    };
    if !can_use {
        return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, key));
    }
    Ok(modifier)
}

fn name(parser: &mut EntitySelectorParser) -> Result<()> {
    let start = parser.reader.cursor;
    let inverted = parser.should_invert_value();
    let name = parser.reader.read_string()?;
    if !parser.name_option.can_parse_element(inverted) {
        return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, "name"));
    }
    parser.name_option.mark_parsed_element(inverted);
    parser.predicates.push(EntityPredicate::Name { name, inverted });
    Ok(())
}

fn distance(parser: &mut EntitySelectorParser) -> Result<()> {
    let start = parser.reader.cursor;
    let value = Doubles::from_reader(parser.reader)?;
    if value.bounds.min.is_some_and(|v| v < 0.0) || value.bounds.max.is_some_and(|v| v < 0.0) {
        return Err(rollback(parser, start, &ERROR_RANGE_NEGATIVE));
    }
    parser.distance = Some(value);
    parser.world_limited = true;
    Ok(())
}

fn level(parser: &mut EntitySelectorParser) -> Result<()> {
    let start = parser.reader.cursor;
    let value = Ints::from_reader(parser.reader)?;
    if value.bounds.min.is_some_and(|v| v < 0) || value.bounds.max.is_some_and(|v| v < 0) {
        return Err(rollback(parser, start, &ERROR_LEVEL_NEGATIVE));
    }
    parser.level = Some(value);
    parser.includes_entities = false;
    Ok(())
}

fn x(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.world_limited = true;
    parser.x = Some(parser.reader.read_double()?);
    Ok(())
}

fn y(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.world_limited = true;
    parser.y = Some(parser.reader.read_double()?);
    Ok(())
}

fn z(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.world_limited = true;
    parser.z = Some(parser.reader.read_double()?);
    Ok(())
}

fn dx(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.world_limited = true;
    parser.delta_x = Some(parser.reader.read_double()?);
    Ok(())
}

fn dy(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.world_limited = true;
    parser.delta_y = Some(parser.reader.read_double()?);
    Ok(())
}

fn dz(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.world_limited = true;
    parser.delta_z = Some(parser.reader.read_double()?);
    Ok(())
}

fn x_rotation(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.rot_x = Some(FloatDegrees::from_reader(parser.reader)?);
    Ok(())
}

fn y_rotation(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.rot_y = Some(FloatDegrees::from_reader(parser.reader)?);
    Ok(())
}

fn limit(parser: &mut EntitySelectorParser) -> Result<()> {
    let start = parser.reader.cursor;
    let count = parser.reader.read_int()?;
    if count < 1 {
        return Err(rollback(parser, start, &ERROR_LIMIT_TOO_SMALL));
    }
    parser.max_results = count;
    parser.limited_option.mark_parsed();
    Ok(())
}

fn sort(parser: &mut EntitySelectorParser) -> Result<()> {
    let start = parser.reader.cursor;
    let name = parser.reader.read_unquoted_string();
    parser.suggestions = SelectorSuggestions::Sort;
    parser.order = match name.as_str() {
        "nearest" => Order::Nearest,
        "furthest" => Order::Furthest,
        "random" => Order::Random,
        "arbitrary" => Order::Arbitrary,
        _ => return Err(rollback_with(parser, start, &ERROR_SORT_UNKNOWN, &name)),
    };
    parser.sorted_option.mark_parsed();
    Ok(())
}

fn gamemode(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.suggestions = SelectorSuggestions::GameMode;
    let start = parser.reader.cursor;
    let inverted = parser.should_invert_value();
    if !parser.gamemode_option.can_parse_element(inverted) {
        return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, "gamemode"));
    }
    let name = parser.reader.read_unquoted_string();
    let Some(mode) = GameType::by_name(&name) else {
        return Err(rollback_with(parser, start, &ERROR_GAME_MODE_INVALID, &name));
    };
    parser.includes_entities = false;
    parser.predicates.push(EntityPredicate::GameMode { mode, inverted });
    parser.gamemode_option.mark_parsed_element(inverted);
    Ok(())
}

fn team(parser: &mut EntitySelectorParser) -> Result<()> {
    let start = parser.reader.cursor;
    let inverted = parser.should_invert_value();
    let name = parser.reader.read_unquoted_string();
    if !parser.team_option.can_parse_element(inverted) {
        return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, "team"));
    }
    parser.predicates.push(EntityPredicate::Team { name, inverted });
    parser.team_option.mark_parsed_element(inverted);
    Ok(())
}

fn entity_type(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.suggestions = SelectorSuggestions::Type;
    let start = parser.reader.cursor;
    let inverted = parser.should_invert_value();
    if parser.is_tag() {
        if !parser.type_option.can_parse_any_tag() {
            return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, "type"));
        }
        let id = read_identifier(parser.reader)?;
        if !parser.type_option.can_parse_tag(&id) {
            return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, "type"));
        }
        parser.predicates.push(EntityPredicate::TypeTag { tag: id.clone(), inverted });
        parser.type_option.mark_parsed_tag(id);
    } else {
        if !parser.type_option.can_parse_element(inverted) {
            return Err(rollback_with(parser, start, &ERROR_INAPPLICABLE_OPTION, "type"));
        }
        let id = read_identifier(parser.reader)?;
        let Some(entity_type) = registry::ENTITY_TYPE.iter().position(|&t| t == id).map(|t| t as i32) else {
            return Err(rollback_with(parser, start, &ERROR_ENTITY_TYPE_INVALID, &id));
        };
        if entity_type == player_type() && !inverted {
            parser.includes_entities = false;
        }
        parser.predicates.push(EntityPredicate::Type { entity_type, inverted });
        if !inverted {
            parser.limit_to_type(entity_type);
        }
        parser.type_option.mark_parsed_element(inverted);
    }
    Ok(())
}

fn tag(parser: &mut EntitySelectorParser) -> Result<()> {
    let inverted = parser.should_invert_value();
    let tag = parser.reader.read_unquoted_string();
    parser.predicates.push(EntityPredicate::Tag { tag, inverted });
    Ok(())
}

fn nbt(parser: &mut EntitySelectorParser) -> Result<()> {
    let inverted = parser.should_invert_value();
    let tag = crate::nbt::parse_compound_as_argument(parser.reader)?;
    parser.predicates.push(EntityPredicate::Nbt { tag, inverted });
    Ok(())
}

fn scores(parser: &mut EntitySelectorParser) -> Result<()> {
    let reader = &mut *parser.reader;
    let mut expected: Vec<(String, Ints)> = Vec::new();
    reader.expect('{')?;
    reader.skip_whitespace();
    while reader.can_read() && reader.peek() != b'}' as u16 {
        reader.skip_whitespace();
        let name = reader.read_unquoted_string();
        reader.skip_whitespace();
        reader.expect('=')?;
        reader.skip_whitespace();
        let value = Ints::from_reader(reader)?;
        expected.retain(|(n, _)| *n != name);
        expected.push((name, value));
        reader.skip_whitespace();
        if reader.at(',') {
            reader.skip();
        }
    }
    reader.expect('}')?;
    if !expected.is_empty() {
        parser.predicates.push(EntityPredicate::Scores(expected));
    }
    parser.scores_option.mark_parsed();
    Ok(())
}

fn advancements(parser: &mut EntitySelectorParser) -> Result<()> {
    let reader = &mut *parser.reader;
    let mut expected: Vec<(String, AdvancementSelectorPredicate)> = Vec::new();
    reader.expect('{')?;
    reader.skip_whitespace();
    while reader.can_read() && reader.peek() != b'}' as u16 {
        reader.skip_whitespace();
        let name = read_identifier(reader)?;
        reader.skip_whitespace();
        reader.expect('=')?;
        reader.skip_whitespace();
        let predicate = if reader.at('{') {
            let mut progress: Vec<(String, bool)> = Vec::new();
            reader.skip_whitespace();
            reader.expect('{')?;
            reader.skip_whitespace();
            while reader.can_read() && reader.peek() != b'}' as u16 {
                reader.skip_whitespace();
                let criterion = reader.read_unquoted_string();
                reader.skip_whitespace();
                reader.expect('=')?;
                reader.skip_whitespace();
                let value = reader.read_boolean()?;
                progress.retain(|(name, _)| *name != criterion);
                progress.push((criterion, value));
                reader.skip_whitespace();
                if reader.at(',') {
                    reader.skip();
                }
            }
            reader.skip_whitespace();
            reader.expect('}')?;
            reader.skip_whitespace();
            AdvancementSelectorPredicate::Criteria(progress)
        } else {
            AdvancementSelectorPredicate::Done(reader.read_boolean()?)
        };
        expected.retain(|(existing, _)| *existing != name);
        expected.push((name, predicate));
        reader.skip_whitespace();
        if reader.at(',') {
            reader.skip();
        }
    }
    reader.expect('}')?;
    if !expected.is_empty() {
        parser.predicates.push(EntityPredicate::Advancements(expected));
        parser.includes_entities = false;
    }
    parser.advancements_option.mark_parsed();
    Ok(())
}

fn predicate(parser: &mut EntitySelectorParser) -> Result<()> {
    parser.should_invert_value();
    read_identifier(parser.reader)?;
    parser.unsupported = Some("predicate");
    Ok(())
}

pub fn suggest_names(parser: &EntitySelectorParser, builder: &mut SuggestionsBuilder) {
    let lower_prefix = builder.remaining().to_lowercase();
    for key in OPTION_NAMES {
        if option(parser, key).is_some_and(|(_, can_use)| can_use) && key.starts_with(&lower_prefix) {
            builder.suggest_with_tooltip(format!("{key}="), Some(Component::translatable(&format!("argument.entity.options.{key}.description"))));
        }
    }
}

pub fn suggest_game_modes(parser: &EntitySelectorParser, builder: &mut SuggestionsBuilder) {
    let state = &parser.gamemode_option;
    let mut prefix = builder.remaining().to_lowercase();
    let mut add_normal = state.can_parse_positive_element();
    let mut add_inverted = state.can_parse_negative_element();
    if !prefix.is_empty() {
        if let Some(rest) = prefix.strip_prefix('!') {
            add_normal = false;
            prefix = rest.to_string();
        } else {
            add_inverted = false;
        }
    }
    for game_type in GameType::VALUES {
        let name = game_type.name();
        if name.starts_with(&prefix) {
            if add_inverted {
                builder.suggest(format!("!{name}"));
            }
            if add_normal {
                builder.suggest(name);
            }
        }
    }
}

pub fn suggest_types(parser: &EntitySelectorParser, builder: &mut SuggestionsBuilder) {
    let state = &parser.type_option;
    if state.can_parse_negative_element() {
        shared_suggestion_provider::suggest_resource_with_prefix(registry::ENTITY_TYPE.iter(), builder, "!");
    }
    if state.can_parse_positive_element() {
        shared_suggestion_provider::suggest_resource(registry::ENTITY_TYPE.iter(), builder);
    }
    if state.can_parse_any_tag() {
        let allowed: Vec<&str> = crisp_data::tags()
            .iter()
            .find(|r| r.registry == "minecraft:entity_type")
            .into_iter()
            .flat_map(|r| r.tags.iter().map(|t| t.0))
            .filter(|tag| state.can_parse_tag(tag))
            .collect();
        if !allowed.is_empty() {
            shared_suggestion_provider::suggest_resource_with_prefix(allowed.iter(), builder, "#");
            shared_suggestion_provider::suggest_resource_with_prefix(allowed.iter(), builder, "!#");
        }
    }
}
