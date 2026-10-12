use super::ERROR_INVALID_DIMENSION;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::server::Server;

impl CommandContext {
    pub fn get_dimension(&self, server: &Server, name: &str) -> Result<usize, CommandSyntaxException> {
        let id = self.get_identifier(name);
        server.get_level(id).ok_or_else(|| ERROR_INVALID_DIMENSION.create(id))
    }
}
