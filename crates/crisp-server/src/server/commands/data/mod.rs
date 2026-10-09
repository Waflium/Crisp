pub mod data_accessor;

use crisp_data::commands::CommandNode;
use crisp_protocol::nbt::Nbt;

use self::data_accessor::{DataAccessor, KINDS};
use crate::commands::CommandFn;
use crate::commands::arguments::nbt_path_argument::{ERROR_DATA_TOO_DEEP, NbtPath, is_too_deep};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::nbt;
use crate::server::Server;

static ERROR_MERGE_UNCHANGED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.data.merge.failed");
static ERROR_GET_NOT_NUMBER: DynamicCommandExceptionType = DynamicCommandExceptionType(|path| tr("commands.data.get.invalid", vec![path]));
static ERROR_MULTIPLE_TAGS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.data.get.multiple");
static ERROR_EXPECTED_OBJECT: DynamicCommandExceptionType = DynamicCommandExceptionType(|node| tr("commands.data.modify.expected_object", vec![node]));
static ERROR_EXPECTED_VALUE: DynamicCommandExceptionType = DynamicCommandExceptionType(|node| tr("commands.data.modify.expected_value", vec![node]));
static ERROR_INVALID_SUBSTRING: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|start, end| tr("commands.data.modify.invalid_substring", vec![start, end]));

const TARGET: usize = 2;

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for target in KINDS {
        out.push((format!("data merge {target} target nbt"), merge_data));
        out.push((format!("data get {target} target"), get_all_data));
        out.push((format!("data get {target} target path"), get_data));
        out.push((format!("data get {target} target path scale"), get_numeric));
        out.push((format!("data remove {target} target path"), remove_data));
        for op in ["insert index", "prepend", "append", "set", "merge"] {
            let node = format!("data modify {target} target targetPath {op}");
            for source in KINDS {
                out.push((format!("{node} from {source} source"), modify));
                out.push((format!("{node} from {source} source sourcePath"), modify));
                out.push((format!("{node} string {source} source"), modify));
                out.push((format!("{node} string {source} source sourcePath"), modify));
                out.push((format!("{node} string {source} source sourcePath start"), modify));
                out.push((format!("{node} string {source} source sourcePath start end"), modify));
            }
            out.push((format!("{node} value value"), modify));
        }
    }
    out
}

fn name(context: &CommandContext, index: usize) -> &'static str {
    CommandNode::get(context.nodes[index].node).name
}

fn get_as_text(tag: &Nbt) -> Result<String, CommandSyntaxException> {
    match tag {
        Nbt::String(value) => Ok(value.clone()),
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) => Ok(nbt::to_string(tag)),
        _ => Err(ERROR_EXPECTED_VALUE.create(nbt::to_string(tag))),
    }
}

fn substring(input: &str, start: i32, end: Option<i32>) -> Result<String, CommandSyntaxException> {
    let units: Vec<u16> = input.encode_utf16().collect();
    let length = units.len() as i32;
    let offset = |index: i32| if index >= 0 { index } else { length + index };
    let (start, end) = (offset(start), end.map_or(length, offset));
    if start >= 0 && end <= length && start <= end {
        Ok(String::from_utf16_lossy(&units[start as usize..end as usize]))
    } else {
        Err(ERROR_INVALID_SUBSTRING.create(start, end))
    }
}

fn source_tags(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, mode: usize) -> Result<Vec<Nbt>, CommandSyntaxException> {
    if name(context, mode) == "value" {
        return Ok(vec![context.get_nbt("value").clone()]);
    }
    let accessor = DataAccessor::access(server, context, source, mode + 1, "source")?;
    let data = accessor.get_data(server)?;
    let tags = if context.has_argument("sourcePath") { context.get_nbt_path("sourcePath").get(&data)? } else { vec![data] };
    if name(context, mode) == "from" {
        return Ok(tags);
    }
    let range = context.has_argument("start").then(|| (context.get_int("start"), context.has_argument("end").then(|| context.get_int("end"))));
    tags.iter()
        .map(|tag| {
            let text = get_as_text(tag)?;
            Ok(Nbt::String(match range {
                Some((start, end)) => substring(&text, start, end)?,
                None => text,
            }))
        })
        .collect()
}

fn modify(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let op = name(context, 5);
    let mode = if op == "insert" { 7 } else { 6 };
    let tags = source_tags(server, context, source, mode)?;
    manipulate_data(server, context, source, |target, path| match op {
        "insert" => path.insert(context.get_int("index"), target, &tags),
        "prepend" => path.insert(0, target, &tags),
        "append" => path.insert(-1, target, &tags),
        "set" => path.set(target, tags.last().expect("sources are never empty")),
        "merge" => {
            let mut combined = Vec::new();
            for tag in &tags {
                if is_too_deep(tag, 0) {
                    return Err(ERROR_DATA_TOO_DEEP.create());
                }
                let Nbt::Compound(fields) = tag else { return Err(ERROR_EXPECTED_OBJECT.create(nbt::to_string(tag))) };
                nbt::merge(&mut combined, fields);
            }
            path.get_or_create_each(target, &|| Nbt::Compound(Vec::new()), |tag| {
                let original = tag.clone();
                let Nbt::Compound(fields) = tag else { return Err(ERROR_EXPECTED_OBJECT.create(nbt::to_string(tag))) };
                nbt::merge(fields, &combined);
                Ok((original != *tag) as i32)
            })
        }
        other => unreachable!("no {other} modification"),
    })
}

fn manipulate_data(
    server: &mut Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    manipulator: impl FnOnce(&mut Nbt, &NbtPath) -> Result<i32, CommandSyntaxException>,
) -> Result<i32, CommandSyntaxException> {
    let target = DataAccessor::access(server, context, source, TARGET, "target")?;
    let mut data = target.get_data(server)?;
    let result = manipulator(&mut data, context.get_nbt_path("targetPath"));
    target.write_back(server, &data);
    let result = result?;
    if result == 0 {
        return Err(ERROR_MERGE_UNCHANGED.create());
    }
    target.set_data(server, data)?;
    source.send_success(server, || target.modified_success(server), true);
    Ok(result)
}

fn remove_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = DataAccessor::access(server, context, source, TARGET, "target")?;
    let mut data = accessor.get_data(server)?;
    let count = context.get_nbt_path("path").remove(&mut data);
    accessor.write_back(server, &data);
    if count == 0 {
        return Err(ERROR_MERGE_UNCHANGED.create());
    }
    accessor.set_data(server, data)?;
    source.send_success(server, || accessor.modified_success(server), true);
    Ok(count)
}

fn get_single_tag(server: &mut Server, path: &NbtPath, accessor: &DataAccessor) -> Result<Nbt, CommandSyntaxException> {
    let mut tags = path.get(&accessor.get_data(server)?)?;
    if tags.len() > 1 {
        return Err(ERROR_MULTIPLE_TAGS.create());
    }
    Ok(tags.remove(0))
}

fn get_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = DataAccessor::access(server, context, source, TARGET, "target")?;
    let tag = get_single_tag(server, context.get_nbt_path("path"), &accessor)?;
    let result = match &tag {
        Nbt::String(value) => value.encode_utf16().count() as i32,
        Nbt::List(items) => items.len() as i32,
        Nbt::Compound(fields) => fields.len() as i32,
        Nbt::ByteArray(v) => v.len() as i32,
        Nbt::IntArray(v) => v.len() as i32,
        Nbt::LongArray(v) => v.len() as i32,
        numeric => numeric.as_f64().unwrap().floor() as i32,
    };
    source.send_success(server, || accessor.print_success(server, &tag), false);
    Ok(result)
}

fn get_numeric(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = DataAccessor::access(server, context, source, TARGET, "target")?;
    let path = context.get_nbt_path("path");
    let scale = context.get_double("scale");
    let Some(value) = get_single_tag(server, path, &accessor)?.as_f64() else { return Err(ERROR_GET_NOT_NUMBER.create(path.original.as_str())) };
    let result = (value * scale).floor() as i32;
    source.send_success(server, || accessor.print_scaled_success(server, path, scale, result), false);
    Ok(result)
}

fn get_all_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = DataAccessor::access(server, context, source, TARGET, "target")?;
    let data = accessor.get_data(server)?;
    source.send_success(server, || accessor.print_success(server, &data), false);
    Ok(1)
}

fn merge_data(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let accessor = DataAccessor::access(server, context, source, TARGET, "target")?;
    let old = accessor.get_data(server)?;
    let tag = context.get_nbt("nbt");
    if is_too_deep(tag, 0) {
        return Err(ERROR_DATA_TOO_DEEP.create());
    }
    let (Nbt::Compound(old_fields), Nbt::Compound(fields)) = (&old, tag) else { unreachable!("compounds") };
    let mut result = old_fields.clone();
    nbt::merge(&mut result, fields);
    let result = Nbt::Compound(result);
    if old == result {
        return Err(ERROR_MERGE_UNCHANGED.create());
    }
    accessor.set_data(server, result)?;
    source.send_success(server, || accessor.modified_success(server), true);
    Ok(1)
}
