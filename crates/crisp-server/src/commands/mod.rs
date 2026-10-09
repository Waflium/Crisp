pub mod arguments;
pub mod brigadier;
pub mod shared_suggestion_provider;
pub mod source;
pub mod suggestion;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::LazyLock;

use crisp_data::commands::{CommandNode, CommandNodeKind};
use crisp_data::{registry, registry_id};
use crisp_protocol::Writer;
use serde_json::Value;

use self::brigadier::{
    CommandContext, CommandSyntaxException, ContextChain, DISPATCHER_UNKNOWN_ARGUMENT, DISPATCHER_UNKNOWN_COMMAND, DynamicCommandExceptionType,
    ParseResults, StringReader, error_context, tr,
};
use self::source::CommandSourceStack;
use self::suggestion::{Suggestions, SuggestionsBuilder};
use crate::network::chat::{Component, HoverEvent};
use crate::server::Server;

pub type CommandFn = fn(&mut Server, &CommandContext, &CommandSourceStack) -> Result<i32, CommandSyntaxException>;

pub type ModifierFn = fn(&mut Server, &CommandContext, &CommandSourceStack) -> Result<Vec<CommandSourceStack>, CommandSyntaxException>;

pub static ERROR_FORK_LIMIT_REACHED: DynamicCommandExceptionType = DynamicCommandExceptionType(|limit| tr("command.forkLimit", vec![limit]));

pub fn node_at(path: &str) -> &'static CommandNode {
    let mut node = CommandNode::root();
    for name in path.split(' ') {
        node = node.children().find(|c| c.name == name).unwrap_or_else(|| panic!("no command node at '{path}'"));
    }
    node
}

static EXECUTORS: LazyLock<Vec<Option<CommandFn>>> = LazyLock::new(|| {
    let mut table = vec![None; CommandNode::all().len()];
    for (path, f) in crate::server::commands::executors() {
        let node = node_at(&path);
        assert!(node.executable, "'{path}' is not executable");
        table[node.id as usize] = Some(f);
    }
    table
});

static MODIFIERS: LazyLock<Vec<Option<ModifierFn>>> = LazyLock::new(|| {
    let mut table = vec![None; CommandNode::all().len()];
    for (path, f) in crate::server::commands::modifiers() {
        let node = node_at(&path);
        assert!(node.modifier, "'{path}' has no redirect modifier");
        table[node.id as usize] = Some(f);
    }
    table
});

pub type SuggestFn = fn(&Server, &CommandContext, &CommandSourceStack, SuggestionsBuilder) -> Result<Suggestions, CommandSyntaxException>;

static SUGGESTERS: LazyLock<Vec<Option<SuggestFn>>> = LazyLock::new(|| {
    let mut table = vec![None; CommandNode::all().len()];
    for (path, f) in crate::server::commands::suggestion_providers() {
        let node = node_at(&path);
        assert_eq!(node.suggestions, Some("minecraft:ask_server"), "'{path}' does not ask the server");
        table[node.id as usize] = Some(f);
    }
    table
});

pub fn completion_suggestions(server: &Server, source: &CommandSourceStack, command: &str) -> Option<Suggestions> {
    let units: Vec<u16> = command.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    if reader.at('/') {
        reader.skip();
    }
    let parse = brigadier::parse(reader, source);
    let cursor = parse.reader.total_length();
    let (parent, start) = parse.context.find_suggestion_context(cursor)?;
    let start = start.min(cursor);
    let truncated = units[..cursor].to_vec();
    let truncated_lower_case = suggestion::java_lower_case(&truncated);
    let context = parse.context.build(&brigadier::utf16(&truncated));
    let all = CommandNode::get(parent)
        .children()
        .map(|child| {
            let builder = SuggestionsBuilder::new(truncated.clone(), truncated_lower_case.clone(), start);
            list_suggestions(server, child, &context, source, builder).unwrap_or_else(|_| Suggestions::empty())
        })
        .collect();
    Some(Suggestions::merge(&units, all))
}

fn list_suggestions(
    server: &Server,
    node: &'static CommandNode,
    context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: SuggestionsBuilder,
) -> Result<Suggestions, CommandSyntaxException> {
    match node.kind {
        CommandNodeKind::Literal => {
            if node.name.to_lowercase().starts_with(&builder.remaining_lower_case()) {
                builder.suggest(node.name);
            }
        }
        CommandNodeKind::Argument => match node.suggestions {
            Some("minecraft:ask_server") => {
                if let Some(provider) = SUGGESTERS[node.id as usize] {
                    return provider(server, context, source, builder);
                }
            }
            Some("minecraft:available_sounds") => shared_suggestion_provider::suggest_resource(registry::SOUND_EVENT.iter(), &mut builder),
            Some("minecraft:summonable_entities") => shared_suggestion_provider::suggest_resource_with_tooltips(
                registry::ENTITY_TYPE.iter().filter(|ty| crate::world::entity::can_summon(ty)).map(|&ty| (ty, crate::world::entity::description(ty))),
                &mut builder,
            ),
            Some(_) => {}
            None => return Ok(arguments::list_suggestions(server, node, source, builder)),
        },
        CommandNodeKind::Root => {}
    }
    Ok(builder.build())
}

pub fn perform_prefixed_command(server: &mut Server, source: &CommandSourceStack, command: &str) {
    let command = command.strip_prefix('/').unwrap_or(command);
    let units: Vec<u16> = command.encode_utf16().collect();
    let parse = brigadier::parse(StringReader::new(&units), source);
    perform_command(server, parse, command, source);
}

pub fn perform_command(server: &mut Server, parse: ParseResults, command: &str, source: &CommandSourceStack) {
    let Some(chain) = finish_parsing(server, parse, command, source) else { return };
    execute_chain(server, chain, source);
}

fn finish_parsing(server: &Server, parse: ParseResults, command: &str, source: &CommandSourceStack) -> Option<ContextChain> {
    let result = parse_exception(&parse).map_or_else(
        || ContextChain::try_flatten(parse.context.build(command)).ok_or_else(|| DISPATCHER_UNKNOWN_COMMAND.create_with_context(&parse.reader)),
        Err,
    );
    match result {
        Ok(chain) => Some(chain),
        Err(e) => {
            source.send_failure(server, e.message.clone());
            if let Some(context) = error_context(&e, command) {
                source.send_failure(server, context);
            }
            None
        }
    }
}

fn parse_exception(parse: &ParseResults) -> Option<CommandSyntaxException> {
    if !parse.reader.can_read() {
        None
    } else if parse.exceptions.len() == 1 {
        Some(parse.exceptions[0].1.clone())
    } else if parse.context.range.is_empty() {
        Some(DISPATCHER_UNKNOWN_COMMAND.create_with_context(&parse.reader))
    } else {
        Some(DISPATCHER_UNKNOWN_ARGUMENT.create_with_context(&parse.reader))
    }
}

fn execute_chain(server: &mut Server, chain: ContextChain, original: &CommandSourceStack) {
    let fork_limit = server.game_rules.get_int(crisp_data::game_rule::MAX_COMMAND_FORKS) as usize;
    let mut forked = false;
    let mut sources = vec![original.clone()];
    for context in &chain.modifiers {
        forked |= context.forks;
        let Some(modifier) = context.modifier else { continue };
        let modifier = MODIFIERS[modifier as usize].unwrap_or_else(|| unimplemented_modifier(context));
        let mut next = Vec::new();
        for source in &sources {
            match modifier(server, context, source) {
                Ok(new_sources) => {
                    if next.len() + new_sources.len() >= fork_limit {
                        original.handle_error(server, ERROR_FORK_LIMIT_REACHED.create(fork_limit as i32), forked);
                        return;
                    }
                    next.extend(new_sources);
                }
                Err(e) => {
                    source.handle_error(server, e, forked);
                    if !forked {
                        return;
                    }
                }
            }
        }
        sources = next;
    }
    let executable = &chain.executable;
    let command = executable.command.and_then(|id| EXECUTORS[id as usize]);
    for source in sources {
        let result = match command {
            Some(command) => command(server, executable, &source),
            None => Err(not_implemented(executable)),
        };
        match result {
            Ok(result) => source.callback.on_result(server, true, result),
            Err(e) => {
                source.callback.on_result(server, false, 0);
                if !forked {
                    source.handle_error(server, e, forked);
                }
            }
        }
    }
}

fn unimplemented_modifier(_context: &CommandContext) -> ModifierFn {
    |_, context, _| Err(not_implemented(context))
}

pub fn unexpected_error(message: &str) -> CommandSyntaxException {
    CommandSyntaxException::new(Component::translatable("command.failed").with_hover(HoverEvent::ShowText(Box::new(Component::literal(message)))))
}

fn not_implemented(context: &CommandContext) -> CommandSyntaxException {
    let name = context.nodes.first().map_or("", |n| CommandNode::get(n.node).name);
    CommandSyntaxException::new(
        Component::translatable("command.failed").with_hover(HoverEvent::ShowText(Box::new(Component::literal(format!("/{name} is not implemented in Crisp yet"))))),
    )
}

pub fn write_commands_packet(w: &mut Writer, source: &CommandSourceStack) {
    let tree = UsableTree::build(source);
    tree.write(w);
}

struct UsableNode {
    original: u16,
    children: Vec<usize>,
    redirect: Option<usize>,
}

struct UsableTree {
    nodes: Vec<UsableNode>,
}

#[derive(Default)]
struct Keys {
    ids: HashMap<(CommandNodeKind, &'static str, u16, Option<u16>, BTreeMap<&'static str, u32>), u32>,
}

impl Keys {
    fn intern(&mut self, node: &'static CommandNode, children: BTreeMap<&'static str, u32>) -> u32 {
        let next = self.ids.len() as u32;
        let type_class = if node.kind == CommandNodeKind::Argument { node.type_class } else { 0 };
        *self.ids.entry((node.kind, node.name, type_class, node.command, children)).or_insert(next)
    }
}

static ORIGINAL_KEYS: LazyLock<(Keys, Vec<u32>)> = LazyLock::new(|| {
    let mut keys = Keys::default();
    let mut ids = vec![0; CommandNode::all().len()];
    for node in CommandNode::all().iter().rev() {
        let children = node.children().map(|c| (c.name, ids[c.id as usize])).collect();
        ids[node.id as usize] = keys.intern(node, children);
    }
    (keys, ids)
});

impl UsableTree {
    fn build(source: &CommandSourceStack) -> UsableTree {
        let original_keys = &ORIGINAL_KEYS.1;
        let mut tree = UsableTree { nodes: vec![UsableNode { original: 0, children: Vec::new(), redirect: None }] };
        let mut converted: HashMap<u32, usize> = HashMap::new();
        converted.insert(original_keys[0], 0);
        tree.fill(CommandNode::root(), 0, source, &mut converted, original_keys);
        tree
    }

    fn fill(&mut self, from: &'static CommandNode, target: usize, source: &CommandSourceStack, converted: &mut HashMap<u32, usize>, keys: &[u32]) {
        for child in from.children() {
            if !source.has_permission(child.permission as i32) {
                continue;
            }
            let redirect = child.redirect.and_then(|r| converted.get(&keys[r as usize]).copied());
            let node = self.nodes.len();
            self.nodes.push(UsableNode { original: child.id, children: Vec::new(), redirect });
            converted.insert(keys[child.id as usize], node);
            self.nodes[target].children.push(node);
            if !child.children.is_empty() {
                self.fill(child, node, source, converted, keys);
            }
        }
    }

    fn keys(&self) -> Vec<u32> {
        let mut keys = Keys::default();
        let mut out = vec![0; self.nodes.len()];
        for i in (0..self.nodes.len()).rev() {
            let node = &self.nodes[i];
            let children = node.children.iter().map(|&c| (CommandNode::get(self.nodes[c].original).name, out[c])).collect();
            out[i] = keys.intern(CommandNode::get(node.original), children);
        }
        out
    }

    fn write(&self, w: &mut Writer) {
        let keys = self.keys();
        let mut ids: HashMap<u32, i32> = HashMap::new();
        let mut entries: Vec<usize> = Vec::new();
        let mut queue = VecDeque::from([0usize]);
        while let Some(node) = queue.pop_front() {
            if ids.contains_key(&keys[node]) {
                continue;
            }
            ids.insert(keys[node], entries.len() as i32);
            entries.push(node);
            queue.extend(&self.nodes[node].children);
            if let Some(redirect) = self.nodes[node].redirect {
                queue.push_back(redirect);
            }
        }
        w.varint(entries.len() as i32);
        for &index in &entries {
            let node = &self.nodes[index];
            let original = CommandNode::get(node.original);
            let mut flags = match original.kind {
                CommandNodeKind::Root => 0,
                CommandNodeKind::Literal => 1,
                CommandNodeKind::Argument => 2,
            };
            if node.redirect.is_some() {
                flags |= 8;
            }
            if original.executable {
                flags |= 4;
            }
            if original.permission > 0 {
                flags |= 32;
            }
            if original.suggestions.is_some() {
                flags |= 16;
            }
            w.u8(flags);
            w.varint(node.children.len() as i32);
            for &c in &node.children {
                w.varint(ids[&keys[c]]);
            }
            if let Some(redirect) = node.redirect {
                w.varint(ids[&keys[redirect]]);
            }
            match original.kind {
                CommandNodeKind::Root => {}
                CommandNodeKind::Literal => w.string(original.name),
                CommandNodeKind::Argument => {
                    w.string(original.name);
                    let parser = original.parser.unwrap();
                    w.varint(registry_id(registry::COMMAND_ARGUMENT_TYPE, parser));
                    write_argument_properties(w, parser, &serde_json::from_str(original.properties).unwrap_or(Value::Null));
                    if let Some(suggestions) = original.suggestions {
                        w.string(suggestions);
                    }
                }
            }
        }
        w.varint(ids[&keys[0]]);
    }
}

fn write_argument_properties(w: &mut Writer, parser: &str, properties: &Value) {
    let has = |key: &str| properties.get(key).is_some();
    let flags = |w: &mut Writer| w.u8(has("min") as u8 | (has("max") as u8) << 1);
    match parser {
        "brigadier:double" => {
            flags(w);
            ["min", "max"].iter().filter_map(|k| properties.get(*k)).for_each(|v| w.f64(v.as_f64().unwrap()));
        }
        "brigadier:float" => {
            flags(w);
            ["min", "max"].iter().filter_map(|k| properties.get(*k)).for_each(|v| w.f32(v.as_f64().unwrap() as f32));
        }
        "brigadier:integer" => {
            flags(w);
            ["min", "max"].iter().filter_map(|k| properties.get(*k)).for_each(|v| w.i32(v.as_i64().unwrap() as i32));
        }
        "brigadier:long" => {
            flags(w);
            ["min", "max"].iter().filter_map(|k| properties.get(*k)).for_each(|v| w.i64(v.as_i64().unwrap()));
        }
        "brigadier:string" => w.varint(match properties["type"].as_str() {
            Some("word") => 0,
            Some("phrase") => 1,
            _ => 2,
        }),
        "minecraft:entity" => {
            let single = properties["amount"] == "single";
            let players = properties["type"] == "players";
            w.u8(single as u8 | (players as u8) << 1);
        }
        "minecraft:score_holder" => w.u8((properties["amount"] == "multiple") as u8),
        "minecraft:time" => w.i32(properties["min"].as_i64().unwrap_or(0) as i32),
        "minecraft:resource" | "minecraft:resource_key" | "minecraft:resource_or_tag" | "minecraft:resource_or_tag_key" | "minecraft:resource_selector" => {
            w.string(properties["registry"].as_str().unwrap())
        }
        _ => {}
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn oracle(level: i32) -> Option<Vec<u8>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../.vanilla/extracted/commands_packet_{level}.bin"));
        std::fs::read(path).ok()
    }

    fn source(permission: i32) -> CommandSourceStack {
        CommandSourceStack {
            source: source::CommandSource::Null,
            position: crisp_core::Vec3::ZERO,
            rotation: (0.0, 0.0),
            permission,
            names: source::Names::Constant(Component::empty()),
            entity: None,
            silent: false,
            anchor: source::Anchor::Feet,
            callback: Default::default(),
            signing_context: None,
        }
    }

    #[test]
    fn commands_packet_matches_vanilla() {
        for level in 0..=4 {
            let expected = oracle(level).expect("run cargo xtask vanilla");
            let mut w = Writer::new();
            write_commands_packet(&mut w, &source(level));
            assert!(w.as_slice() == expected.as_slice(), "permission level {level}: {} bytes, vanilla {}", w.as_slice().len(), expected.len());
        }
    }

    #[test]
    fn every_registered_path_exists() {
        LazyLock::force(&EXECUTORS);
        LazyLock::force(&MODIFIERS);
    }
}

