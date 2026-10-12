use super::advancement_tree::AdvancementTree;

struct Position {
    node: usize,
    parent: Option<usize>,
    previous_sibling: Option<usize>,
    child_index: i32,
    children: Vec<usize>,
    ancestor: usize,
    thread: Option<usize>,
    x: i32,
    y: f32,
    mod_: f32,
    change: f32,
    shift: f32,
}

pub struct TreeNodePosition {
    positions: Vec<Position>,
}

impl TreeNodePosition {
    fn create(&mut self, tree: &AdvancementTree, node: usize, parent: Option<usize>, previous_sibling: Option<usize>, child_index: i32, depth: i32) -> usize {
        if tree.node(node).advancement().display.is_none() {
            panic!("Can't position an invisible advancement!");
        }
        let index = self.positions.len();
        self.positions.push(Position {
            node,
            parent,
            previous_sibling,
            child_index,
            children: Vec::new(),
            ancestor: index,
            thread: None,
            x: depth,
            y: -1.0,
            mod_: 0.0,
            change: 0.0,
            shift: 0.0,
        });
        let mut previous = None;
        for &child in &tree.node(node).children {
            previous = self.add_child(tree, index, child, previous);
        }
        index
    }

    fn add_child(&mut self, tree: &AdvancementTree, this: usize, node: usize, mut previous: Option<usize>) -> Option<usize> {
        if tree.node(node).advancement().display.is_some() {
            let child_index = self.positions[this].children.len() as i32 + 1;
            let depth = self.positions[this].x + 1;
            let child = self.create(tree, node, Some(this), previous, child_index, depth);
            self.positions[this].children.push(child);
            previous = Some(child);
        } else {
            for &grandchild in &tree.node(node).children {
                previous = self.add_child(tree, this, grandchild, previous);
            }
        }
        previous
    }

    fn first_walk(&mut self, this: usize) {
        if self.positions[this].children.is_empty() {
            self.positions[this].y = match self.positions[this].previous_sibling {
                Some(previous_sibling) => self.positions[previous_sibling].y + 1.0,
                None => 0.0,
            };
        } else {
            let mut default_ancestor: Option<usize> = None;
            let children = self.positions[this].children.clone();
            for &child in &children {
                self.first_walk(child);
                default_ancestor = Some(self.apportion(child, default_ancestor.unwrap_or(child)));
            }
            self.execute_shifts(this);
            let midpoint = (self.positions[children[0]].y + self.positions[children[children.len() - 1]].y) / 2.0;
            match self.positions[this].previous_sibling {
                Some(previous_sibling) => {
                    let y = self.positions[previous_sibling].y + 1.0;
                    self.positions[this].y = y;
                    self.positions[this].mod_ = y - midpoint;
                }
                None => self.positions[this].y = midpoint,
            }
        }
    }

    fn second_walk(&mut self, this: usize, mod_sum: f32, depth: i32, mut min: f32) -> f32 {
        let position = &mut self.positions[this];
        position.y += mod_sum;
        position.x = depth;
        if position.y < min {
            min = position.y;
        }
        let mod_ = position.mod_;
        for child in self.positions[this].children.clone() {
            min = self.second_walk(child, mod_sum + mod_, depth + 1, min);
        }
        min
    }

    fn third_walk(&mut self, this: usize, offset: f32) {
        self.positions[this].y += offset;
        for child in self.positions[this].children.clone() {
            self.third_walk(child, offset);
        }
    }

    fn execute_shifts(&mut self, this: usize) {
        let mut shift = 0.0f32;
        let mut change = 0.0f32;
        for i in (0..self.positions[this].children.len()).rev() {
            let child = self.positions[this].children[i];
            let child = &mut self.positions[child];
            child.y += shift;
            child.mod_ += shift;
            change += child.change;
            shift += child.shift + change;
        }
    }

    fn previous_or_thread(&self, this: usize) -> Option<usize> {
        let position = &self.positions[this];
        position.thread.or_else(|| position.children.first().copied())
    }

    fn next_or_thread(&self, this: usize) -> Option<usize> {
        let position = &self.positions[this];
        position.thread.or_else(|| position.children.last().copied())
    }

    fn apportion(&mut self, this: usize, mut default_ancestor: usize) -> usize {
        let Some(previous_sibling) = self.positions[this].previous_sibling else { return default_ancestor };
        let parent = self.positions[this].parent.expect("sibling has a parent");
        let mut vir = this;
        let mut vor = this;
        let mut vil = previous_sibling;
        let mut vol = self.positions[parent].children[0];
        let mut sir = self.positions[this].mod_;
        let mut sor = self.positions[this].mod_;
        let mut sil = self.positions[vil].mod_;
        let mut sol = self.positions[vol].mod_;
        while let (Some(next_vil), Some(next_vir)) = (self.next_or_thread(vil), self.previous_or_thread(vir)) {
            vil = next_vil;
            vir = next_vir;
            vol = self.previous_or_thread(vol).expect("left contour");
            vor = self.next_or_thread(vor).expect("right contour");
            self.positions[vor].ancestor = this;
            let shift = self.positions[vil].y + sil - (self.positions[vir].y + sir) + 1.0;
            if shift > 0.0 {
                let ancestor = self.get_ancestor(vil, this, default_ancestor);
                self.move_subtree(ancestor, this, shift);
                sir += shift;
                sor += shift;
            }
            sil += self.positions[vil].mod_;
            sir += self.positions[vir].mod_;
            sol += self.positions[vol].mod_;
            sor += self.positions[vor].mod_;
        }
        if self.next_or_thread(vil).is_some() && self.next_or_thread(vor).is_none() {
            self.positions[vor].thread = self.next_or_thread(vil);
            self.positions[vor].mod_ += sil - sor;
        } else {
            if self.previous_or_thread(vir).is_some() && self.previous_or_thread(vol).is_none() {
                self.positions[vol].thread = self.previous_or_thread(vir);
                self.positions[vol].mod_ += sir - sol;
            }
            default_ancestor = this;
        }
        default_ancestor
    }

    fn move_subtree(&mut self, this: usize, right: usize, shift: f32) {
        let subtrees = (self.positions[right].child_index - self.positions[this].child_index) as f32;
        if subtrees != 0.0 {
            self.positions[right].change -= shift / subtrees;
            self.positions[this].change += shift / subtrees;
        }
        let right = &mut self.positions[right];
        right.shift += shift;
        right.y += shift;
        right.mod_ += shift;
    }

    fn get_ancestor(&self, this: usize, other: usize, default_ancestor: usize) -> usize {
        let ancestor = self.positions[this].ancestor;
        let other_parent = self.positions[other].parent.expect("sibling has a parent");
        if self.positions[other_parent].children.contains(&ancestor) { ancestor } else { default_ancestor }
    }

    fn finalize_position(&self, tree: &mut AdvancementTree, this: usize) {
        let position = &self.positions[this];
        tree.node_mut(position.node).set_location(position.x as f32, position.y);
        for &child in &position.children {
            self.finalize_position(tree, child);
        }
    }

    pub fn run(tree: &mut AdvancementTree, node: usize) {
        if tree.node(node).advancement().display.is_none() {
            panic!("Can't position children of an invisible root!");
        }
        let mut positions = TreeNodePosition { positions: Vec::new() };
        let root = positions.create(tree, node, None, None, 1, 0);
        positions.first_walk(root);
        let root_y = positions.positions[root].y;
        let min = positions.second_walk(root, 0.0, 0, root_y);
        if min < 0.0 {
            positions.third_walk(root, -min);
        }
        positions.finalize_position(tree, root);
    }
}
