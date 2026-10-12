use crisp_core::mth;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::{RAIN_DELAY, RAIN_DURATION, THUNDER_DURATION};

const DEFAULT_TIME: i32 = -1;

fn input(context: &CommandContext) -> i32 {
    if context.has_argument("duration") { context.get_int("duration") } else { DEFAULT_TIME }
}

fn get_duration(server: &mut Server, source: &CommandSourceStack, input: i32, (min, max): (i32, i32)) -> i32 {
    if input == DEFAULT_TIME { mth::random_between_inclusive(&mut server.levels[source.level].random, min, max) } else { input }
}

pub fn set_clear(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let duration = input(context);
    let clear_time = get_duration(server, source, duration, RAIN_DELAY);
    server.set_weather_parameters(clear_time, 0, false, false);
    source.send_success(server, || Component::translatable("commands.weather.set.clear"), true);
    Ok(duration)
}

pub fn set_rain(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let duration = input(context);
    let rain_time = get_duration(server, source, duration, RAIN_DURATION);
    server.set_weather_parameters(0, rain_time, true, false);
    source.send_success(server, || Component::translatable("commands.weather.set.rain"), true);
    Ok(duration)
}

pub fn set_thunder(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let duration = input(context);
    let rain_time = get_duration(server, source, duration, THUNDER_DURATION);
    server.set_weather_parameters(0, rain_time, true, true);
    source.send_success(server, || Component::translatable("commands.weather.set.thunder"), true);
    Ok(duration)
}
