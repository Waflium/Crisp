use super::node::Node;

#[derive(Default)]
pub struct BinaryHeap {
    heap: Vec<u32>,
    size: usize,
}

impl BinaryHeap {
    pub fn insert(&mut self, nodes: &mut [Node], node: u32) -> u32 {
        if nodes[node as usize].heap_idx >= 0 {
            panic!("OW KNOWS!");
        }
        if self.size == self.heap.len() {
            self.heap.push(node);
        } else {
            self.heap[self.size] = node;
        }
        nodes[node as usize].heap_idx = self.size as i32;
        let idx = self.size;
        self.size += 1;
        self.up_heap(nodes, idx);
        node
    }

    pub fn clear(&mut self) {
        self.size = 0;
    }

    pub fn pop(&mut self, nodes: &mut [Node]) -> u32 {
        let popped = self.heap[0];
        self.size -= 1;
        self.heap[0] = self.heap[self.size];
        if self.size > 0 {
            self.down_heap(nodes, 0);
        }
        nodes[popped as usize].heap_idx = -1;
        popped
    }

    pub fn change_cost(&mut self, nodes: &mut [Node], node: u32, new_cost: f32) {
        let old_cost = nodes[node as usize].f;
        nodes[node as usize].f = new_cost;
        let idx = nodes[node as usize].heap_idx as usize;
        if new_cost < old_cost {
            self.up_heap(nodes, idx);
        } else {
            self.down_heap(nodes, idx);
        }
    }

    fn up_heap(&mut self, nodes: &mut [Node], mut idx: usize) {
        let node = self.heap[idx];
        let cost = nodes[node as usize].f;
        while idx > 0 {
            let parent_idx = (idx - 1) >> 1;
            let parent = self.heap[parent_idx];
            if !(cost < nodes[parent as usize].f) {
                break;
            }
            self.heap[idx] = parent;
            nodes[parent as usize].heap_idx = idx as i32;
            idx = parent_idx;
        }
        self.heap[idx] = node;
        nodes[node as usize].heap_idx = idx as i32;
    }

    fn down_heap(&mut self, nodes: &mut [Node], mut idx: usize) {
        let node = self.heap[idx];
        let cost = nodes[node as usize].f;
        loop {
            let left_idx = 1 + (idx << 1);
            let right_idx = left_idx + 1;
            if left_idx >= self.size {
                break;
            }
            let left_node = self.heap[left_idx];
            let left_cost = nodes[left_node as usize].f;
            let right_cost = if right_idx >= self.size { f32::INFINITY } else { nodes[self.heap[right_idx] as usize].f };
            if left_cost < right_cost {
                if !(left_cost < cost) {
                    break;
                }
                self.heap[idx] = left_node;
                nodes[left_node as usize].heap_idx = idx as i32;
                idx = left_idx;
            } else {
                if !(right_cost < cost) {
                    break;
                }
                let right_node = self.heap[right_idx];
                self.heap[idx] = right_node;
                nodes[right_node as usize].heap_idx = idx as i32;
                idx = right_idx;
            }
        }
        self.heap[idx] = node;
        nodes[node as usize].heap_idx = idx as i32;
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}
