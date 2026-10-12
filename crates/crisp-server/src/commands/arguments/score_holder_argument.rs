use super::ArgumentValue;
use super::entity_argument::{ERROR_NOT_SINGLE_ENTITY, NO_ENTITIES_FOUND};
use super::java_uuid_from_string;
use super::selector::entity_selector::{EntitySelector, player_by_name};
use super::selector::entity_selector_parser::EntitySelectorParser;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::world::scores::score_holder::ScoreHolder;

pub static ERROR_NO_RESULTS: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.scoreHolder.empty");

#[derive(Clone, Debug, PartialEq)]
pub enum ScoreHolderResult {
    Selector(Box<EntitySelector>),
    Wildcard,
    NameOnly(String),
    Uuid { text: String, uuid: u128 },
    Name(String),
}

pub fn parse(reader: &mut StringReader, multiple: bool, allow_selectors: bool) -> Result<ScoreHolderResult, CommandSyntaxException> {
    if reader.can_read() && reader.peek() == b'@' as u16 {
        let selector = EntitySelectorParser::new(reader, allow_selectors).parse()?;
        if !multiple && selector.max_results > 1 {
            return Err(ERROR_NOT_SINGLE_ENTITY.create_with_context(reader));
        }
        return Ok(ScoreHolderResult::Selector(Box::new(selector)));
    }
    let start = reader.cursor;
    while reader.can_read() && reader.peek() != b' ' as u16 {
        reader.skip();
    }
    let text = reader.substring(start, reader.cursor);
    if text == ScoreHolder::WILDCARD_NAME {
        return Ok(ScoreHolderResult::Wildcard);
    }
    if text.starts_with('#') {
        return Ok(ScoreHolderResult::NameOnly(text));
    }
    Ok(match java_uuid_from_string(&text) {
        Some(uuid) => ScoreHolderResult::Uuid { text, uuid },
        None => ScoreHolderResult::Name(text),
    })
}

impl ScoreHolderResult {
    fn names(&self, server: &Server, source: &CommandSourceStack, wildcard: impl FnOnce() -> Vec<ScoreHolder>) -> Result<Vec<ScoreHolder>, CommandSyntaxException> {
        Ok(match self {
            ScoreHolderResult::Selector(selector) => {
                let entities = selector.find_entities(server, source)?;
                if entities.is_empty() {
                    return Err(NO_ENTITIES_FOUND.create());
                }
                entities.into_iter().map(|e| ScoreHolder::for_entity(&server.level, e)).collect()
            }
            ScoreHolderResult::Wildcard => {
                let results = wildcard();
                if results.is_empty() {
                    return Err(ERROR_NO_RESULTS.create());
                }
                results
            }
            ScoreHolderResult::NameOnly(name) => vec![ScoreHolder::for_name_only(name.clone())],
            ScoreHolderResult::Uuid { text, uuid } => {
                let level = &server.level;
                match level.all_entities().find(|&id| level.entity(id).uuid == *uuid) {
                    Some(entity) => vec![ScoreHolder::for_entity(level, entity)],
                    None => vec![ScoreHolder::for_name_only(text.clone())],
                }
            }
            ScoreHolderResult::Name(name) => match player_by_name(server, name) {
                Some(player) => vec![ScoreHolder::for_entity(&server.level, player)],
                None => vec![ScoreHolder::for_name_only(name.clone())],
            },
        })
    }
}

impl CommandContext {
    fn score_holder(&self, name: &str) -> &ScoreHolderResult {
        match self.argument(name) {
            ArgumentValue::ScoreHolder(result) => result,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_score_holders_with(
        &self,
        server: &Server,
        source: &CommandSourceStack,
        name: &str,
        wildcard: impl FnOnce() -> Vec<ScoreHolder>,
    ) -> Result<Vec<ScoreHolder>, CommandSyntaxException> {
        let result = self.score_holder(name).names(server, source, wildcard)?;
        if result.is_empty() {
            return Err(NO_ENTITIES_FOUND.create());
        }
        Ok(result)
    }

    pub fn get_score_holder(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<ScoreHolder, CommandSyntaxException> {
        Ok(self.get_score_holders_with(server, source, name, Vec::new)?.swap_remove(0))
    }

    pub fn get_score_holders(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Vec<ScoreHolder>, CommandSyntaxException> {
        self.get_score_holders_with(server, source, name, || server.scoreboard.borrow().tracked_players())
    }
}

pub fn suggest_score_holders(
    server: &Server,
    _context: &crate::commands::brigadier::CommandContext,
    source: &CommandSourceStack,
    builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let names: Vec<String> = server.level.players.values().map(|p| p.profile.name.clone()).collect();
    Ok(super::selector_suggestions(&builder, source, |b| crate::commands::shared_suggestion_provider::suggest(names, b)))
}
