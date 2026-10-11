use crisp_core::hash_map::FastMap;
use crisp_core::{BlockPos, mth};
use crisp_data::{BlockState, block_tags, blocks};

use super::node::Node;
use super::pathfinding_context::PathfindingContext;
use super::target::Target;
use crate::world::entity::EntityId;
use crate::world::level::block::campfire_block::is_lit_campfire;

pub struct NodeEvaluator {
    pub mob: EntityId,
    pub nodes: Vec<Node>,
    node_ids: FastMap<i32, u32>,
    pub entity_width: i32,
    pub entity_height: i32,
    pub entity_depth: i32,
    pub can_pass_doors: bool,
    pub can_open_doors: bool,
    pub can_float: bool,
    pub can_walk_over_fences: bool,
}

impl Default for NodeEvaluator {
    fn default() -> NodeEvaluator {
        NodeEvaluator {
            mob: 0,
            nodes: Vec::new(),
            node_ids: FastMap::default(),
            entity_width: 0,
            entity_height: 0,
            entity_depth: 0,
            can_pass_doors: true,
            can_open_doors: false,
            can_float: false,
            can_walk_over_fences: false,
        }
    }
}

impl NodeEvaluator {
    pub fn prepare(&mut self, context: &PathfindingContext, mob: EntityId) {
        self.mob = mob;
        self.nodes.clear();
        self.node_ids.clear();
        let dimensions = context.level().level.entity(mob).dimensions;
        self.entity_width = mth::floor_f(dimensions.width + 1.0);
        self.entity_height = mth::floor_f(dimensions.height + 1.0);
        self.entity_depth = mth::floor_f(dimensions.width + 1.0);
    }

    pub fn get_node_at(&mut self, pos: BlockPos) -> u32 {
        self.get_node(pos.x, pos.y, pos.z)
    }

    pub fn get_node(&mut self, x: i32, y: i32, z: i32) -> u32 {
        let nodes = &mut self.nodes;
        *self.node_ids.entry(Node::create_hash(x, y, z)).or_insert_with(|| {
            nodes.push(Node::new(x, y, z));
            nodes.len() as u32 - 1
        })
    }

    pub fn get_target_node_at(&mut self, x: f64, y: f64, z: f64) -> Target {
        let node = self.get_node(mth::floor(x), mth::floor(y), mth::floor(z));
        Target::new(&self.nodes[node as usize])
    }
}

pub fn is_burning_block(block_state: BlockState) -> bool {
    block_state.is(&block_tags::FIRE)
        || block_state.is_block(blocks::LAVA)
        || block_state.is_block(blocks::MAGMA_BLOCK)
        || is_lit_campfire(block_state)
        || block_state.is_block(blocks::LAVA_CAULDRON)
}
