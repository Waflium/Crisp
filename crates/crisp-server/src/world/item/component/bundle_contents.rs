use crisp_protocol::nbt::Nbt;

use super::growable_mutable_container::{self, GrowableMutableContainer};
use super::simple_mutable_container::{self, SimpleMutableContainer, SlotConsumer};
use super::values::{self, ItemStackTemplate};
use crate::server::level::ServerLevel;
use crate::util::fraction::{ArithmeticResult, Fraction};
use crate::world::item::component as dc;
use crate::world::item::item_provider::ItemProvider;
use crate::world::item::slot::slot_selector::SlotSelector;
use crate::world::item::{self, ItemStack};

const BUNDLE_IN_BUNDLE_WEIGHT: Fraction = Fraction::get_fraction(1, 16).unwrap();

fn compute_content_weight(items: &[ItemStack]) -> ArithmeticResult<Fraction> {
    let mut weight = Fraction::ZERO;
    for stack in items {
        let item_weight = get_weight(stack)?;
        weight = weight.add(item_weight.multiply_by(Fraction::get_fraction(stack.count, 1)?)?)?;
    }
    Some(weight)
}

pub fn weight(contents: &Nbt) -> ArithmeticResult<Fraction> {
    compute_content_weight(&item_copies(contents))
}

pub fn item_copies(contents: &Nbt) -> Vec<ItemStack> {
    values::templates(contents).iter().map(|template| template.create().unwrap_or_else(ItemStack::empty)).collect()
}

fn get_weight(item: &ItemStack) -> ArithmeticResult<Fraction> {
    if let Some(bundle) = item.get(dc::BUNDLE_CONTENTS) {
        return weight(bundle)?.add(BUNDLE_IN_BUNDLE_WEIGHT);
    }
    let has_bees = item.get(dc::BEES).is_some_and(|bees| !bees.as_list().is_empty());
    if has_bees { Some(Fraction::ONE) } else { Fraction::get_fraction(1, item.max_stack_size()) }
}

pub fn can_item_be_in_bundle(item_to_add: &ItemStack) -> bool {
    !item_to_add.is_empty() && item::can_fit_inside_container_items(item_to_add.get_item())
}

pub fn template_of(item: &ItemStack) -> Nbt {
    ItemStackTemplate { item: item.get_item().name().to_string(), count: item.count, components: item.components_patch().to_nbt() }.to_nbt()
}

pub fn as_mutable(contents: &Nbt) -> Mutable {
    match weight(contents) {
        Some(weight) => Mutable { items: item_copies(contents), weight, needs_flattening: false },
        None => Mutable::new(),
    }
}

fn get_stacked_weight(weight: Fraction, count: i32) -> Fraction {
    Fraction::get_fraction(count, 1).and_then(|count| weight.multiply_by(count)).expect("Fraction overflow")
}

fn get_stacked_weight_of(stack: &ItemStack) -> Fraction {
    get_stacked_weight(get_weight(stack).expect("Excessive total bundle weight"), stack.count)
}

fn get_weight_with_added_items(weight: Fraction, items_to_add: &ItemStack) -> Option<Fraction> {
    if can_item_be_in_bundle(items_to_add) {
        let item_weight = get_weight(items_to_add)?;
        let new_weight = weight.add(get_stacked_weight(item_weight, items_to_add.count)).expect("Fraction overflow");
        if new_weight.compare_to(Fraction::ONE).is_le() {
            return Some(new_weight);
        }
    }
    None
}

pub struct Mutable {
    items: Vec<ItemStack>,
    weight: Fraction,
    needs_flattening: bool,
}

impl Mutable {
    pub fn new() -> Mutable {
        Mutable { items: Vec::new(), weight: Fraction::ZERO, needs_flattening: false }
    }

    fn find_stack_index_within_range(&self, items_to_add: &ItemStack, min_inclusive: usize, max_exclusive: usize) -> Option<usize> {
        if !items_to_add.is_stackable() {
            return None;
        }
        (min_inclusive..max_exclusive.min(self.items.len())).find(|&i| ItemStack::is_same_item_same_components(&self.items[i], items_to_add))
    }

    fn find_stack_index(&self, items_to_add: &ItemStack) -> Option<usize> {
        self.find_stack_index_within_range(items_to_add, 0, self.items.len())
    }

    pub fn replace_slot_items(&mut self, new_items: &mut dyn ItemProvider, slot_selector: &SlotSelector) -> i32 {
        self.merge_identical_stacks();
        growable_mutable_container::growable_replace_slot_items(self, new_items, slot_selector)
    }

    pub fn modify_slots(&mut self, level: &mut ServerLevel, consumer: &mut SlotConsumer, slot_selector: &SlotSelector) {
        self.merge_identical_stacks();
        simple_mutable_container::simple_modify_slots(self, level, consumer, slot_selector);
    }

    fn merge_identical_stacks(&mut self) {
        if !self.needs_flattening {
            return;
        }
        let mut index = 0;
        while index < self.items.len() {
            let item_stack = self.items[index].clone();
            if item_stack.is_empty() {
                self.items.remove(index);
                continue;
            }
            if let Some(stack_index) = self.find_stack_index_within_range(&item_stack, index + 1, self.items.len()) {
                let merged_count = item_stack.count + self.items[stack_index].count;
                self.items[stack_index] = item_stack.copy_with_count(merged_count);
                self.items.remove(index);
                continue;
            }
            index += 1;
        }
        self.needs_flattening = false;
    }

    fn get_max_amount_to_add(&self, item_weight: Fraction) -> ArithmeticResult<i32> {
        let remaining_weight = Fraction::ONE.subtract(self.weight)?;
        Some(remaining_weight.divide_by(item_weight)?.int_value().max(0))
    }

    pub fn try_insert(&mut self, items_to_add: &mut ItemStack) -> i32 {
        if !can_item_be_in_bundle(items_to_add) {
            return 0;
        }
        let Some(item_weight) = get_weight(items_to_add) else { return 0 };
        let amount_to_add = items_to_add.count.min(self.get_max_amount_to_add(item_weight).expect("Fraction overflow"));
        if amount_to_add == 0 {
            return 0;
        }
        self.weight = Fraction::get_fraction(amount_to_add, 1).and_then(|count| item_weight.multiply_by(count)).and_then(|stacked| self.weight.add(stacked)).expect("Fraction overflow");
        match self.find_stack_index(items_to_add) {
            Some(stack_index) => {
                let removed_stack = self.items.remove(stack_index);
                let merged_stack = removed_stack.copy_with_count(removed_stack.count + amount_to_add);
                items_to_add.shrink(amount_to_add);
                self.items.insert(0, merged_stack);
            }
            None => self.items.insert(0, items_to_add.split(amount_to_add)),
        }
        amount_to_add
    }

    pub fn to_immutable(&mut self) -> Nbt {
        self.merge_identical_stacks();
        Nbt::List(self.items.iter().map(template_of).collect())
    }
}

impl SimpleMutableContainer for Mutable {
    fn items(&mut self) -> &mut Vec<ItemStack> {
        &mut self.items
    }

    fn set_item(&mut self, slot: usize, item_stack: ItemStack) -> bool {
        let current_item = &self.items[slot];
        let adjusted_weight = if current_item.is_empty() { self.weight } else { self.weight.subtract(get_stacked_weight_of(current_item)).expect("Fraction overflow") };
        let new_weight = if item_stack.is_empty() { Some(adjusted_weight) } else { get_weight_with_added_items(adjusted_weight, &item_stack) };
        let Some(new_weight) = new_weight else { return false };
        self.items[slot] = item_stack;
        self.weight = new_weight;
        self.needs_flattening = true;
        true
    }
}

impl GrowableMutableContainer for Mutable {
    fn can_insert_new_slots(&mut self) -> bool {
        self.weight.compare_to(Fraction::ONE).is_lt()
    }

    fn add_slot_with_item(&mut self, new_items: &mut dyn ItemProvider) -> bool {
        if !new_items.find_next_non_empty() {
            return false;
        }
        let Some(new_weight) = get_weight_with_added_items(self.weight, &new_items.peek()) else { return false };
        let next = new_items.next();
        self.items.push(next);
        self.weight = new_weight;
        self.needs_flattening = true;
        true
    }
}
