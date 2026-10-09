use crisp_core::mth;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::commands::CommandFn;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::util::java_format_fixed;
use crate::world::level::border::world_border::{MAX_CENTER_COORDINATE, MAX_SIZE};

static ERROR_SAME_CENTER: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.center.failed");
static ERROR_SAME_SIZE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.set.failed.nochange");
static ERROR_TOO_SMALL: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.set.failed.small");
static ERROR_SAME_WARNING_TIME: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.warning.time.failed");
static ERROR_SAME_WARNING_DISTANCE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.warning.distance.failed");
static ERROR_SAME_DAMAGE_BUFFER: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.damage.buffer.failed");
static ERROR_SAME_DAMAGE_AMOUNT: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.worldborder.damage.amount.failed");

fn error_too_big() -> CommandSyntaxException {
    CommandSyntaxException::new(Component::translatable_with("commands.worldborder.set.failed.big", vec![Arg::Float(MAX_SIZE as f32)]))
}

fn error_too_far_out() -> CommandSyntaxException {
    CommandSyntaxException::new(Component::translatable_with("commands.worldborder.set.failed.far", vec![Arg::Double(MAX_CENTER_COORDINATE)]))
}

pub fn executors() -> Vec<(String, CommandFn)> {
    let table: &[(&str, CommandFn)] = &[
        ("worldborder add distance", add),
        ("worldborder add distance time", add),
        ("worldborder set distance", set),
        ("worldborder set distance time", set),
        ("worldborder center pos", center),
        ("worldborder damage amount damagePerBlock", damage_amount),
        ("worldborder damage buffer distance", damage_buffer),
        ("worldborder get", get),
        ("worldborder warning distance distance", warning_distance),
        ("worldborder warning time time", warning_time),
    ];
    table.iter().map(|&(path, f)| (path.to_string(), f)).collect()
}

fn add(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let border = &server.level.world_border;
    let distance = border.get_size() + context.get_double("distance");
    let ticks = if context.has_argument("time") { border.get_lerp_time() + context.get_int("time") as i64 } else { 0 };
    set_size(server, source, distance, ticks)
}

fn set(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let ticks = if context.has_argument("time") { context.get_int("time") as i64 } else { 0 };
    set_size(server, source, context.get_double("distance"), ticks)
}

fn center(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = context.get_coordinates("pos").position(server, source);
    let (x, z) = (pos.x as f32, pos.z as f32);
    let border = &mut server.level.world_border;
    if border.get_center_x() == x as f64 && border.get_center_z() == z as f64 {
        return Err(ERROR_SAME_CENTER.create());
    }
    if (x as f64).abs() > MAX_CENTER_COORDINATE || (z as f64).abs() > MAX_CENTER_COORDINATE {
        return Err(error_too_far_out());
    }
    border.set_center(x as f64, z as f64);
    let args = vec![java_format_fixed(x as f64, 2).into(), java_format_fixed(z as f64, 2).into()];
    source.send_success(server, || Component::translatable_with("commands.worldborder.center.success", args), true);
    Ok(0)
}

fn damage_amount(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let damage_per_block = context.get_float("damagePerBlock");
    let border = &mut server.level.world_border;
    if border.get_damage_per_block() == damage_per_block as f64 {
        return Err(ERROR_SAME_DAMAGE_AMOUNT.create());
    }
    border.set_damage_per_block(damage_per_block as f64);
    let amount = java_format_fixed(damage_per_block as f64, 2);
    source.send_success(server, || Component::translatable_with("commands.worldborder.damage.amount.success", vec![amount.into()]), true);
    Ok(damage_per_block as i32)
}

fn damage_buffer(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let distance = context.get_float("distance");
    let border = &mut server.level.world_border;
    if border.get_safe_zone() == distance as f64 {
        return Err(ERROR_SAME_DAMAGE_BUFFER.create());
    }
    border.set_safe_zone(distance as f64);
    let buffer = java_format_fixed(distance as f64, 2);
    source.send_success(server, || Component::translatable_with("commands.worldborder.damage.buffer.success", vec![buffer.into()]), true);
    Ok(distance as i32)
}

fn get(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let size = server.level.world_border.get_size();
    source.send_success(server, || Component::translatable_with("commands.worldborder.get", vec![java_format_fixed(size, 0).into()]), false);
    Ok(mth::floor(size + 0.5))
}

fn warning_distance(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let distance = context.get_int("distance");
    let border = &mut server.level.world_border;
    if border.get_warning_blocks() == distance {
        return Err(ERROR_SAME_WARNING_DISTANCE.create());
    }
    border.set_warning_blocks(distance);
    source.send_success(server, || Component::translatable_with("commands.worldborder.warning.distance.success", vec![Arg::Int(distance)]), true);
    Ok(distance)
}

fn warning_time(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let ticks = context.get_int("time");
    let border = &mut server.level.world_border;
    if border.get_warning_time() == ticks {
        return Err(ERROR_SAME_WARNING_TIME.create());
    }
    border.set_warning_time(ticks);
    source.send_success(server, || Component::translatable_with("commands.worldborder.warning.time.success", vec![format_ticks_to_seconds(ticks as i64).into()]), true);
    Ok(ticks)
}

fn set_size(server: &mut Server, source: &CommandSourceStack, distance: f64, ticks: i64) -> Result<i32, CommandSyntaxException> {
    let border = &mut server.level.world_border;
    let current = border.get_size();
    if current == distance {
        return Err(ERROR_SAME_SIZE.create());
    }
    if distance < 1.0 {
        return Err(ERROR_TOO_SMALL.create());
    }
    if distance > MAX_SIZE {
        return Err(error_too_big());
    }
    let formatted_distance = java_format_fixed(distance, 1);
    if ticks > 0 {
        border.lerp_size_between(current, distance, ticks);
        let key = if distance > current { "commands.worldborder.set.grow" } else { "commands.worldborder.set.shrink" };
        source.send_success(server, || Component::translatable_with(key, vec![formatted_distance.into(), format_ticks_to_seconds(ticks).into()]), true);
    } else {
        border.set_size(distance);
        source.send_success(server, || Component::translatable_with("commands.worldborder.set.immediate", vec![formatted_distance.into()]), true);
    }
    Ok((distance - current) as i32)
}

fn format_ticks_to_seconds(ticks: i64) -> String {
    java_format_fixed(ticks as f64 / 20.0, 2)
}
