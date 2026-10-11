use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;

static ERROR_RANGE_TOO_LARGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.random.error.range_too_large");
static ERROR_RANGE_TOO_SMALL: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.random.error.range_too_small");

pub fn value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    random_sample(server, context, source, false)
}

pub fn roll(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    random_sample(server, context, source, true)
}

fn random_sample(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, announce: bool) -> Result<i32, CommandSyntaxException> {
    let range = context.get_int_range("range").bounds;
    let min = range.min.unwrap_or(i32::MIN);
    let max = range.max.unwrap_or(i32::MAX);
    let span = max as i64 - min as i64;
    if span == 0 {
        return Err(ERROR_RANGE_TOO_SMALL.create());
    }
    if span >= i32::MAX as i64 {
        return Err(ERROR_RANGE_TOO_LARGE.create());
    }
    let value = if context.has_argument("sequence") {
        let seed = server.seed;
        let random = server.level.random_sequences.get(context.get_identifier("sequence"), seed);
        crisp_core::mth::random_between_inclusive(random, min, max)
    } else {
        crisp_core::mth::random_between_inclusive(&mut server.level.random, min, max)
    };
    if announce {
        let message = Component::translatable_with("commands.random.roll", vec![source.display_name(server).into(), value.into(), min.into(), max.into()]);
        server.broadcast_system_message(&message, false);
    } else {
        source.send_success(server, || Component::translatable_with("commands.random.sample.success", vec![value.into()]), false);
    }
    Ok(value)
}

fn reset_success(server: &Server, source: &CommandSourceStack, sequence: &str) -> i32 {
    source.send_success(server, || Component::translatable_with("commands.random.reset.success", vec![Component::literal(sequence).into()]), false);
    1
}

pub fn reset_sequence(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let sequence = context.get_identifier("sequence");
    let seed = server.seed;
    server.level.random_sequences.reset(sequence, seed);
    Ok(reset_success(server, source, sequence))
}

fn seed_defaults(context: &CommandContext) -> (i32, bool, bool) {
    let flag = |name: &str| !context.has_argument(name) || context.get_bool(name);
    (context.get_int("seed"), flag("includeWorldSeed"), flag("includeSequenceId"))
}

pub fn reset_sequence_with(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let sequence = context.get_identifier("sequence");
    let (salt, include_world_seed, include_sequence_id) = seed_defaults(context);
    let seed = server.seed;
    server.level.random_sequences.reset_with(sequence, seed, salt, include_world_seed, include_sequence_id);
    Ok(reset_success(server, source, sequence))
}

fn reset_all_success(server: &Server, source: &CommandSourceStack, count: i32) -> i32 {
    source.send_success(server, || Component::translatable_with("commands.random.reset.all.success", vec![Arg::Int(count)]), false);
    count
}

pub fn reset_all_sequences(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let count = server.level.random_sequences.clear();
    Ok(reset_all_success(server, source, count))
}

pub fn reset_all_sequences_and_set_new_defaults(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (salt, include_world_seed, include_sequence_id) = seed_defaults(context);
    server.level.random_sequences.set_seed_defaults(salt, include_world_seed, include_sequence_id);
    let count = server.level.random_sequences.clear();
    Ok(reset_all_success(server, source, count))
}

pub fn suggest_random_sequence(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(server.level.random_sequences.keys(), &mut builder);
    Ok(builder.build())
}
