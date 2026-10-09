use crisp_data::commands::{CommandNode, CommandNodeKind};

use crate::commands::brigadier::{CommandContextBuilder, ParseResults};

pub struct Argument {
    pub name: &'static str,
    pub value: String,
}

pub fn of(parse: &ParseResults, command: &[u16]) -> Vec<Argument> {
    let root = &parse.context;
    let mut arguments = Vec::new();
    let mut context = root;
    visit_node_arguments(context, command, &mut arguments);
    while let Some(child) = &context.child
        && child.root_node != root.root_node
    {
        visit_node_arguments(child, command, &mut arguments);
        context = child;
    }
    arguments
}

pub fn has_signable_arguments(parse: &ParseResults) -> bool {
    !of(parse, parse.reader.string()).is_empty()
}

fn visit_node_arguments(context: &CommandContextBuilder, command: &[u16], out: &mut Vec<Argument>) {
    for parsed in &context.nodes {
        let node = CommandNode::get(parsed.node);
        if node.kind != CommandNodeKind::Argument || node.parser != Some("minecraft:message") {
            continue;
        }
        if let Some((_, value)) = context.arguments.iter().find(|(name, _)| *name == node.name) {
            let range = value.range;
            out.push(Argument { name: node.name, value: String::from_utf16_lossy(&command[range.start..range.end]) });
        }
    }
}
