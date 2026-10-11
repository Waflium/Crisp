use crisp_core::{BlockPos, Vec3};

use super::node::Node;
use crate::world::entity::Entity;

#[derive(Clone, Debug)]
pub struct Path {
    nodes: Vec<Node>,
    next_node_index: i32,
    target: BlockPos,
    dist_to_target: f32,
    reached: bool,
}

impl Path {
    pub fn new(nodes: Vec<Node>, target: BlockPos, reached: bool) -> Path {
        let dist_to_target = nodes.last().map_or(f32::MAX, |node| node.distance_manhattan_pos(target));
        Path { nodes, next_node_index: 0, target, dist_to_target, reached }
    }

    pub fn advance(&mut self) {
        self.next_node_index += 1;
    }

    pub fn not_started(&self) -> bool {
        self.next_node_index <= 0
    }

    pub fn is_done(&self) -> bool {
        self.next_node_index >= self.nodes.len() as i32
    }

    pub fn get_end_node(&self) -> Option<&Node> {
        self.nodes.last()
    }

    pub fn get_node(&self, i: usize) -> &Node {
        &self.nodes[i]
    }

    pub fn truncate_nodes(&mut self, index: usize) {
        self.nodes.truncate(index);
    }

    pub fn replace_node(&mut self, index: usize, replace_with: Node) {
        self.nodes[index] = replace_with;
    }

    pub fn get_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_next_node_index(&self) -> i32 {
        self.next_node_index
    }

    pub fn set_next_node_index(&mut self, next_node_index: i32) {
        self.next_node_index = next_node_index;
    }

    pub fn get_entity_pos_at_node(&self, entity: &Entity, index: usize) -> Vec3 {
        let node = &self.nodes[index];
        let offset = (entity.dimensions.width + 1.0) as i32 as f64 * 0.5;
        Vec3::new(node.x as f64 + offset, node.y as f64, node.z as f64 + offset)
    }

    pub fn get_node_pos(&self, index: usize) -> BlockPos {
        self.nodes[index].as_block_pos()
    }

    pub fn get_next_entity_pos(&self, entity: &Entity) -> Vec3 {
        self.get_entity_pos_at_node(entity, self.next_node_index as usize)
    }

    pub fn get_next_node_pos(&self) -> BlockPos {
        self.nodes[self.next_node_index as usize].as_block_pos()
    }

    pub fn get_next_node(&self) -> &Node {
        &self.nodes[self.next_node_index as usize]
    }

    pub fn get_previous_node(&self) -> Option<&Node> {
        (self.next_node_index > 0).then(|| &self.nodes[self.next_node_index as usize - 1])
    }

    pub fn same_as(&self, path: Option<&Path>) -> bool {
        path.is_some_and(|path| self.nodes == path.nodes)
    }

    pub fn can_reach(&self) -> bool {
        self.reached
    }

    pub fn get_target(&self) -> BlockPos {
        self.target
    }

    pub fn get_dist_to_target(&self) -> f32 {
        self.dist_to_target
    }
}
