use crisp_protocol::nbt::Nbt;

use super::values::{self, ItemStackTemplate};
use crate::util::fraction::{ArithmeticResult, Fraction};
use crate::world::item::component as dc;
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

pub struct Mutable {
    items: Vec<ItemStack>,
    weight: Fraction,
}

impl Mutable {
    pub fn new() -> Mutable {
        Mutable { items: Vec::new(), weight: Fraction::ZERO }
    }

    fn find_stack_index(&self, items_to_add: &ItemStack) -> Option<usize> {
        if !items_to_add.is_stackable() {
            return None;
        }
        self.items.iter().position(|stack| ItemStack::is_same_item_same_components(stack, items_to_add))
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

    pub fn to_immutable(&self) -> Nbt {
        Nbt::List(
            self.items
                .iter()
                .map(|item| ItemStackTemplate { item: item.get_item().name().to_string(), count: item.count, components: item.components_patch().to_nbt() }.to_nbt())
                .collect(),
        )
    }
}
