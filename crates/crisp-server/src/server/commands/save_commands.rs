use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.save.failed");
static ERROR_ALREADY_ON: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.save.alreadyOn");
static ERROR_ALREADY_OFF: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.save.alreadyOff");

fn save(server: &mut Server, source: &CommandSourceStack, flush: bool) -> Result<i32, CommandSyntaxException> {
    source.send_success(server, || Component::translatable("commands.save.saving"), false);
    if !server.save_everything(true, flush, true) {
        return Err(ERROR_FAILED.create());
    }
    source.send_success(server, || Component::translatable("commands.save.success"), true);
    Ok(1)
}

pub fn save_all(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    save(server, source, false)
}

pub fn save_all_flush(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    save(server, source, true)
}

pub fn save_on(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.set_auto_save(true) {
        return Err(ERROR_ALREADY_ON.create());
    }
    source.send_success(server, || Component::translatable("commands.save.enabled"), true);
    Ok(1)
}

pub fn save_off(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.set_auto_save(false) {
        return Err(ERROR_ALREADY_OFF.create());
    }
    source.send_success(server, || Component::translatable("commands.save.disabled"), true);
    Ok(1)
}
