use crisp_core::AABB;
use crisp_core::mth;

use super::entity_selector::{EntityPredicate, EntitySelector, Order, player_type};
use super::options::entity_selector_options;
use super::options::invertable_set_option_state::InvertableSetOptionState;
use super::options::set_once_option_state::SetOnceOptionState;
use crate::advancements::predicates::min_max_bounds::{Doubles, FloatDegrees, Ints};
use crate::commands::arguments::java_uuid_from_string;
use crate::commands::brigadier::{CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, StringReader, tr, utf16};
use crate::commands::shared_suggestion_provider;
use crate::commands::source::CommandSourceStack;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::network::chat::Component;

pub static ERROR_INVALID_NAME_OR_UUID: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.invalid");
pub const UNKNOWN_SELECTOR_TYPE_KEY: &str = "argument.entity.selector.unknown";
pub static ERROR_UNKNOWN_SELECTOR_TYPE: DynamicCommandExceptionType = DynamicCommandExceptionType(|t| tr(UNKNOWN_SELECTOR_TYPE_KEY, vec![t]));
pub static ERROR_SELECTORS_NOT_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.selector.not_allowed");
pub static ERROR_MISSING_SELECTOR_TYPE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.selector.missing");
pub static ERROR_EXPECTED_END_OF_OPTIONS: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.options.unterminated");
pub static ERROR_EXPECTED_OPTION_VALUE: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("argument.entity.options.valueless", vec![name]));

pub fn allow_selectors(source: &CommandSourceStack) -> bool {
    source.has_permission(2)
}

pub struct EntitySelectorParser<'r, 'a> {
    pub reader: &'r mut StringReader<'a>,
    allow_selectors: bool,
    pub max_results: i32,
    pub includes_entities: bool,
    pub world_limited: bool,
    pub distance: Option<Doubles>,
    pub level: Option<Ints>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub delta_x: Option<f64>,
    pub delta_y: Option<f64>,
    pub delta_z: Option<f64>,
    pub rot_x: Option<FloatDegrees>,
    pub rot_y: Option<FloatDegrees>,
    pub predicates: Vec<EntityPredicate>,
    pub order: Order,
    pub current_entity: bool,
    player_name: Option<String>,
    entity_uuid: Option<u128>,
    pub name_option: InvertableSetOptionState,
    pub limited_option: SetOnceOptionState,
    pub sorted_option: SetOnceOptionState,
    pub gamemode_option: InvertableSetOptionState,
    pub team_option: InvertableSetOptionState,
    pub entity_type: Option<i32>,
    pub type_option: InvertableSetOptionState,
    pub scores_option: SetOnceOptionState,
    pub advancements_option: SetOnceOptionState,
    uses_selectors: bool,
    pub unsupported: Option<&'static str>,
    start_position: usize,
    pub suggestions: SelectorSuggestions,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectorSuggestions {
    Nothing,
    NameOrSelector,
    Name,
    Selector,
    OpenOptions,
    OptionsKeyOrClose,
    OptionsKey,
    OptionsNextOrClose,
    Sort,
    GameMode,
    Type,
}

impl<'r, 'a> EntitySelectorParser<'r, 'a> {
    pub fn new(reader: &'r mut StringReader<'a>, allow_selectors: bool) -> EntitySelectorParser<'r, 'a> {
        EntitySelectorParser {
            reader,
            allow_selectors,
            max_results: 0,
            includes_entities: false,
            world_limited: false,
            distance: None,
            level: None,
            x: None,
            y: None,
            z: None,
            delta_x: None,
            delta_y: None,
            delta_z: None,
            rot_x: None,
            rot_y: None,
            predicates: Vec::new(),
            order: Order::Arbitrary,
            current_entity: false,
            player_name: None,
            entity_uuid: None,
            name_option: InvertableSetOptionState::default(),
            limited_option: SetOnceOptionState::default(),
            sorted_option: SetOnceOptionState::default(),
            gamemode_option: InvertableSetOptionState::default(),
            team_option: InvertableSetOptionState::default(),
            entity_type: None,
            type_option: InvertableSetOptionState::default(),
            scores_option: SetOnceOptionState::default(),
            advancements_option: SetOnceOptionState::default(),
            uses_selectors: false,
            unsupported: None,
            start_position: 0,
            suggestions: SelectorSuggestions::Nothing,
        }
    }

    fn get_selector(&mut self) -> EntitySelector {
        let aabb = if self.delta_x.is_none() && self.delta_y.is_none() && self.delta_z.is_none() {
            self.distance.and_then(|d| d.bounds.max).map(|max_range| AABB::new(-max_range, -max_range, -max_range, max_range + 1.0, max_range + 1.0, max_range + 1.0))
        } else {
            Some(create_aabb(self.delta_x.unwrap_or(0.0), self.delta_y.unwrap_or(0.0), self.delta_z.unwrap_or(0.0)))
        };
        EntitySelector {
            max_results: self.max_results,
            includes_entities: self.includes_entities,
            world_limited: self.world_limited,
            predicates: self.predicates.clone(),
            range: self.distance,
            position: (self.x, self.y, self.z),
            aabb,
            order: self.order,
            current_entity: self.current_entity,
            player_name: self.player_name.clone(),
            entity_uuid: self.entity_uuid,
            entity_type: self.entity_type,
            uses_selector: self.uses_selectors,
            unsupported: self.unsupported,
        }
    }

    fn finalize_predicates(&mut self) {
        if let Some(range) = self.rot_x {
            self.predicates.push(rotation_predicate(range, true));
        }
        if let Some(range) = self.rot_y {
            self.predicates.push(rotation_predicate(range, false));
        }
        if let Some(level) = self.level {
            self.predicates.push(EntityPredicate::Level(level));
        }
    }

    fn parse_selector(&mut self) -> Result<(), CommandSyntaxException> {
        self.uses_selectors = true;
        self.suggestions = SelectorSuggestions::Selector;
        if !self.reader.can_read() {
            return Err(ERROR_MISSING_SELECTOR_TYPE.create_with_context(self.reader));
        }
        let start = self.reader.cursor;
        let selector_type = self.reader.read();
        let alive = match char::from_u32(selector_type as u32) {
            Some('a') => {
                self.max_results = i32::MAX;
                self.includes_entities = false;
                self.order = Order::Arbitrary;
                self.limit_to_type(player_type());
                false
            }
            Some('e') => {
                self.max_results = i32::MAX;
                self.includes_entities = true;
                self.order = Order::Arbitrary;
                true
            }
            Some('n') => {
                self.max_results = 1;
                self.includes_entities = true;
                self.order = Order::Nearest;
                true
            }
            Some('p') => {
                self.max_results = 1;
                self.includes_entities = false;
                self.order = Order::Nearest;
                self.limit_to_type(player_type());
                false
            }
            Some('r') => {
                self.max_results = 1;
                self.includes_entities = false;
                self.order = Order::Random;
                self.limit_to_type(player_type());
                false
            }
            Some('s') => {
                self.max_results = 1;
                self.includes_entities = true;
                self.current_entity = true;
                false
            }
            _ => {
                self.reader.cursor = start;
                return Err(ERROR_UNKNOWN_SELECTOR_TYPE.create_with_context(self.reader, format!("@{}", utf16(&[selector_type]))));
            }
        };
        if alive {
            self.predicates.push(EntityPredicate::Alive);
        }
        self.suggestions = SelectorSuggestions::OpenOptions;
        if self.reader.at('[') {
            self.reader.skip();
            self.suggestions = SelectorSuggestions::OptionsKeyOrClose;
            self.parse_options()?;
        }
        Ok(())
    }

    fn parse_name_or_uuid(&mut self) -> Result<(), CommandSyntaxException> {
        if self.reader.can_read() {
            self.suggestions = SelectorSuggestions::Name;
        }
        let start = self.reader.cursor;
        let name = self.reader.read_string()?;
        match java_uuid_from_string(&name) {
            Some(uuid) => {
                self.entity_uuid = Some(uuid);
                self.includes_entities = true;
            }
            None => {
                if name.is_empty() || name.encode_utf16().count() > 16 {
                    self.reader.cursor = start;
                    return Err(ERROR_INVALID_NAME_OR_UUID.create_with_context(self.reader));
                }
                self.includes_entities = false;
                self.player_name = Some(name);
            }
        }
        self.max_results = 1;
        Ok(())
    }

    fn parse_options(&mut self) -> Result<(), CommandSyntaxException> {
        self.suggestions = SelectorSuggestions::OptionsKey;
        self.reader.skip_whitespace();
        while self.reader.can_read() && self.reader.peek() != b']' as u16 {
            self.reader.skip_whitespace();
            let start = self.reader.cursor;
            let key = self.reader.read_string()?;
            let modifier = entity_selector_options::get(self, &key, start)?;
            self.reader.skip_whitespace();
            if !self.reader.at('=') {
                self.reader.cursor = start;
                return Err(ERROR_EXPECTED_OPTION_VALUE.create_with_context(self.reader, key));
            }
            self.reader.skip();
            self.reader.skip_whitespace();
            self.suggestions = SelectorSuggestions::Nothing;
            modifier(self)?;
            self.reader.skip_whitespace();
            self.suggestions = SelectorSuggestions::OptionsNextOrClose;
            if self.reader.can_read() {
                if self.reader.peek() != b',' as u16 {
                    if self.reader.peek() != b']' as u16 {
                        return Err(ERROR_EXPECTED_END_OF_OPTIONS.create_with_context(self.reader));
                    }
                    break;
                }
                self.reader.skip();
                self.suggestions = SelectorSuggestions::OptionsKey;
            }
        }
        if self.reader.can_read() {
            self.reader.skip();
            self.suggestions = SelectorSuggestions::Nothing;
            Ok(())
        } else {
            Err(ERROR_EXPECTED_END_OF_OPTIONS.create_with_context(self.reader))
        }
    }

    pub fn should_invert_value(&mut self) -> bool {
        self.reader.skip_whitespace();
        if self.reader.at('!') {
            self.reader.skip();
            self.reader.skip_whitespace();
            true
        } else {
            false
        }
    }

    pub fn is_tag(&mut self) -> bool {
        self.reader.skip_whitespace();
        if self.reader.at('#') {
            self.reader.skip();
            self.reader.skip_whitespace();
            true
        } else {
            false
        }
    }

    pub fn limit_to_type(&mut self, entity_type: i32) {
        self.entity_type = Some(entity_type);
    }

    pub fn parse(&mut self) -> Result<EntitySelector, CommandSyntaxException> {
        self.start_position = self.reader.cursor;
        self.suggestions = SelectorSuggestions::NameOrSelector;
        if self.reader.at('@') {
            if !self.allow_selectors {
                return Err(ERROR_SELECTORS_NOT_ALLOWED.create_with_context(self.reader));
            }
            self.reader.skip();
            self.parse_selector()?;
        } else {
            self.parse_name_or_uuid()?;
        }
        self.finalize_predicates();
        Ok(self.get_selector())
    }
}

impl EntitySelectorParser<'_, '_> {
    pub fn fill_suggestions(&self, builder: &SuggestionsBuilder, names: impl FnOnce(&mut SuggestionsBuilder)) -> Suggestions {
        let mut builder = builder.create_offset(self.reader.cursor);
        match self.suggestions {
            SelectorSuggestions::Nothing => {}
            SelectorSuggestions::NameOrSelector => {
                names(&mut builder);
                if self.allow_selectors {
                    fill_selector_suggestions(&mut builder);
                }
            }
            SelectorSuggestions::Name => {
                let mut sub = builder.create_offset(self.start_position);
                names(&mut sub);
                builder.add(sub);
            }
            SelectorSuggestions::Selector => {
                let mut sub = builder.create_offset(builder.start() - 1);
                fill_selector_suggestions(&mut sub);
                builder.add(sub);
            }
            SelectorSuggestions::OpenOptions => builder.suggest("["),
            SelectorSuggestions::OptionsKeyOrClose => {
                builder.suggest("]");
                entity_selector_options::suggest_names(self, &mut builder);
            }
            SelectorSuggestions::OptionsKey => entity_selector_options::suggest_names(self, &mut builder),
            SelectorSuggestions::OptionsNextOrClose => {
                builder.suggest(",");
                builder.suggest("]");
            }
            SelectorSuggestions::Sort => shared_suggestion_provider::suggest(["nearest", "furthest", "random", "arbitrary"], &mut builder),
            SelectorSuggestions::GameMode => entity_selector_options::suggest_game_modes(self, &mut builder),
            SelectorSuggestions::Type => entity_selector_options::suggest_types(self, &mut builder),
        }
        builder.build()
    }
}

fn fill_selector_suggestions(builder: &mut SuggestionsBuilder) {
    for (selector, key) in [
        ("@p", "argument.entity.selector.nearestPlayer"),
        ("@a", "argument.entity.selector.allPlayers"),
        ("@r", "argument.entity.selector.randomPlayer"),
        ("@s", "argument.entity.selector.self"),
        ("@e", "argument.entity.selector.allEntities"),
        ("@n", "argument.entity.selector.nearestEntity"),
    ] {
        builder.suggest_with_tooltip(selector, Some(Component::translatable(key)));
    }
}

fn create_aabb(x: f64, y: f64, z: f64) -> AABB {
    let (x_neg, y_neg, z_neg) = (x < 0.0, y < 0.0, z < 0.0);
    let min_x = if x_neg { x } else { 0.0 };
    let min_y = if y_neg { y } else { 0.0 };
    let min_z = if z_neg { z } else { 0.0 };
    let max_x = (if x_neg { 0.0 } else { x }) + 1.0;
    let max_y = (if y_neg { 0.0 } else { y }) + 1.0;
    let max_z = (if z_neg { 0.0 } else { z }) + 1.0;
    AABB::new(min_x, min_y, min_z, max_x, max_y, max_z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str, allow_selectors: bool) -> Result<(EntitySelector, usize), String> {
        let units: Vec<u16> = input.encode_utf16().collect();
        let mut reader = StringReader::new(&units);
        let result = EntitySelectorParser::new(&mut reader, allow_selectors).parse();
        result.map(|s| (s, reader.cursor)).map_err(|e| e.get_message())
    }

    #[test]
    fn selectors() {
        let (all, end) = parse("@a", true).unwrap();
        assert_eq!((all.max_results, all.includes_entities, all.order, all.entity_type, end), (i32::MAX, false, Order::Arbitrary, Some(player_type()), 2));
        let (nearest, _) = parse("@n", true).unwrap();
        assert_eq!((nearest.max_results, nearest.includes_entities, nearest.order, &nearest.predicates[..]), (1, true, Order::Nearest, &[EntityPredicate::Alive][..]));
        let (me, _) = parse("@s", true).unwrap();
        assert!(me.is_self_selector() && me.includes_entities && me.predicates.is_empty());
        let (random, _) = parse("@r[limit=3] rest", true).unwrap();
        assert_eq!((random.max_results, random.order), (3, Order::Random));
        let (options, end) = parse("@e[ type = !item , tag= , x_rotation=..10, distance=..2]", true).unwrap();
        assert_eq!(end, 56);
        assert!(options.world_limited);
        assert_eq!(options.aabb, Some(AABB::new(-2.0, -2.0, -2.0, 3.0, 3.0, 3.0)));
        let (boxed, _) = parse("@e[dx=-2,dz=3,y=5]", true).unwrap();
        assert_eq!((boxed.aabb, boxed.position), (Some(AABB::new(-2.0, 0.0, 0.0, 1.0, 1.0, 4.0)), (None, Some(5.0), None)));
        let (players, _) = parse("@e[type=player]", true).unwrap();
        assert!(!players.includes_entities);
        let (levels, _) = parse("@e[level=1..]", true).unwrap();
        assert!(!levels.includes_entities);
    }

    #[test]
    fn names_and_uuids() {
        let (name, end) = parse("Steve rest", true).unwrap();
        assert_eq!((name.player_name.as_deref(), name.max_results, name.includes_entities, end), (Some("Steve"), 1, false, 5));
        let (uuid, _) = parse("dd12be42-52a9-4a91-a8a1-11c01849e498", true).unwrap();
        assert_eq!((uuid.entity_uuid, uuid.includes_entities), (Some(0xdd12be42_52a9_4a91_a8a1_11c01849e498), true));
        let (quoted, _) = parse("\"Steve\"", true).unwrap();
        assert_eq!(quoted.player_name.as_deref(), Some("Steve"));
        assert_eq!(parse("@p", false).unwrap_err(), "Selector not allowed at position 0: <--[HERE]");
        assert!(parse("Steve", false).is_ok());
    }

    const VANILLA: &[(&str, &str)] = &[
        ("@a", "OK 2 max=2147483647 ents=false"),
        ("@r[limit=3] rest", "OK 11 max=3 ents=false"),
        ("@e[ type = !item , tag= , x_rotation=..10, distance=..2]", "OK 56 max=2147483647 ents=true"),
        ("@e[dx=-2,dz=3,y=5]", "OK 18 max=2147483647 ents=true"),
        ("@e[type=player]", "OK 15 max=2147483647 ents=false"),
        ("@e[level=1..]", "OK 13 max=2147483647 ents=false"),
        ("Steve rest", "OK 5 max=1 ents=false"),
        ("dd12be42-52a9-4a91-a8a1-11c01849e498", "OK 36 max=1 ents=true"),
        ("\"Steve\"", "OK 7 max=1 ents=false"),
        ("", "Invalid name or UUID at position 0: <--[HERE]"),
        ("abcdefghijklmnopq", "Invalid name or UUID at position 0: <--[HERE]"),
        ("\"Steve", "Unclosed quoted string at position 6: \"Steve<--[HERE]"),
        ("@", "Missing selector type at position 1: @<--[HERE]"),
        ("@x", "Unknown selector type '@x' at position 1: @<--[HERE]"),
        ("@e[", "Expected end of options at position 3: @e[<--[HERE]"),
        ("@e[x=1", "Expected end of options at position 6: @e[x=1<--[HERE]"),
        ("@e[x=1;", "Expected end of options at position 6: @e[x=1<--[HERE]"),
        ("@e[foo=1]", "Unknown option 'foo' at position 3: @e[<--[HERE]"),
        ("@e[limit]", "Expected value for option 'limit' at position 3: @e[<--[HERE]"),
        ("@e[limit=1,limit=2]", "Option 'limit' isn't applicable here at position 11: ...e[limit=1,<--[HERE]"),
        ("@s[sort=nearest]", "Option 'sort' isn't applicable here at position 3: @s[<--[HERE]"),
        ("@e[limit=0]", "Limit must be at least 1 at position 9: @e[limit=<--[HERE]"),
        ("@e[limit=]", "Expected integer at position 9: @e[limit=<--[HERE]"),
        ("@e[distance=-1]", "Distance cannot be negative at position 12: ...[distance=<--[HERE]"),
        ("@e[distance=5..1]", "Min cannot be bigger than max at position 12: ...[distance=<--[HERE]"),
        ("@e[distance=]", "Expected value or range of values at position 12: ...[distance=<--[HERE]"),
        ("@e[distance=1..2..3]", "Expected end of options at position 16: ...tance=1..2<--[HERE]"),
        ("@e[level=-1]", "Level shouldn't be negative at position 9: @e[level=<--[HERE]"),
        ("@e[sort=foo]", "Invalid or unknown sort type 'foo' at position 8: @e[sort=<--[HERE]"),
        ("@e[gamemode=foo]", "Invalid or unknown game mode 'foo' at position 12: ...[gamemode=<--[HERE]"),
        ("@a[gamemode=survival,gamemode=creative]", "Option 'gamemode' isn't applicable here at position 21: ...=survival,<--[HERE]"),
        ("@a[gamemode=!survival,gamemode=!creative]", "OK 41 max=2147483647 ents=false"),
        ("@e[type=foo]", "Invalid or unknown entity type 'minecraft:foo' at position 8: @e[type=<--[HERE]"),
        ("@e[type=item,type=pig]", "Option 'type' isn't applicable here at position 13: ...type=item,<--[HERE]"),
        ("@e[type=!item,type=pig]", "Option 'type' isn't applicable here at position 19: ...item,type=<--[HERE]"),
        ("@e[type=#skeletons,type=#skeletons]", "Option 'type' isn't applicable here at position 24: ...tons,type=<--[HERE]"),
        ("@e[type=Item]", "Invalid or unknown entity type 'minecraft:' at position 8: @e[type=<--[HERE]"),
        ("@a[name=Bob,name=Al]", "Option 'name' isn't applicable here at position 12: ...[name=Bob,<--[HERE]"),
        ("@e[team=a,team=b]", "Option 'team' isn't applicable here at position 10: @e[team=a,<--[HERE]"),
        ("@e[scores={a}]", "Expected '=' at position 12: ...[scores={a<--[HERE]"),
        ("@e[advancements={a=maybe}]", "Invalid boolean: expected 'true' or 'false' but found 'maybe' at position 19: ...ements={a=<--[HERE]"),
        ("@e[predicate=A]", "Expected end of options at position 13: ...predicate=<--[HERE]"),
        ("@a[advancements={story/root=true}]", "OK 34 max=2147483647 ents=false"),
        ("@e[x_rotation=1.2.3]", "Invalid float '1.2.3' at position 14: ..._rotation=<--[HERE]"),
        ("@e[name=!]", "OK 10 max=2147483647 ents=true"),
        ("@e[name=\"a b\",name=!c]", "Option 'name' isn't applicable here at position 14: ...ame=\"a b\",<--[HERE]"),
        ("@e[tag=!]", "OK 9 max=2147483647 ents=true"),
        ("@e[,]", "Unknown option '' at position 3: @e[<--[HERE]"),
        ("@e[]", "OK 4 max=2147483647 ents=true"),
        ("@e[ ]", "OK 5 max=2147483647 ents=true"),
        ("@e[x=1,]", "OK 8 max=2147483647 ents=true"),
        ("@e[limit=1, ]", "Unknown option '' at position 12: ...[limit=1, <--[HERE]"),
        ("@e[gamemode=!]", "Invalid or unknown game mode '' at position 12: ...[gamemode=<--[HERE]"),
        ("@e[type=!]", "Invalid or unknown entity type 'minecraft:' at position 8: @e[type=<--[HERE]"),
        ("@e[distance=..-1]", "Distance cannot be negative at position 12: ...[distance=<--[HERE]"),
        ("@e[level=1.5]", "Invalid integer '1.5' at position 9: @e[level=<--[HERE]"),
        ("@e[x=--1]", "Invalid double '--1' at position 5: @e[x=<--[HERE]"),
        ("@e[y_rotation=10..5]", "OK 20 max=2147483647 ents=true"),
        ("@e[scores={}]", "OK 13 max=2147483647 ents=true"),
        ("@e[scores={a=1,b=..2}]", "OK 22 max=2147483647 ents=true"),
        ("@e[advancements={a={b=true,c=false}}]", "OK 37 max=2147483647 ents=false"),
        ("@e[advancements={a={b}}]", "Expected '=' at position 21: ...ents={a={b<--[HERE]"),
        ("@e[sort=]", "Invalid or unknown sort type '' at position 8: @e[sort=<--[HERE]"),
        ("12345678901234567", "Invalid name or UUID at position 0: <--[HERE]"),
        ("1-2-3-4-5", "OK 9 max=1 ents=true"),
        ("@é", "Unknown selector type '@é' at position 1: @<--[HERE]"),
    ];

    #[test]
    fn parses_like_vanilla() {
        for &(input, expected) in VANILLA {
            let actual = match parse(input, true) {
                Ok((s, cursor)) => format!("OK {cursor} max={} ents={}", s.max_results, s.includes_entities),
                Err(e) => e,
            };
            assert_eq!(actual, expected, "{input}");
        }
    }

    #[test]
    fn unsupported_options() {
        let (advancements, _) = parse("@a[advancements={story/root=true}]", true).unwrap();
        assert_eq!(advancements.unsupported, None);
        let (predicate, _) = parse("@a[predicate=test:x]", true).unwrap();
        assert_eq!(predicate.unsupported, Some("predicate"));
        let (empty, _) = parse("@a[advancements={}]", true).unwrap();
        assert_eq!(empty.unsupported, None);
        assert!(parse("@e[nbt={}]", true).is_ok());
    }
}

fn rotation_predicate(range: FloatDegrees, x: bool) -> EntityPredicate {
    let min = mth::wrap_degrees(range.bounds.min.unwrap_or(0.0));
    let max = mth::wrap_degrees(range.bounds.max.unwrap_or(359.0));
    EntityPredicate::Rotation { min, max, x }
}
