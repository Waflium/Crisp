use super::ItemStack;
use super::array_item_provider::ArrayItemProvider;

pub trait ItemProvider {
    fn has_next(&self) -> bool;

    fn restart(&mut self);

    fn next(&mut self) -> ItemStack;

    fn peek(&mut self) -> ItemStack;

    fn find_next(&mut self, predicate: &dyn Fn(&ItemStack) -> bool) -> bool;

    fn find_next_non_empty(&mut self) -> bool {
        self.find_next(&|s| !s.is_empty())
    }
}

pub fn of(items: Vec<ItemStack>) -> Box<dyn ItemProvider> {
    Box::new(ArrayItemProvider::new(items))
}

pub fn cycle(items: Vec<ItemStack>) -> Box<dyn ItemProvider> {
    Box::new(ArrayItemProvider::cycled(items))
}

pub fn or_else_provide(item_provider: Box<dyn ItemProvider>, fallback_item: ItemStack) -> Box<dyn ItemProvider> {
    Box::new(FallbackItemProvider { item_provider, fallback_item, is_using_fallback: false })
}

pub struct FallbackItemProvider {
    item_provider: Box<dyn ItemProvider>,
    fallback_item: ItemStack,
    is_using_fallback: bool,
}

impl ItemProvider for FallbackItemProvider {
    fn has_next(&self) -> bool {
        true
    }

    fn restart(&mut self) {
        self.item_provider.restart();
        self.is_using_fallback = false;
    }

    fn peek(&mut self) -> ItemStack {
        if !self.is_using_fallback {
            if self.item_provider.has_next() {
                return self.item_provider.peek();
            }
            self.is_using_fallback = true;
        }
        self.fallback_item.clone()
    }

    fn next(&mut self) -> ItemStack {
        if !self.is_using_fallback {
            if self.item_provider.has_next() {
                return self.item_provider.next();
            }
            self.is_using_fallback = true;
        }
        self.fallback_item.clone()
    }

    fn find_next(&mut self, predicate: &dyn Fn(&ItemStack) -> bool) -> bool {
        if self.is_using_fallback {
            predicate(&self.fallback_item)
        } else if self.item_provider.find_next(predicate) {
            true
        } else if predicate(&self.fallback_item) {
            self.is_using_fallback = true;
            true
        } else {
            false
        }
    }
}
