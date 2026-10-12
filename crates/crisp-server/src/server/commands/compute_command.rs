use super::loot_context_sources;
use crate::commands::CommandFn;
use crate::commands::arguments::resource_or_id_argument::ResourceOrId;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::core::holder::RegistryElement;
use crate::network::chat::{Arg, Component};
use crate::server::Server;

static INVALID_NAMED_VALUE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|provider, value| tr("command.compute.result.named.invalid", vec![provider, value]));
static INVALID_UNNAMED_VALUE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("command.compute.result.unnamed.invalid", vec![value]));

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for source in ["default", "block computePos", "entity computeTarget"] {
        out.push((format!("compute {source} float provider"), compute_as_float));
        out.push((format!("compute {source} float provider scale"), compute_as_float));
        out.push((format!("compute {source} integer provider"), compute_as_int));
    }
    out
}

fn print_exact_output<T: RegistryElement>(server: &Server, source: &CommandSourceStack, provider: &ResourceOrId<T>, result: i32) {
    source.send_success(
        server,
        || match provider.unwrap_key() {
            Some(key) => Component::translatable_with("command.compute.result.named.exact", vec![Arg::String(key.to_string()), Arg::Int(result)]),
            None => Component::translatable_with("command.compute.result.unnamed.exact", vec![Arg::Int(result)]),
        },
        false,
    );
}

fn print_rounded_output<T: RegistryElement>(server: &Server, source: &CommandSourceStack, provider: &ResourceOrId<T>, result: i32, original: f32) {
    source.send_success(
        server,
        || match provider.unwrap_key() {
            Some(key) => {
                Component::translatable_with("command.compute.result.named.rounded", vec![Arg::String(key.to_string()), Arg::Float(original), Arg::Int(result)])
            }
            None => Component::translatable_with("command.compute.result.unnamed.rounded", vec![Arg::Float(original), Arg::Int(result)]),
        },
        false,
    );
}

fn compute_as_int(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let provider = context.get_int_provider("provider");
    let result = loot_context_sources::create_context(server, context, source, |loot_context| provider.value().get_int_unsafe(loot_context))?;
    match result {
        Ok(result) => {
            print_exact_output(server, source, provider, result);
            Ok(result)
        }
        Err(e) => Err(throw_invalid_value(provider, e.get_message().to_string())),
    }
}

fn compute_as_float(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let provider = context.get_float_provider("provider");
    let scale = if context.has_argument("scale") { context.get_float("scale") } else { 1.0 };
    let original = loot_context_sources::create_context(server, context, source, |loot_context| provider.value().get_float_unsafe(loot_context))?;
    let original = original.map_err(|e| throw_invalid_value(provider, e.get_message().to_string()))?;
    if !original.is_finite() {
        return Err(throw_invalid_value(provider, crate::nbt::java_float_to_string(original)));
    }
    let result = crisp_core::mth::floor_f(original * scale);
    if result as f32 == original {
        print_exact_output(server, source, provider, result);
    } else {
        print_rounded_output(server, source, provider, result, original);
    }
    Ok(result)
}

fn throw_invalid_value<T: RegistryElement>(provider: &ResourceOrId<T>, invalid_value: String) -> CommandSyntaxException {
    match provider.unwrap_key() {
        Some(key) => INVALID_NAMED_VALUE.create(Arg::String(key.to_string()), invalid_value),
        None => INVALID_UNNAMED_VALUE.create(invalid_value),
    }
}
