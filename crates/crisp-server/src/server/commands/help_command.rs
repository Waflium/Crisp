use crisp_data::commands::{CommandNode, CommandNodeKind};

use crate::commands::brigadier::{self, CommandContext, CommandSyntaxException, SimpleCommandExceptionType, StringReader};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.help.failed");

fn usage_text(node: &CommandNode) -> String {
    match node.kind {
        CommandNodeKind::Argument => format!("<{}>", node.name),
        _ => node.name.to_string(),
    }
}

fn can_use(node: &CommandNode, source: &CommandSourceStack) -> bool {
    source.has_permission(node.permission as i32)
}

pub fn smart_usage(node: &'static CommandNode, source: &CommandSourceStack) -> Vec<String> {
    let optional = node.executable;
    node.children().filter_map(|child| smart_usage_of(child, source, optional, false)).collect()
}

fn smart_usage_of(node: &'static CommandNode, source: &CommandSourceStack, optional: bool, deep: bool) -> Option<String> {
    if !can_use(node, source) {
        return None;
    }
    let this = if optional { format!("[{}]", usage_text(node)) } else { usage_text(node) };
    let child_optional = node.executable;
    let (open, close) = if child_optional { ("[", "]") } else { ("(", ")") };
    if deep {
        return Some(this);
    }
    if let Some(redirect) = node.redirect {
        let redirect = if redirect == CommandNode::root().id { "...".to_string() } else { format!("-> {}", usage_text(CommandNode::get(redirect))) };
        return Some(format!("{this} {redirect}"));
    }
    let children: Vec<&'static CommandNode> = node.children().filter(|c| can_use(c, source)).collect();
    if children.len() == 1 {
        if let Some(usage) = smart_usage_of(children[0], source, child_optional, child_optional) {
            return Some(format!("{this} {usage}"));
        }
    } else if children.len() > 1 {
        let mut child_usage: Vec<String> = Vec::new();
        for child in &children {
            if let Some(usage) = smart_usage_of(child, source, child_optional, true)
                && !child_usage.contains(&usage)
            {
                child_usage.push(usage);
            }
        }
        if child_usage.len() == 1 {
            let usage = &child_usage[0];
            return Some(format!("{this} {}", if child_optional { format!("[{usage}]") } else { usage.clone() }));
        }
        if child_usage.len() > 1 {
            let names: Vec<String> = children.iter().map(|c| usage_text(c)).collect();
            return Some(format!("{this} {open}{}{close}", names.join("|")));
        }
    }
    Some(this)
}

pub fn help(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let usage = smart_usage(CommandNode::root(), source);
    for line in &usage {
        source.send_success(server, || Component::literal(format!("/{line}")), false);
    }
    Ok(usage.len() as i32)
}

pub fn help_command(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let input = context.get_string("command").to_string();
    let units: Vec<u16> = input.encode_utf16().collect();
    let parse = brigadier::parse(StringReader::new(&units), source);
    let Some(last) = parse.context.nodes.last() else { return Err(ERROR_FAILED.create()) };
    let usage = smart_usage(CommandNode::get(last.node), source);
    for line in &usage {
        source.send_success(server, || Component::literal(format!("/{input} {line}")), false);
    }
    Ok(usage.len() as i32)
}
