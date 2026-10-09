use crate::COMMAND_NODES;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CommandNodeKind {
    Root,
    Literal,
    Argument,
}

pub struct CommandNode {
    pub id: u16,
    pub kind: CommandNodeKind,
    pub name: &'static str,
    pub executable: bool,
    pub children: &'static [u16],
    pub redirect: Option<u16>,
    pub fork: bool,
    pub modifier: bool,
    pub command: Option<u16>,
    pub type_class: u16,
    pub parser: Option<&'static str>,
    pub properties: &'static str,
    pub permission: u8,
    pub suggestions: Option<&'static str>,
}

impl CommandNode {
    pub fn all() -> &'static [CommandNode] {
        COMMAND_NODES
    }

    pub fn root() -> &'static CommandNode {
        &COMMAND_NODES[0]
    }

    pub fn get(index: u16) -> &'static CommandNode {
        &COMMAND_NODES[index as usize]
    }

    pub fn children(&self) -> impl Iterator<Item = &'static CommandNode> {
        self.children.iter().map(|&i| CommandNode::get(i))
    }
}
