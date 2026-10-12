use super::ArgumentValue;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, StringReader, tr};
use crate::server::Server;
use crate::world::scores::display_slot::DisplaySlot;
use crate::world::scores::team_color::TeamColor;

static ERROR_INVALID_SLOT: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.scoreboardDisplaySlot.invalid", vec![value]));
static ERROR_INVALID_COLOR: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.color.invalid", vec![value]));
static ERROR_TEAM_NOT_FOUND: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("team.notFound", vec![name]));

pub fn parse_display_slot(reader: &mut StringReader) -> Result<DisplaySlot, CommandSyntaxException> {
    let name = reader.read_unquoted_string();
    DisplaySlot::by_name(&name).ok_or_else(|| ERROR_INVALID_SLOT.create_with_context(reader, name))
}

pub fn parse_team_color(reader: &mut StringReader) -> Result<TeamColor, CommandSyntaxException> {
    let id = reader.read_unquoted_string();
    TeamColor::by_name(&id).ok_or_else(|| ERROR_INVALID_COLOR.create_with_context(reader, id))
}

impl CommandContext {
    pub fn get_display_slot(&self, name: &str) -> DisplaySlot {
        match self.argument(name) {
            ArgumentValue::DisplaySlot(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_team_color(&self, name: &str) -> TeamColor {
        match self.argument(name) {
            ArgumentValue::TeamColor(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_team(&self, server: &Server, name: &str) -> Result<String, CommandSyntaxException> {
        let id = self.get_string(name);
        match server.scoreboard.borrow().get_player_team(id) {
            Some(team) => Ok(team.name().to_string()),
            None => Err(ERROR_TEAM_NOT_FOUND.create(id)),
        }
    }
}
