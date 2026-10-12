use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::world::entity::EntityId;

pub static ERROR_NOT_A_WAYPOINT: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.waypoint.invalid");

impl CommandContext {
    pub fn get_waypoint(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<EntityId, CommandSyntaxException> {
        let entity = self.get_entity(server, source, name)?;
        if server.entity_level(entity).is_living(entity) { Ok(entity) } else { Err(ERROR_NOT_A_WAYPOINT.create()) }
    }
}
