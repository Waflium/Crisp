use crate::world::item::ItemStack;

pub struct SlotCollection {
    items: Vec<ItemStack>,
}

impl SlotCollection {
    pub const EMPTY: SlotCollection = SlotCollection { items: Vec::new() };

    pub fn of(items: Vec<ItemStack>) -> SlotCollection {
        SlotCollection { items }
    }

    pub fn item_copies(self) -> Vec<ItemStack> {
        self.items
    }

    pub fn filter(mut self, predicate: impl Fn(&ItemStack) -> bool) -> SlotCollection {
        self.items.retain(|item| predicate(item));
        self
    }

    pub fn flat_map(self, mut mapper: impl FnMut(ItemStack) -> SlotCollection) -> SlotCollection {
        SlotCollection { items: self.items.into_iter().flat_map(|item| mapper(item).items).collect() }
    }

    pub fn limit(mut self, limit: usize) -> SlotCollection {
        self.items.truncate(limit);
        self
    }

    pub fn concat(terms: Vec<SlotCollection>) -> SlotCollection {
        SlotCollection { items: terms.into_iter().flat_map(|term| term.items).collect() }
    }
}
