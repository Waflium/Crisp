use crisp_core::BlockPos;
use crisp_core::random::java_string_hash;
use indexmap::IndexSet;

#[derive(Clone, Debug)]
pub struct JavaHashSet {
    entries: IndexSet<String>,
    capacity: usize,
}

impl Default for JavaHashSet {
    fn default() -> JavaHashSet {
        JavaHashSet { entries: IndexSet::new(), capacity: 16 }
    }
}

impl JavaHashSet {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn insert(&mut self, value: String) -> bool {
        if !self.entries.insert(value) {
            return false;
        }
        if self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
        true
    }

    pub fn remove(&mut self, value: &str) -> bool {
        self.entries.shift_remove(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let bucket = |s: &str| {
            let h = java_string_hash(s);
            (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (self.capacity - 1)
        };
        let mut order: Vec<&str> = self.entries.iter().map(String::as_str).collect();
        order.sort_by_key(|s| bucket(s));
        order.into_iter()
    }
}

pub trait JavaHashCode {
    fn java_hash_code(&self) -> i32;
}

impl JavaHashCode for BlockPos {
    fn java_hash_code(&self) -> i32 {
        self.y.wrapping_add(self.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(self.x)
    }
}

impl JavaHashCode for i32 {
    fn java_hash_code(&self) -> i32 {
        *self
    }
}

impl JavaHashCode for u128 {
    fn java_hash_code(&self) -> i32 {
        let hilo = (*self >> 64) as u64 ^ *self as u64;
        (hilo >> 32) as i32 ^ hilo as i32
    }
}

const NO_NODE: u32 = u32::MAX;
const TREEIFY_THRESHOLD: usize = 8;
const UNTREEIFY_THRESHOLD: usize = 6;
const MIN_TREEIFY_CAPACITY: usize = 64;

#[derive(Clone)]
struct Node<T> {
    value: T,
    hash: i32,
    next: u32,
    prev: u32,
    parent: u32,
    left: u32,
    right: u32,
    red: bool,
    tree: bool,
}

#[derive(Clone)]
pub struct JavaObjectHashSet<T> {
    table: Vec<u32>,
    nodes: Vec<Node<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for JavaObjectHashSet<T> {
    fn default() -> JavaObjectHashSet<T> {
        JavaObjectHashSet { table: vec![NO_NODE; 16], nodes: Vec::new(), free: Vec::new(), len: 0 }
    }
}

impl<T: JavaHashCode + PartialEq + Copy> JavaObjectHashSet<T> {
    fn hash(value: &T) -> i32 {
        let h = value.java_hash_code();
        h ^ ((h as u32) >> 16) as i32
    }

    fn bucket(&self, hash: i32) -> usize {
        hash as u32 as usize & (self.table.len() - 1)
    }

    pub fn with_capacity(initial_capacity: usize) -> JavaObjectHashSet<T> {
        JavaObjectHashSet { table: vec![NO_NODE; initial_capacity.max(1).next_power_of_two()], nodes: Vec::new(), free: Vec::new(), len: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|head| *head = NO_NODE);
        self.nodes.clear();
        self.free.clear();
        self.len = 0;
    }

    pub fn contains(&self, value: &T) -> bool {
        self.find(value).is_some()
    }

    fn find(&self, value: &T) -> Option<u32> {
        let hash = Self::hash(value);
        let mut cursor = self.table[self.bucket(hash)];
        while cursor != NO_NODE {
            let node = &self.nodes[cursor as usize];
            if node.hash == hash && node.value == *value {
                return Some(cursor);
            }
            cursor = node.next;
        }
        None
    }

    fn new_node(&mut self, value: T, hash: i32, next: u32, tree: bool) -> u32 {
        let node = Node { value, hash, next, prev: NO_NODE, parent: NO_NODE, left: NO_NODE, right: NO_NODE, red: false, tree };
        match self.free.pop() {
            Some(id) => {
                self.nodes[id as usize] = node;
                id
            }
            None => {
                self.nodes.push(node);
                (self.nodes.len() - 1) as u32
            }
        }
    }

    pub fn insert(&mut self, value: T) -> bool {
        let hash = Self::hash(&value);
        let index = self.bucket(hash);
        let head = self.table[index];
        if head == NO_NODE {
            self.table[index] = self.new_node(value, hash, NO_NODE, false);
        } else if self.nodes[head as usize].tree {
            if !self.put_tree_val(index, hash, value) {
                return false;
            }
        } else {
            let mut p = head;
            if self.nodes[p as usize].hash == hash && self.nodes[p as usize].value == value {
                return false;
            }
            let mut bin_count = 0;
            loop {
                let e = self.nodes[p as usize].next;
                if e == NO_NODE {
                    let id = self.new_node(value, hash, NO_NODE, false);
                    self.nodes[p as usize].next = id;
                    if bin_count >= TREEIFY_THRESHOLD - 1 {
                        self.treeify_bin(hash);
                    }
                    break;
                }
                if self.nodes[e as usize].hash == hash && self.nodes[e as usize].value == value {
                    return false;
                }
                p = e;
                bin_count += 1;
            }
        }
        self.len += 1;
        if self.len > self.table.len() * 3 / 4 {
            self.resize();
        }
        true
    }

    fn treeify_bin(&mut self, hash: i32) {
        if self.table.len() < MIN_TREEIFY_CAPACITY {
            self.resize();
            return;
        }
        let index = self.bucket(hash);
        let mut previous = NO_NODE;
        let mut cursor = self.table[index];
        while cursor != NO_NODE {
            let node = &mut self.nodes[cursor as usize];
            node.tree = true;
            node.prev = previous;
            previous = cursor;
            cursor = node.next;
        }
        self.treeify(index);
    }

    fn treeify(&mut self, index: usize) {
        let mut root = NO_NODE;
        let mut x = self.table[index];
        while x != NO_NODE {
            let next = self.nodes[x as usize].next;
            self.nodes[x as usize].left = NO_NODE;
            self.nodes[x as usize].right = NO_NODE;
            if root == NO_NODE {
                self.nodes[x as usize].parent = NO_NODE;
                self.nodes[x as usize].red = false;
                root = x;
            } else {
                let h = self.nodes[x as usize].hash;
                let mut p = root;
                loop {
                    let ph = self.nodes[p as usize].hash;
                    let dir = if ph > h { -1 } else if ph < h { 1 } else { -1 };
                    let xp = p;
                    p = if dir <= 0 { self.nodes[p as usize].left } else { self.nodes[p as usize].right };
                    if p == NO_NODE {
                        self.nodes[x as usize].parent = xp;
                        if dir <= 0 {
                            self.nodes[xp as usize].left = x;
                        } else {
                            self.nodes[xp as usize].right = x;
                        }
                        root = self.balance_insertion(root, x);
                        break;
                    }
                }
            }
            x = next;
        }
        self.move_root_to_front(root);
    }

    fn untreeify(&mut self, head: u32) -> u32 {
        let mut cursor = head;
        while cursor != NO_NODE {
            let node = &mut self.nodes[cursor as usize];
            node.tree = false;
            node.prev = NO_NODE;
            node.parent = NO_NODE;
            node.left = NO_NODE;
            node.right = NO_NODE;
            node.red = false;
            cursor = node.next;
        }
        head
    }

    fn root_of(&self, mut node: u32) -> u32 {
        loop {
            let parent = self.nodes[node as usize].parent;
            if parent == NO_NODE {
                return node;
            }
            node = parent;
        }
    }

    fn tree_find(&self, start: u32, hash: i32, value: &T) -> Option<u32> {
        let mut p = start;
        while p != NO_NODE {
            let node = &self.nodes[p as usize];
            if node.hash > hash {
                p = node.left;
            } else if node.hash < hash {
                p = node.right;
            } else if node.value == *value {
                return Some(p);
            } else if node.left == NO_NODE {
                p = node.right;
            } else if node.right == NO_NODE {
                p = node.left;
            } else if let Some(q) = self.tree_find(node.right, hash, value) {
                return Some(q);
            } else {
                p = node.left;
            }
        }
        None
    }

    fn put_tree_val(&mut self, index: usize, hash: i32, value: T) -> bool {
        let head = self.table[index];
        let root = self.root_of(head);
        let mut searched = false;
        let mut p = root;
        loop {
            let ph = self.nodes[p as usize].hash;
            let dir = if ph > hash {
                -1
            } else if ph < hash {
                1
            } else if self.nodes[p as usize].value == value {
                return false;
            } else {
                if !searched {
                    searched = true;
                    let (left, right) = (self.nodes[p as usize].left, self.nodes[p as usize].right);
                    if self.tree_find(left, hash, &value).is_some() || self.tree_find(right, hash, &value).is_some() {
                        return false;
                    }
                }
                -1
            };
            let xp = p;
            p = if dir <= 0 { self.nodes[p as usize].left } else { self.nodes[p as usize].right };
            if p == NO_NODE {
                let xpn = self.nodes[xp as usize].next;
                let x = self.new_node(value, hash, xpn, true);
                if dir <= 0 {
                    self.nodes[xp as usize].left = x;
                } else {
                    self.nodes[xp as usize].right = x;
                }
                self.nodes[xp as usize].next = x;
                self.nodes[x as usize].parent = xp;
                self.nodes[x as usize].prev = xp;
                if xpn != NO_NODE {
                    self.nodes[xpn as usize].prev = x;
                }
                let root = self.balance_insertion(root, x);
                self.move_root_to_front(root);
                return true;
            }
        }
    }

    fn move_root_to_front(&mut self, root: u32) {
        if root == NO_NODE {
            return;
        }
        let index = self.bucket(self.nodes[root as usize].hash);
        let first = self.table[index];
        if root != first {
            self.table[index] = root;
            let rp = self.nodes[root as usize].prev;
            let rn = self.nodes[root as usize].next;
            if rn != NO_NODE {
                self.nodes[rn as usize].prev = rp;
            }
            if rp != NO_NODE {
                self.nodes[rp as usize].next = rn;
            }
            if first != NO_NODE {
                self.nodes[first as usize].prev = root;
            }
            self.nodes[root as usize].next = first;
            self.nodes[root as usize].prev = NO_NODE;
        }
    }

    fn red(&self, node: u32) -> bool {
        node != NO_NODE && self.nodes[node as usize].red
    }

    fn rotate_left(&mut self, mut root: u32, p: u32) -> u32 {
        if p == NO_NODE {
            return root;
        }
        let r = self.nodes[p as usize].right;
        if r == NO_NODE {
            return root;
        }
        let rl = self.nodes[r as usize].left;
        self.nodes[p as usize].right = rl;
        if rl != NO_NODE {
            self.nodes[rl as usize].parent = p;
        }
        let pp = self.nodes[p as usize].parent;
        self.nodes[r as usize].parent = pp;
        if pp == NO_NODE {
            root = r;
            self.nodes[r as usize].red = false;
        } else if self.nodes[pp as usize].left == p {
            self.nodes[pp as usize].left = r;
        } else {
            self.nodes[pp as usize].right = r;
        }
        self.nodes[r as usize].left = p;
        self.nodes[p as usize].parent = r;
        root
    }

    fn rotate_right(&mut self, mut root: u32, p: u32) -> u32 {
        if p == NO_NODE {
            return root;
        }
        let l = self.nodes[p as usize].left;
        if l == NO_NODE {
            return root;
        }
        let lr = self.nodes[l as usize].right;
        self.nodes[p as usize].left = lr;
        if lr != NO_NODE {
            self.nodes[lr as usize].parent = p;
        }
        let pp = self.nodes[p as usize].parent;
        self.nodes[l as usize].parent = pp;
        if pp == NO_NODE {
            root = l;
            self.nodes[l as usize].red = false;
        } else if self.nodes[pp as usize].right == p {
            self.nodes[pp as usize].right = l;
        } else {
            self.nodes[pp as usize].left = l;
        }
        self.nodes[l as usize].right = p;
        self.nodes[p as usize].parent = l;
        root
    }

    fn balance_insertion(&mut self, mut root: u32, mut x: u32) -> u32 {
        self.nodes[x as usize].red = true;
        loop {
            let mut xp = self.nodes[x as usize].parent;
            if xp == NO_NODE {
                self.nodes[x as usize].red = false;
                return x;
            }
            if !self.nodes[xp as usize].red {
                return root;
            }
            let mut xpp = self.nodes[xp as usize].parent;
            if xpp == NO_NODE {
                return root;
            }
            let xppl = self.nodes[xpp as usize].left;
            if xp == xppl {
                let xppr = self.nodes[xpp as usize].right;
                if self.red(xppr) {
                    self.nodes[xppr as usize].red = false;
                    self.nodes[xp as usize].red = false;
                    self.nodes[xpp as usize].red = true;
                    x = xpp;
                } else {
                    if x == self.nodes[xp as usize].right {
                        x = xp;
                        root = self.rotate_left(root, x);
                        xp = self.nodes[x as usize].parent;
                        xpp = if xp == NO_NODE { NO_NODE } else { self.nodes[xp as usize].parent };
                    }
                    if xp != NO_NODE {
                        self.nodes[xp as usize].red = false;
                        if xpp != NO_NODE {
                            self.nodes[xpp as usize].red = true;
                            root = self.rotate_right(root, xpp);
                        }
                    }
                }
            } else if self.red(xppl) {
                self.nodes[xppl as usize].red = false;
                self.nodes[xp as usize].red = false;
                self.nodes[xpp as usize].red = true;
                x = xpp;
            } else {
                if x == self.nodes[xp as usize].left {
                    x = xp;
                    root = self.rotate_right(root, x);
                    xp = self.nodes[x as usize].parent;
                    xpp = if xp == NO_NODE { NO_NODE } else { self.nodes[xp as usize].parent };
                }
                if xp != NO_NODE {
                    self.nodes[xp as usize].red = false;
                    if xpp != NO_NODE {
                        self.nodes[xpp as usize].red = true;
                        root = self.rotate_left(root, xpp);
                    }
                }
            }
        }
    }

    fn balance_deletion(&mut self, mut root: u32, mut x: u32) -> u32 {
        loop {
            if x == NO_NODE || x == root {
                return root;
            }
            let mut xp = self.nodes[x as usize].parent;
            if xp == NO_NODE {
                self.nodes[x as usize].red = false;
                return x;
            }
            if self.nodes[x as usize].red {
                self.nodes[x as usize].red = false;
                return root;
            }
            let xpl = self.nodes[xp as usize].left;
            if xpl == x {
                let mut xpr = self.nodes[xp as usize].right;
                if self.red(xpr) {
                    self.nodes[xpr as usize].red = false;
                    self.nodes[xp as usize].red = true;
                    root = self.rotate_left(root, xp);
                    xp = self.nodes[x as usize].parent;
                    xpr = if xp == NO_NODE { NO_NODE } else { self.nodes[xp as usize].right };
                }
                if xpr == NO_NODE {
                    x = xp;
                } else {
                    let (sl, sr) = (self.nodes[xpr as usize].left, self.nodes[xpr as usize].right);
                    if !self.red(sr) && !self.red(sl) {
                        self.nodes[xpr as usize].red = true;
                        x = xp;
                    } else {
                        if !self.red(sr) {
                            if sl != NO_NODE {
                                self.nodes[sl as usize].red = false;
                            }
                            self.nodes[xpr as usize].red = true;
                            root = self.rotate_right(root, xpr);
                            xp = self.nodes[x as usize].parent;
                            xpr = if xp == NO_NODE { NO_NODE } else { self.nodes[xp as usize].right };
                        }
                        if xpr != NO_NODE {
                            self.nodes[xpr as usize].red = xp != NO_NODE && self.nodes[xp as usize].red;
                            let sr = self.nodes[xpr as usize].right;
                            if sr != NO_NODE {
                                self.nodes[sr as usize].red = false;
                            }
                        }
                        if xp != NO_NODE {
                            self.nodes[xp as usize].red = false;
                            root = self.rotate_left(root, xp);
                        }
                        x = root;
                    }
                }
            } else {
                let mut xpl = xpl;
                if self.red(xpl) {
                    self.nodes[xpl as usize].red = false;
                    self.nodes[xp as usize].red = true;
                    root = self.rotate_right(root, xp);
                    xp = self.nodes[x as usize].parent;
                    xpl = if xp == NO_NODE { NO_NODE } else { self.nodes[xp as usize].left };
                }
                if xpl == NO_NODE {
                    x = xp;
                } else {
                    let (sl, sr) = (self.nodes[xpl as usize].left, self.nodes[xpl as usize].right);
                    if !self.red(sl) && !self.red(sr) {
                        self.nodes[xpl as usize].red = true;
                        x = xp;
                    } else {
                        if !self.red(sl) {
                            if sr != NO_NODE {
                                self.nodes[sr as usize].red = false;
                            }
                            self.nodes[xpl as usize].red = true;
                            root = self.rotate_left(root, xpl);
                            xp = self.nodes[x as usize].parent;
                            xpl = if xp == NO_NODE { NO_NODE } else { self.nodes[xp as usize].left };
                        }
                        if xpl != NO_NODE {
                            self.nodes[xpl as usize].red = xp != NO_NODE && self.nodes[xp as usize].red;
                            let sl = self.nodes[xpl as usize].left;
                            if sl != NO_NODE {
                                self.nodes[sl as usize].red = false;
                            }
                        }
                        if xp != NO_NODE {
                            self.nodes[xp as usize].red = false;
                            root = self.rotate_right(root, xp);
                        }
                        x = root;
                    }
                }
            }
        }
    }

    fn resize(&mut self) {
        let old_capacity = self.table.len();
        let old_table = std::mem::replace(&mut self.table, vec![NO_NODE; old_capacity * 2]);
        for (j, &head) in old_table.iter().enumerate() {
            if head == NO_NODE {
                continue;
            }
            let tree = self.nodes[head as usize].tree;
            let (mut lo_head, mut lo_tail, mut hi_head, mut hi_tail) = (NO_NODE, NO_NODE, NO_NODE, NO_NODE);
            let (mut lc, mut hc) = (0, 0);
            let mut cursor = head;
            while cursor != NO_NODE {
                let next = self.nodes[cursor as usize].next;
                self.nodes[cursor as usize].next = NO_NODE;
                let (list_head, list_tail, count) = if self.nodes[cursor as usize].hash as u32 as usize & old_capacity == 0 {
                    (&mut lo_head, &mut lo_tail, &mut lc)
                } else {
                    (&mut hi_head, &mut hi_tail, &mut hc)
                };
                self.nodes[cursor as usize].prev = *list_tail;
                if *list_tail == NO_NODE {
                    *list_head = cursor;
                } else {
                    self.nodes[*list_tail as usize].next = cursor;
                }
                *list_tail = cursor;
                *count += 1;
                cursor = next;
            }
            if !tree {
                self.table[j] = lo_head;
                self.table[j + old_capacity] = hi_head;
                continue;
            }
            if lo_head != NO_NODE {
                if lc <= UNTREEIFY_THRESHOLD {
                    self.table[j] = self.untreeify(lo_head);
                } else {
                    self.table[j] = lo_head;
                    if hi_head != NO_NODE {
                        self.treeify(j);
                    }
                }
            }
            if hi_head != NO_NODE {
                if hc <= UNTREEIFY_THRESHOLD {
                    self.table[j + old_capacity] = self.untreeify(hi_head);
                } else {
                    self.table[j + old_capacity] = hi_head;
                    if lo_head != NO_NODE {
                        self.treeify(j + old_capacity);
                    }
                }
            }
        }
    }

    pub fn remove(&mut self, value: &T) -> bool {
        let Some(p) = self.find(value) else { return false };
        let index = self.bucket(self.nodes[p as usize].hash);
        if self.nodes[p as usize].tree {
            self.remove_tree_node(index, p);
        } else {
            let mut cursor = self.table[index];
            let mut previous = NO_NODE;
            while cursor != p {
                previous = cursor;
                cursor = self.nodes[cursor as usize].next;
            }
            let next = self.nodes[p as usize].next;
            if previous == NO_NODE {
                self.table[index] = next;
            } else {
                self.nodes[previous as usize].next = next;
            }
        }
        self.free.push(p);
        self.len -= 1;
        true
    }

    fn remove_tree_node(&mut self, index: usize, p: u32) {
        let mut first = self.table[index];
        let original_first = first;
        let succ = self.nodes[p as usize].next;
        let pred = self.nodes[p as usize].prev;
        if pred == NO_NODE {
            self.table[index] = succ;
            first = succ;
        } else {
            self.nodes[pred as usize].next = succ;
        }
        if succ != NO_NODE {
            self.nodes[succ as usize].prev = pred;
        }
        if first == NO_NODE {
            return;
        }
        let mut root = self.root_of(original_first);
        let root_right = self.nodes[root as usize].right;
        let root_left = self.nodes[root as usize].left;
        if root_right == NO_NODE || root_left == NO_NODE || self.nodes[root_left as usize].left == NO_NODE {
            self.table[index] = self.untreeify(first);
            return;
        }
        let (pl, pr) = (self.nodes[p as usize].left, self.nodes[p as usize].right);
        let replacement;
        if pl != NO_NODE && pr != NO_NODE {
            let mut s = pr;
            while self.nodes[s as usize].left != NO_NODE {
                s = self.nodes[s as usize].left;
            }
            let c = self.nodes[s as usize].red;
            self.nodes[s as usize].red = self.nodes[p as usize].red;
            self.nodes[p as usize].red = c;
            let sr = self.nodes[s as usize].right;
            let pp = self.nodes[p as usize].parent;
            if s == pr {
                self.nodes[p as usize].parent = s;
                self.nodes[s as usize].right = p;
            } else {
                let sp = self.nodes[s as usize].parent;
                self.nodes[p as usize].parent = sp;
                if sp != NO_NODE {
                    if s == self.nodes[sp as usize].left {
                        self.nodes[sp as usize].left = p;
                    } else {
                        self.nodes[sp as usize].right = p;
                    }
                }
                self.nodes[s as usize].right = pr;
                self.nodes[pr as usize].parent = s;
            }
            self.nodes[p as usize].left = NO_NODE;
            self.nodes[p as usize].right = sr;
            if sr != NO_NODE {
                self.nodes[sr as usize].parent = p;
            }
            self.nodes[s as usize].left = pl;
            self.nodes[pl as usize].parent = s;
            self.nodes[s as usize].parent = pp;
            if pp == NO_NODE {
                root = s;
            } else if p == self.nodes[pp as usize].left {
                self.nodes[pp as usize].left = s;
            } else {
                self.nodes[pp as usize].right = s;
            }
            replacement = if sr != NO_NODE { sr } else { p };
        } else if pl != NO_NODE {
            replacement = pl;
        } else if pr != NO_NODE {
            replacement = pr;
        } else {
            replacement = p;
        }
        if replacement != p {
            let pp = self.nodes[p as usize].parent;
            self.nodes[replacement as usize].parent = pp;
            if pp == NO_NODE {
                root = replacement;
                self.nodes[replacement as usize].red = false;
            } else if p == self.nodes[pp as usize].left {
                self.nodes[pp as usize].left = replacement;
            } else {
                self.nodes[pp as usize].right = replacement;
            }
            self.nodes[p as usize].left = NO_NODE;
            self.nodes[p as usize].right = NO_NODE;
            self.nodes[p as usize].parent = NO_NODE;
        }
        let r = if self.nodes[p as usize].red { root } else { self.balance_deletion(root, replacement) };
        if replacement == p {
            let pp = self.nodes[p as usize].parent;
            self.nodes[p as usize].parent = NO_NODE;
            if pp != NO_NODE {
                if p == self.nodes[pp as usize].left {
                    self.nodes[pp as usize].left = NO_NODE;
                } else if p == self.nodes[pp as usize].right {
                    self.nodes[pp as usize].right = NO_NODE;
                }
            }
        }
        self.move_root_to_front(r);
    }

    pub fn first(&self) -> Option<T> {
        self.table.iter().find(|&&head| head != NO_NODE).map(|&head| self.nodes[head as usize].value)
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.table.iter().flat_map(move |&head| {
            std::iter::successors((head != NO_NODE).then_some(head), move |&cursor| {
                let next = self.nodes[cursor as usize].next;
                (next != NO_NODE).then_some(next)
            })
            .map(move |cursor| self.nodes[cursor as usize].value)
        })
    }
}


pub fn identifier_hash(id: &str) -> i32 {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    java_string_hash(namespace).wrapping_mul(31).wrapping_add(java_string_hash(path))
}

#[derive(Clone, Debug)]
pub struct JavaHashMap<V> {
    entries: indexmap::IndexMap<String, V>,
    capacity: usize,
    hash: fn(&str) -> i32,
}

impl<V> JavaHashMap<V> {
    pub fn new(hash: fn(&str) -> i32) -> JavaHashMap<V> {
        JavaHashMap { entries: indexmap::IndexMap::new(), capacity: 16, hash }
    }

    pub fn copy_of(hash: fn(&str) -> i32, entries: impl IntoIterator<Item = (String, V)>) -> JavaHashMap<V> {
        let entries: indexmap::IndexMap<String, V> = entries.into_iter().collect();
        let capacity = if entries.is_empty() { 16 } else { ((entries.len() as f64 / 0.75f32 as f64).ceil() as usize).next_power_of_two() };
        JavaHashMap { entries, capacity, hash }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.values()
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries.get_mut(key)
    }

    pub fn insert(&mut self, key: String, value: V) -> Option<V> {
        let old = self.entries.insert(key, value);
        if old.is_none() && self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
        old
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        self.entries.shift_remove(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        let bucket = |s: &str| {
            let h = (self.hash)(s);
            (h ^ ((h as u32) >> 16) as i32) as u32 as usize & (self.capacity - 1)
        };
        let mut order: Vec<(&str, &V)> = self.entries.iter().map(|(k, v)| (k.as_str(), v)).collect();
        order.sort_by_key(|(k, _)| bucket(k));
        order.into_iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.entries.values_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn java_object_hash_set_matches_java() {
        let expected = std::fs::read_to_string(std::env::var("HASHSET_ORACLE").expect("HASHSET_ORACLE")).unwrap();
        for (trial, line) in expected.lines().enumerate() {
            let mut state = trial as i64;
            let mut next = |bound: i32| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (((state as u64) >> 33) % bound as u64) as i32
            };
            let mut set = JavaObjectHashSet::default();
            let ops = 50 + next(400);
            let spread = 1 + next(64);
            for _ in 0..ops {
                let x = 9584 + next(spread);
                let y = -40 + next(12);
                let dz = next(spread);
                let z = 6976 + dz * if next(3) == 0 { 31 } else { 1 };
                let pos = BlockPos::new(x, y, z);
                if next(5) == 0 {
                    set.remove(&pos);
                } else {
                    set.insert(pos);
                }
            }
            let actual: String = std::iter::once(trial.to_string()).chain(set.iter().map(|p| format!("{},{},{}", p.x, p.y, p.z))).collect::<Vec<_>>().join(" ");
            assert_eq!(actual, line, "trial {trial}");
        }
    }
}
