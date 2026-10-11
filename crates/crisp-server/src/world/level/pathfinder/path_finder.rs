use crisp_core::BlockPos;

use super::binary_heap::BinaryHeap;
use super::node::Node;
use super::path::Path;
use super::pathfinding_context::PathfindingContext;
use super::target::Target;
use super::walk_node_evaluator::WalkNodeEvaluator;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;

pub struct PathFinder {
    neighbors: [u32; 32],
    max_visited_nodes: i32,
    pub node_evaluator: WalkNodeEvaluator,
    open_set: BinaryHeap,
}

impl Default for PathFinder {
    fn default() -> PathFinder {
        PathFinder::new(WalkNodeEvaluator::default(), 0)
    }
}

impl PathFinder {
    pub fn new(node_evaluator: WalkNodeEvaluator, max_visited_nodes: i32) -> PathFinder {
        PathFinder { neighbors: [0; 32], max_visited_nodes, node_evaluator, open_set: BinaryHeap::default() }
    }

    pub fn set_max_visited_nodes(&mut self, max_visited_nodes: i32) {
        self.max_visited_nodes = max_visited_nodes;
    }

    pub fn find_path(&mut self, level: PathNavigationRegion, entity: EntityId, targets: &[BlockPos], max_path_length: f32, reach_range: i32, max_visited_nodes_multiplier: f32) -> Option<Path> {
        self.open_set.clear();
        let context = PathfindingContext::new(level, entity);
        self.node_evaluator.prepare(&context, entity);
        let from = self.node_evaluator.get_start(&context)?;
        let mut tos: Vec<(Target, BlockPos)> = Vec::with_capacity(targets.len());
        for &pos in java_hash_map_order(targets).iter() {
            let target = self.node_evaluator.get_target(pos.x as f64, pos.y as f64, pos.z as f64);
            tos.push((target, pos));
        }
        let path = self.find_path_from(&context, from, &mut tos, max_path_length, reach_range, max_visited_nodes_multiplier);
        self.node_evaluator.done();
        path
    }

    fn find_path_from(&mut self, context: &PathfindingContext, from: u32, targets: &mut [(Target, BlockPos)], max_path_length: f32, reach_range: i32, max_visited_nodes_multiplier: f32) -> Option<Path> {
        let h = self.get_best_h(from, targets);
        let nodes = &mut self.node_evaluator.node_evaluator.nodes;
        let from_node = &mut nodes[from as usize];
        from_node.g = 0.0;
        from_node.h = h;
        from_node.f = h;
        self.open_set.clear();
        self.open_set.insert(nodes, from);
        let mut count = 0;
        let mut reached_targets = false;
        let max_visited_nodes_adjusted = (self.max_visited_nodes as f32 * max_visited_nodes_multiplier) as i32;
        while !self.open_set.is_empty() {
            count += 1;
            if count >= max_visited_nodes_adjusted {
                break;
            }
            let current = self.open_set.pop(&mut self.node_evaluator.node_evaluator.nodes);
            let current_node = {
                let node = &mut self.node_evaluator.node_evaluator.nodes[current as usize];
                node.closed = true;
                *node
            };
            for (target, _) in targets.iter_mut() {
                if current_node.distance_manhattan(&target.node) <= reach_range as f32 {
                    target.set_reached();
                    reached_targets = true;
                }
            }
            if reached_targets {
                break;
            }
            let from_node = self.node_evaluator.node_evaluator.nodes[from as usize];
            if !(current_node.distance_to(&from_node) >= max_path_length) {
                let mut neighbors = self.neighbors;
                let neighbor_count = self.node_evaluator.get_neighbors(context, &mut neighbors, current);
                self.neighbors = neighbors;
                for &neighbor in &neighbors[..neighbor_count] {
                    let neighbor_node = self.node_evaluator.node_evaluator.nodes[neighbor as usize];
                    let distance = current_node.distance_to(&neighbor_node);
                    let walked_distance = current_node.walked_distance + distance;
                    self.node_evaluator.node_evaluator.nodes[neighbor as usize].walked_distance = walked_distance;
                    let tentative_g_score = current_node.g + distance + neighbor_node.cost_malus;
                    if walked_distance < max_path_length && (!neighbor_node.in_open_set() || tentative_g_score < neighbor_node.g) {
                        let h = self.get_best_h(neighbor, targets) * 1.5;
                        let nodes = &mut self.node_evaluator.node_evaluator.nodes;
                        let n = &mut nodes[neighbor as usize];
                        n.came_from = Some(current);
                        n.g = tentative_g_score;
                        n.h = h;
                        if n.in_open_set() {
                            let cost = n.g + n.h;
                            self.open_set.change_cost(nodes, neighbor, cost);
                        } else {
                            n.f = n.g + n.h;
                            self.open_set.insert(nodes, neighbor);
                        }
                    }
                }
            }
        }
        let nodes = &self.node_evaluator.node_evaluator.nodes;
        let mut best: Option<Path> = None;
        if reached_targets {
            for (target, pos) in targets.iter().filter(|(target, _)| target.is_reached()) {
                let path = reconstruct_path(nodes, target.get_best_node(), *pos, true);
                if best.as_ref().is_none_or(|best| path.get_node_count() < best.get_node_count()) {
                    best = Some(path);
                }
            }
        } else {
            for (target, pos) in targets.iter() {
                let path = reconstruct_path(nodes, target.get_best_node(), *pos, false);
                let better = best.as_ref().is_none_or(|best| {
                    path.get_dist_to_target() < best.get_dist_to_target() || path.get_dist_to_target() == best.get_dist_to_target() && path.get_node_count() < best.get_node_count()
                });
                if better {
                    best = Some(path);
                }
            }
        }
        best
    }

    fn get_best_h(&self, from: u32, targets: &mut [(Target, BlockPos)]) -> f32 {
        let from_node = &self.node_evaluator.node_evaluator.nodes[from as usize];
        let mut best_h = f32::MAX;
        for (target, _) in targets.iter_mut() {
            let h = from_node.distance_to(&target.node);
            target.update_best(h, from);
            best_h = h.min(best_h);
        }
        best_h
    }
}

fn reconstruct_path(nodes: &[Node], closest: u32, target: BlockPos, reached: bool) -> Path {
    let mut path_nodes = Vec::new();
    let mut node = closest;
    path_nodes.push(nodes[node as usize]);
    while let Some(came_from) = nodes[node as usize].came_from {
        node = came_from;
        path_nodes.push(nodes[node as usize]);
    }
    path_nodes.reverse();
    Path::new(path_nodes, target, reached)
}

fn java_hash_map_order(targets: &[BlockPos]) -> Vec<BlockPos> {
    let mut capacity = 16usize;
    while targets.len() as f32 > capacity as f32 * 0.75 {
        capacity <<= 1;
    }
    let bucket = |pos: &BlockPos| {
        let h = Node::create_hash(pos.x, pos.y, pos.z);
        ((h ^ ((h as u32) >> 16) as i32) as usize) & (capacity - 1)
    };
    let mut ordered = targets.to_vec();
    ordered.sort_by_key(bucket);
    ordered
}
