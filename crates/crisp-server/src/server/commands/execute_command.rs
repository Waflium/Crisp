use std::sync::{Arc, Mutex};

use crisp_core::{Vec3, mth};
use crisp_data::commands::CommandNode;
use crisp_protocol::nbt::Nbt;

use super::data::data_accessor::{DataAccessor, KINDS};
use super::function_command::modify_sender_for_execution;
use super::item::item_commands;
use super::look_at::anchor_apply;
use crate::commands::{CommandFn, CustomModifierFn, ModifierFn};
use crate::commands::arguments::ArgumentValue;
use crate::commands::arguments::function_argument;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::execution::tasks::{BuildContexts, IsolatedCall};
use crate::commands::execution::{ChainModifiers, ChainStage, ContinuationSources, EntryAction, ExecutionControl};
use crate::commands::source::{Anchor, CommandResultCallback, CommandSourceStack};
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::loot_context::LootContextBuilder;
use crate::world::level::storage::loot::loot_params::LootParams;

static ERROR_CONDITIONAL_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.execute.conditional.fail");
static ERROR_CONDITIONAL_FAILED_COUNT: DynamicCommandExceptionType = DynamicCommandExceptionType(|count| tr("commands.execute.conditional.fail_count", vec![count]));

type Sources = Result<Vec<CommandSourceStack>, CommandSyntaxException>;

fn entities(server: &Server, context: &CommandContext, source: &CommandSourceStack, name: &str) -> Result<Vec<EntityId>, CommandSyntaxException> {
    context.get_optional_entities(server, source, name)
}

fn rotation_of(server: &Server, entity: EntityId) -> (f32, f32) {
    let e = server.entity_level(entity).entity(entity);
    (e.x_rot, e.y_rot)
}

pub fn as_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    Ok(entities(server, context, source, "targets")?.into_iter().map(|e| source.with_entity(e)).collect())
}

pub fn at_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let targets = entities(server, context, source, "targets")?;
    Ok(targets
        .into_iter()
        .map(|e| {
            let level = server.level_of(e).expect("target on this server");
            source.with_level(server, level).with_position(server.levels[level].entity(e).position()).with_rotation(rotation_of(server, e))
        })
        .collect())
}

pub fn positioned(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let pos = context.get_coordinates("pos").position(server, source);
    Ok(vec![source.with_position(pos).with_anchor(Anchor::Feet)])
}

pub fn positioned_as(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let targets = entities(server, context, source, "targets")?;
    Ok(targets.into_iter().map(|e| source.with_position(server.entity_level(e).entity(e).position())).collect())
}

pub fn rotated(_server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    Ok(vec![source.with_rotation(context.get_coordinates("rot").rotation(source))])
}

pub fn rotated_as(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let targets = entities(server, context, source, "targets")?;
    Ok(targets.into_iter().map(|e| source.with_rotation(rotation_of(server, e))).collect())
}

pub fn facing_entity(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let anchor = match context.argument("anchor") {
        ArgumentValue::Anchor(a) => *a,
        other => panic!("anchor is {other:?}"),
    };
    let targets = entities(server, context, source, "targets")?;
    Ok(targets.into_iter().map(|e| source.facing(server, anchor_apply(server.entity_level(e), anchor, e))).collect())
}

pub fn facing_pos(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let pos = context.get_coordinates("pos").position(server, source);
    Ok(vec![source.facing(server, pos)])
}

pub fn align(_server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let axes = match context.argument("axes") {
        ArgumentValue::Axes(a) => *a,
        other => panic!("axes is {other:?}"),
    };
    let p = source.position;
    let floor = |v: f64, bit: u8| if axes & bit != 0 { mth::floor(v) as f64 } else { v };
    Ok(vec![source.with_position(Vec3::new(floor(p.x, 1), floor(p.y, 2), floor(p.z, 4)))])
}

pub fn anchored(_server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let anchor = match context.argument("anchor") {
        ArgumentValue::Anchor(a) => *a,
        other => panic!("anchor is {other:?}"),
    };
    Ok(vec![source.with_anchor(anchor)])
}

pub fn in_dimension(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let level = context.get_dimension(server, "dimension")?;
    Ok(vec![source.with_level(server, level)])
}

fn expected(context: &CommandContext) -> bool {
    name(context, 0) == "if"
}

fn expect(context: &CommandContext, source: &CommandSourceStack, result: bool) -> Sources {
    Ok(if result == expected(context) { vec![source.clone()] } else { Vec::new() })
}

fn conditional(server: &Server, context: &CommandContext, source: &CommandSourceStack, result: bool) -> Result<i32, CommandSyntaxException> {
    if expected(context) == result {
        source.send_success(server, || Component::translatable("commands.execute.conditional.pass"), false);
        Ok(1)
    } else {
        Err(ERROR_CONDITIONAL_FAILED.create())
    }
}

fn numeric_conditional(server: &Server, context: &CommandContext, source: &CommandSourceStack, count: i32) -> Result<i32, CommandSyntaxException> {
    if expected(context) {
        if count > 0 {
            source.send_success(server, || Component::translatable_with("commands.execute.conditional.pass_count", vec![Arg::Int(count)]), false);
            Ok(count)
        } else {
            Err(ERROR_CONDITIONAL_FAILED.create())
        }
    } else if count == 0 {
        source.send_success(server, || Component::translatable("commands.execute.conditional.pass"), false);
        Ok(1)
    } else {
        Err(ERROR_CONDITIONAL_FAILED_COUNT.create(count))
    }
}

pub fn if_entity(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let found = !entities(server, context, source, "entities")?.is_empty();
    expect(context, source, found)
}

pub fn if_entity_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let count = entities(server, context, source, "entities")?.len() as i32;
    numeric_conditional(server, context, source, count)
}

fn is_chunk_loaded(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> bool {
    let pos = context.get_coordinates("pos").block_pos(server, source);
    let level = &server.levels[source.level];
    level.chunks.has_chunk(pos.chunk()) && level.in_entity_ticking_range(pos.chunk()) && level.entity_manager.are_entities_loaded(pos.chunk().pack())
}

pub fn if_loaded(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let loaded = is_chunk_loaded(server, context, source);
    expect(context, source, loaded)
}

pub fn if_loaded_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let loaded = is_chunk_loaded(server, context, source);
    conditional(server, context, source, loaded)
}

pub fn if_dimension(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let level = context.get_dimension(server, "dimension")?;
    expect(context, source, level == source.level)
}

pub fn if_dimension_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let level = context.get_dimension(server, "dimension")?;
    conditional(server, context, source, level == source.level)
}

static ERROR_AREA_TOO_LARGE: crate::commands::brigadier::Dynamic2CommandExceptionType =
    crate::commands::brigadier::Dynamic2CommandExceptionType(|max, count| tr("commands.execute.blocks.toobig", vec![max, count]));

fn test_block(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<bool, CommandSyntaxException> {
    let pos = crate::commands::arguments::coordinates::get_loaded_block_pos(server, context, source, "pos")?;
    let predicate = context.get_block_predicate("block").clone();
    Ok(predicate.test(&mut server.levels[source.level], pos))
}

pub fn if_block(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let result = test_block(server, context, source)?;
    expect(context, source, result)
}

pub fn if_block_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let result = test_block(server, context, source)?;
    conditional(server, context, source, result)
}

fn check_regions(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, skip_air: bool) -> Result<Option<i32>, CommandSyntaxException> {
    use crate::commands::arguments::coordinates::get_loaded_block_pos;
    use crate::world::level::BlockGetter;
    use crate::world::level::levelgen::structure::BoundingBox;
    let start = get_loaded_block_pos(server, context, source, "start")?;
    let end = get_loaded_block_pos(server, context, source, "end")?;
    let destination = get_loaded_block_pos(server, context, source, "destination")?;
    let from = BoundingBox::from_corners(start, end);
    let length = from.length();
    let to = BoundingBox::from_corners(destination, crisp_core::BlockPos::new(destination.x + length.x, destination.y + length.y, destination.z + length.z));
    let offset = crisp_core::BlockPos::new(to.min.x - from.min.x, to.min.y - from.min.y, to.min.z - from.min.z);
    let area = from.x_span() as i64 * from.y_span() as i64 * from.z_span() as i64;
    if area > 32768 {
        return Err(ERROR_AREA_TOO_LARGE.create(32768, area));
    }
    let level = &mut server.levels[source.level];
    let mut count = 0;
    for z in from.min.z..=from.max.z {
        for y in from.min.y..=from.max.y {
            for x in from.min.x..=from.max.x {
                let source_pos = crisp_core::BlockPos::new(x, y, z);
                let destination_pos = crisp_core::BlockPos::new(x + offset.x, y + offset.y, z + offset.z);
                let source_block = level.block_state(source_pos);
                if skip_air && source_block.is_block(crisp_data::blocks::AIR) {
                    continue;
                }
                if source_block != level.block_state(destination_pos) {
                    return Ok(None);
                }
                let source_entity = level.get_block_entity(source_pos).map(|e| (e.ty(), e.components().clone(), e.save_custom_only()));
                let destination_entity = level.get_block_entity(destination_pos).map(|e| (e.ty(), e.components().clone(), e.save_custom_only()));
                if let Some(source_entity) = source_entity
                    && destination_entity.as_ref() != Some(&source_entity)
                {
                    return Ok(None);
                }
                count += 1;
            }
        }
    }
    Ok(Some(count))
}

fn skip_air(context: &CommandContext) -> bool {
    crisp_data::commands::CommandNode::get(context.nodes.last().unwrap().node).name == "masked"
}

pub fn if_blocks(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let result = check_regions(server, context, source, skip_air(context))?.is_some();
    expect(context, source, result)
}

pub fn if_blocks_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let count = check_regions(server, context, source, skip_air(context))?;
    match (expected(context), count) {
        (true, Some(count)) => {
            source.send_success(server, || Component::translatable_with("commands.execute.conditional.pass_count", vec![Arg::Int(count)]), false);
            Ok(count)
        }
        (true, None) => Err(ERROR_CONDITIONAL_FAILED.create()),
        (false, Some(count)) => Err(ERROR_CONDITIONAL_FAILED_COUNT.create(count)),
        (false, None) => {
            source.send_success(server, || Component::translatable("commands.execute.conditional.pass"), false);
            Ok(1)
        }
    }
}

pub fn modifiers() -> Vec<(String, ModifierFn)> {
    let mut out: Vec<(String, ModifierFn)> = Vec::new();
    for kind in KINDS {
        for store in ["result", "success"] {
            for number in ["int", "float", "short", "long", "double", "byte"] {
                out.push((format!("execute store {store} {kind} target path {number} scale"), store_data));
            }
        }
        for test in ["if", "unless"] {
            out.push((format!("execute {test} data {kind} source path"), if_data));
        }
    }
    for test in ["if", "unless"] {
        for comparison in SCORE_COMPARISONS {
            out.push((format!("execute {test} score target targetObjective {comparison} source sourceObjective"), if_score));
        }
        out.push((format!("execute {test} score target targetObjective matches range"), if_score));
        out.push((format!("execute {test} predicate predicate"), if_predicate));
        for kind in ["entity", "block"] {
            out.push((format!("execute {test} items {kind} source slots item_predicate"), if_items));
            out.push((format!("execute {test} slots {kind} source slots"), if_slots));
        }
    }
    for store in ["result", "success"] {
        out.push((format!("execute store {store} score targets objective"), store_score));
    }
    out
}

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for kind in KINDS {
        for test in ["if", "unless"] {
            out.push((format!("execute {test} data {kind} source path"), if_data_run));
        }
    }
    for test in ["if", "unless"] {
        for comparison in SCORE_COMPARISONS {
            out.push((format!("execute {test} score target targetObjective {comparison} source sourceObjective"), if_score_run));
        }
        out.push((format!("execute {test} score target targetObjective matches range"), if_score_run));
        out.push((format!("execute {test} predicate predicate"), if_predicate_run));
        for kind in ["entity", "block"] {
            out.push((format!("execute {test} items {kind} source slots item_predicate"), if_items_run));
            out.push((format!("execute {test} slots {kind} source slots"), if_slots_run));
        }
    }
    out
}

const SCORE_COMPARISONS: [&str; 5] = ["=", "<", "<=", ">", ">="];

fn check_score(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> Result<bool, CommandSyntaxException> {
    let target = context.get_score_holder(server, source, "target")?;
    let target_objective = context.get_objective(server, "targetObjective")?;
    let comparison = name(context, 4);
    let scoreboard = server.scoreboard.borrow();
    if comparison == "matches" {
        let range = context.get_int_range("range");
        return Ok(scoreboard.get_player_score_info(target.scoreboard_name(), target_objective).is_some_and(|score| range.matches(score.value)));
    }
    let source_holder = context.get_score_holder(server, source, "source")?;
    let source_objective = context.get_objective(server, "sourceObjective")?;
    let a = scoreboard.get_player_score_info(target.scoreboard_name(), target_objective);
    let b = scoreboard.get_player_score_info(source_holder.scoreboard_name(), source_objective);
    let (Some(a), Some(b)) = (a, b) else { return Ok(false) };
    let (a, b) = (a.value, b.value);
    Ok(match comparison {
        "=" => a == b,
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        ">=" => a >= b,
        other => unreachable!("no {other} comparison"),
    })
}

pub fn if_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let result = check_score(server, context, source)?;
    expect(context, source, result)
}

pub fn if_score_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let result = check_score(server, context, source)?;
    conditional(server, context, source, result)
}

pub fn store_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let store_result = name(context, 1) == "result";
    let names = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_objective(server, "objective")?;
    let sender = source.clone();
    Ok(vec![source.with_chained_callback(move |server, success, result| {
        for name in &names {
            let index = server.score_holder_level(name);
            let level = &mut server.levels[index];
            let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(name, objective, false);
            let value = if store_result { result } else { success as i32 };
            if score.set(level, value).is_err() {
                sender.send_failure(server, crate::commands::unexpected_error("Cannot modify read-only score").message);
                break;
            }
        }
    })])
}

fn node_index(context: &CommandContext, index: usize) -> usize {
    (context.root_node == CommandNode::root().id) as usize + index
}

fn name(context: &CommandContext, index: usize) -> &'static str {
    CommandNode::get(context.nodes[node_index(context, index)].node).name
}

pub fn store_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let store_result = name(context, 1) == "result";
    let accessor = DataAccessor::access(server, context, source, node_index(context, 2), "target")?;
    let path = context.get_nbt_path("path").clone();
    let scale = context.get_double("scale");
    let constructor: fn(f64) -> Nbt = match name(context, 5) {
        "int" => |v| Nbt::Int(v as i32),
        "float" => |v| Nbt::Float(v as f32),
        "short" => |v| Nbt::Short(v as i32 as i16),
        "long" => |v| Nbt::Long(v as i64),
        "double" => Nbt::Double,
        "byte" => |v| Nbt::Byte(v as i32 as i8),
        other => unreachable!("no {other} store"),
    };
    Ok(vec![source.with_chained_callback(move |server, success, result| {
        let Ok(mut data) = accessor.get_data(server) else { return };
        let value = if store_result { result } else { success as i32 };
        let set = path.set(&mut data, &constructor(value as f64 * scale));
        accessor.write_back(server, &data);
        if set.is_ok() {
            let _ = accessor.set_data(server, data);
        }
    })])
}

fn check_matching_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = DataAccessor::access(server, context, source, node_index(context, 2), "source")?;
    Ok(context.get_nbt_path("path").count_matching(&accessor.get_data(server)?))
}

pub fn if_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let count = check_matching_data(server, context, source)?;
    expect(context, source, count > 0)
}

pub fn if_data_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let count = check_matching_data(server, context, source)?;
    numeric_conditional(server, context, source, count)
}

fn count_items(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = item_commands::access(server, context, source, name(context, 2), "source")?;
    let slots = accessor.get_slots(server, source, context.get_slot_source("slots").value())?;
    let predicate = context.get_item_predicate("item_predicate");
    Ok(slots.iter().filter(|item_stack| predicate.test(item_stack)).map(|item_stack| item_stack.count).sum())
}

fn count_slots(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = item_commands::access(server, context, source, name(context, 2), "source")?;
    let slots = accessor.get_slots(server, source, context.get_slot_source("slots").value())?;
    Ok(slots.len() as i32)
}

pub fn if_items(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let count = count_items(server, context, source)?;
    expect(context, source, count > 0)
}

pub fn if_items_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let count = count_items(server, context, source)?;
    numeric_conditional(server, context, source, count)
}

pub fn if_slots(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let count = count_slots(server, context, source)?;
    expect(context, source, count > 0)
}

pub fn if_slots_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let count = count_slots(server, context, source)?;
    numeric_conditional(server, context, source, count)
}

fn check_custom_predicate(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> bool {
    let predicate = context.get_loot_predicate("predicate");
    let mut params = LootParams::new(&mut server.levels[source.level]);
    params.origin = Some(source.position);
    params.this_entity = source.entity;
    let mut loot_context = LootContextBuilder::new(params).create(None);
    predicate.value().test(&mut loot_context)
}

pub fn if_predicate(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let result = check_custom_predicate(server, context, source);
    expect(context, source, result)
}

pub fn if_predicate_run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let result = check_custom_predicate(server, context, source);
    conditional(server, context, source, result)
}

pub fn summon(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Sources {
    let entity_type = super::summon_command::summonable_entity_type(context, "entity")?;
    let id = super::summon_command::create_entity(server, source, &entity_type, source.position, &Nbt::Compound(Vec::new()), true)?;
    Ok(vec![source.with_entity(id)])
}

static ERROR_FUNCTION_CONDITION_INSTANTATION_FAILURE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|id, reason| tr("commands.execute.function.instantiationFailure", vec![id, reason]));

pub fn custom_modifiers() -> Vec<(String, CustomModifierFn)> {
    vec![("execute if function name".into(), if_function), ("execute unless function name".into(), unless_function)]
}

fn if_function(
    server: &mut Server,
    original_source: &CommandSourceStack,
    current_sources: Vec<CommandSourceStack>,
    current_step: &ChainStage,
    modifiers: ChainModifiers,
    output: &ExecutionControl,
) {
    schedule_function_conditions_and_test(server, original_source, current_sources, |value| value != 0, current_step, None, output, modifiers);
}

fn unless_function(
    server: &mut Server,
    original_source: &CommandSourceStack,
    current_sources: Vec<CommandSourceStack>,
    current_step: &ChainStage,
    modifiers: ChainModifiers,
    output: &ExecutionControl,
) {
    schedule_function_conditions_and_test(server, original_source, current_sources, |value| value == 0, current_step, None, output, modifiers);
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_function_conditions_and_test(
    server: &mut Server,
    original_source: &CommandSourceStack,
    current_sources: Vec<CommandSourceStack>,
    check: fn(i32) -> bool,
    current_step: &ChainStage,
    parameters: Option<&Nbt>,
    output: &ExecutionControl,
    modifiers: ChainModifiers,
) {
    let functions_to_run = match function_argument::get_functions(server, current_step.top_context(), "name") {
        Ok(functions) => functions,
        Err(e) => {
            original_source.handle_error(server, e, modifiers.is_forked());
            return;
        }
    };
    if functions_to_run.is_empty() {
        return;
    }
    let mut instantiated_functions = Vec::with_capacity(functions_to_run.len());
    for function in &functions_to_run {
        match function.instantiate(parameters) {
            Ok(instantiated) => instantiated_functions.push(instantiated),
            Err(reason) => {
                original_source.handle_error(server, ERROR_FUNCTION_CONDITION_INSTANTATION_FAILURE.create(function.id(), reason), modifiers.is_forked());
                break;
            }
        }
    }
    let instantiated_functions = Arc::new(instantiated_functions);
    let filtered_sources = Arc::new(Mutex::new(Vec::with_capacity(current_sources.len())));
    for source in current_sources {
        let new_function_context = modify_sender_for_execution(&source.clear_callbacks());
        let filtered = filtered_sources.clone();
        let function_callback = CommandResultCallback::new(move |_, _, result| {
            if check(result) {
                filtered.lock().unwrap().push(source.clone());
            }
        });
        output.queue_next(EntryAction::IsolatedCall(IsolatedCall {
            functions: instantiated_functions.clone(),
            source: new_function_context,
            output: function_callback,
        }));
    }
    output.queue_next(EntryAction::Continuation {
        build: BuildContexts(current_step.next_stage()),
        modifiers,
        original: original_source.clone(),
        sources: ContinuationSources::Shared(filtered_sources),
    });
}
