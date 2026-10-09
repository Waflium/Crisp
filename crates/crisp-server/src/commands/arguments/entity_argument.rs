use super::ArgumentValue;
use super::selector::entity_selector::EntitySelector;
use super::selector::entity_selector_parser::EntitySelectorParser;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::world::entity::EntityId;

pub static ERROR_NOT_SINGLE_ENTITY: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.toomany");
pub static ERROR_NOT_SINGLE_PLAYER: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.player.toomany");
pub static ERROR_ONLY_PLAYERS_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.player.entities");
pub static NO_ENTITIES_FOUND: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.notfound.entity");
pub static NO_PLAYERS_FOUND: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.notfound.player");
pub static ERROR_SELECTORS_NOT_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.entity.selector.not_allowed");

pub fn parse(reader: &mut StringReader, single: bool, players_only: bool, allow_selectors: bool) -> Result<EntitySelector, CommandSyntaxException> {
    let selector = EntitySelectorParser::new(reader, allow_selectors).parse()?;
    if selector.max_results > 1 && single {
        reader.cursor = 0;
        if players_only {
            return Err(ERROR_NOT_SINGLE_PLAYER.create_with_context(reader));
        }
        return Err(ERROR_NOT_SINGLE_ENTITY.create_with_context(reader));
    }
    if selector.includes_entities && players_only && !selector.is_self_selector() {
        reader.cursor = 0;
        return Err(ERROR_ONLY_PLAYERS_ALLOWED.create_with_context(reader));
    }
    Ok(selector)
}

impl CommandContext {
    fn selector(&self, name: &str) -> &EntitySelector {
        match self.argument(name) {
            ArgumentValue::EntitySelector(selector) => selector,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_entity(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<EntityId, CommandSyntaxException> {
        self.selector(name).find_single_entity(server, source)
    }

    pub fn get_optional_entities(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Vec<EntityId>, CommandSyntaxException> {
        self.selector(name).find_entities(server, source)
    }

    pub fn get_entities(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Vec<EntityId>, CommandSyntaxException> {
        let result = self.selector(name).find_entities(server, source)?;
        if result.is_empty() {
            return Err(NO_ENTITIES_FOUND.create());
        }
        Ok(result)
    }

    pub fn get_optional_players(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Vec<EntityId>, CommandSyntaxException> {
        self.selector(name).find_players(server, source)
    }

    pub fn get_players(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Vec<EntityId>, CommandSyntaxException> {
        let players = self.selector(name).find_players(server, source)?;
        if players.is_empty() {
            return Err(NO_PLAYERS_FOUND.create());
        }
        Ok(players)
    }
}
