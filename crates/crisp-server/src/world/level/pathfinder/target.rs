use super::node::Node;

pub struct Target {
    pub node: Node,
    best_heuristic: f32,
    best_node: Option<u32>,
    reached: bool,
}

impl Target {
    pub fn new(node: &Node) -> Target {
        Target { node: Node::new(node.x, node.y, node.z), best_heuristic: f32::MAX, best_node: None, reached: false }
    }

    pub fn update_best(&mut self, heuristic: f32, node: u32) {
        if heuristic < self.best_heuristic {
            self.best_heuristic = heuristic;
            self.best_node = Some(node);
        }
    }

    pub fn get_best_node(&self) -> u32 {
        self.best_node.expect("a best node")
    }

    pub fn set_reached(&mut self) {
        self.reached = true;
    }

    pub fn is_reached(&self) -> bool {
        self.reached
    }
}
