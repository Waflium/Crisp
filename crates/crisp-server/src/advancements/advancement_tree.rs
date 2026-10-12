use std::collections::HashMap;

use super::advancement_holder::AdvancementHolder;
use super::advancement_node::AdvancementNode;
use super::tree_node_position::TreeNodePosition;

#[derive(Default)]
pub struct AdvancementTree {
    nodes: Vec<AdvancementNode>,
    by_id: HashMap<&'static str, usize>,
}

impl AdvancementTree {
    pub fn add_all(&mut self, advancements: &[&'static AdvancementHolder]) {
        let mut advancements_to_add: Vec<&'static AdvancementHolder> = advancements.to_vec();
        while !advancements_to_add.is_empty() {
            let before = advancements_to_add.len();
            advancements_to_add.retain(|holder| !self.try_insert(holder));
            if advancements_to_add.len() == before {
                error!("Couldn't load advancements: [{}]", advancements_to_add.iter().map(|holder| holder.id.as_str()).collect::<Vec<_>>().join(", "));
                break;
            }
        }
        info!("Loaded {} advancements", self.nodes.len());
    }

    fn try_insert(&mut self, holder: &'static AdvancementHolder) -> bool {
        let parent_node = match &holder.value.parent {
            Some(parent_id) => match self.by_id.get(parent_id.as_str()) {
                Some(&parent) => Some(parent),
                None => return false,
            },
            None => None,
        };
        let index = self.nodes.len();
        self.nodes.push(AdvancementNode::new(holder, parent_node));
        if let Some(parent) = parent_node {
            self.nodes[parent].add_child(index);
        }
        self.by_id.insert(&holder.id, index);
        true
    }

    pub fn roots(&self) -> impl Iterator<Item = &AdvancementNode> {
        self.nodes.iter().filter(|node| node.is_root())
    }

    pub fn tasks(&self) -> impl Iterator<Item = &AdvancementNode> {
        self.nodes.iter().filter(|node| node.is_task())
    }

    pub fn nodes(&self) -> &[AdvancementNode] {
        &self.nodes
    }

    pub fn node(&self, index: usize) -> &AdvancementNode {
        &self.nodes[index]
    }

    pub fn node_mut(&mut self, index: usize) -> &mut AdvancementNode {
        &mut self.nodes[index]
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub fn get(&self, id: &str) -> Option<&AdvancementNode> {
        self.index_of(id).map(|index| &self.nodes[index])
    }

    pub fn parent(&self, node: &AdvancementNode) -> Option<&AdvancementNode> {
        node.parent.map(|parent| &self.nodes[parent])
    }

    pub fn children<'a>(&'a self, node: &'a AdvancementNode) -> impl Iterator<Item = &'a AdvancementNode> {
        node.children.iter().map(|&child| &self.nodes[child])
    }

    pub fn root<'a>(&'a self, node: &'a AdvancementNode) -> &'a AdvancementNode {
        let mut root = node;
        while let Some(parent) = self.parent(root) {
            root = parent;
        }
        root
    }

    pub fn reposition_nodes(&mut self) {
        let roots: Vec<usize> = (0..self.nodes.len()).filter(|&index| self.nodes[index].is_root()).collect();
        for root in roots {
            if self.nodes[root].holder.value.display.is_some() {
                TreeNodePosition::run(self, root);
            }
        }
    }
}
