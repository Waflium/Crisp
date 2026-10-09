use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::server::Server;
use crate::world::scores::objective::ObjectiveId;

static ERROR_OBJECTIVE_NOT_FOUND: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("arguments.objective.notFound", vec![name]));
static ERROR_OBJECTIVE_READ_ONLY: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("arguments.objective.readonly", vec![name]));

impl CommandContext {
    pub fn get_objective(&self, server: &Server, name: &str) -> Result<ObjectiveId, CommandSyntaxException> {
        let id = self.get_string(name);
        match server.level.scoreboard.get_objective(id) {
            Some(objective) => Ok(objective.id()),
            None => Err(ERROR_OBJECTIVE_NOT_FOUND.create(id)),
        }
    }

    pub fn get_writable_objective(&self, server: &Server, name: &str) -> Result<ObjectiveId, CommandSyntaxException> {
        let id = self.get_objective(server, name)?;
        let objective = server.level.scoreboard.objective(id);
        if objective.criteria().is_read_only() {
            return Err(ERROR_OBJECTIVE_READ_ONLY.create(objective.name()));
        }
        Ok(id)
    }
}
