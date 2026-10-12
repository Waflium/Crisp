use super::ItemStack;
use super::item_provider::ItemProvider;

pub struct ArrayItemProvider {
    items: Vec<ItemStack>,
    position: usize,
    peeked_item: Option<ItemStack>,
    cycled: bool,
}

impl ArrayItemProvider {
    pub fn new(items: Vec<ItemStack>) -> ArrayItemProvider {
        ArrayItemProvider { items, position: 0, peeked_item: None, cycled: false }
    }

    pub fn cycled(items: Vec<ItemStack>) -> ArrayItemProvider {
        ArrayItemProvider { items, position: 0, peeked_item: None, cycled: true }
    }

    fn compute_next_item(&self) -> ItemStack {
        if self.cycled { self.items[self.position % self.items.len()].clone() } else { self.items[self.position].clone() }
    }

    fn set_position(&mut self, new_position: usize) {
        if self.position != new_position {
            self.position = new_position;
            self.peeked_item = None;
        }
    }
}

impl ItemProvider for ArrayItemProvider {
    fn has_next(&self) -> bool {
        if self.cycled { !self.items.is_empty() } else { self.position < self.items.len() }
    }

    fn restart(&mut self) {
        self.set_position(0);
    }

    fn next(&mut self) -> ItemStack {
        assert!(self.has_next(), "java.util.NoSuchElementException");
        let next = match self.peeked_item.take() {
            Some(peeked_item) => peeked_item,
            None => self.compute_next_item(),
        };
        self.position += 1;
        next
    }

    fn peek(&mut self) -> ItemStack {
        assert!(self.has_next(), "java.util.NoSuchElementException");
        if self.peeked_item.is_none() {
            self.peeked_item = Some(self.compute_next_item());
        }
        self.peeked_item.clone().unwrap()
    }

    fn find_next(&mut self, predicate: &dyn Fn(&ItemStack) -> bool) -> bool {
        if self.cycled {
            for offset in 0..self.items.len() {
                let index = (self.position + offset) % self.items.len();
                if predicate(&self.items[index]) {
                    self.set_position(self.position + offset);
                    return true;
                }
            }
            return false;
        }
        for i in self.position..self.items.len() {
            if predicate(&self.items[i]) {
                self.set_position(i);
                return true;
            }
        }
        false
    }
}
