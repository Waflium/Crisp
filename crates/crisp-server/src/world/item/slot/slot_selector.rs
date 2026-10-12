use std::cell::Cell;
use std::rc::Rc;

use crate::world::item::ItemStack;

pub type ItemFilter<'s> = Rc<dyn Fn(&ItemStack) -> bool + 's>;

#[derive(Clone)]
pub enum SlotSelector<'s> {
    Filtering(Option<ItemFilter<'s>>),
    Tracking { slot_selector: Box<SlotSelector<'s>>, limit: i32, selected_count: Rc<Cell<i32>> },
}

pub const ANY_SLOT: SlotSelector<'static> = SlotSelector::Filtering(None);

pub fn non_empty_slots() -> SlotSelector<'static> {
    selecting(Rc::new(|s: &ItemStack| !s.is_empty()))
}

pub fn selecting(predicate: ItemFilter) -> SlotSelector {
    SlotSelector::Filtering(Some(predicate))
}

pub fn tracking(slot_selector: SlotSelector, selected_count: Rc<Cell<i32>>) -> SlotSelector {
    SlotSelector::Tracking { slot_selector: Box::new(slot_selector), limit: i32::MAX, selected_count }
}

impl<'s> SlotSelector<'s> {
    pub fn try_select_slot(&self, item_in_slot: &ItemStack) -> bool {
        match self {
            SlotSelector::Filtering(filter) => filter.as_ref().is_none_or(|filter| filter(item_in_slot)),
            SlotSelector::Tracking { slot_selector, limit, selected_count } => {
                if selected_count.get() < *limit && slot_selector.try_select_slot(item_in_slot) {
                    selected_count.set(selected_count.get() + 1);
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn filter(&self, predicate: ItemFilter<'s>) -> SlotSelector<'s> {
        match self {
            SlotSelector::Filtering(None) => SlotSelector::Filtering(Some(predicate)),
            SlotSelector::Filtering(Some(filter)) => {
                let filter = filter.clone();
                SlotSelector::Filtering(Some(Rc::new(move |t: &ItemStack| filter(t) && predicate(t))))
            }
            SlotSelector::Tracking { slot_selector, limit, selected_count } => {
                SlotSelector::Tracking { slot_selector: Box::new(slot_selector.filter(predicate)), limit: *limit, selected_count: selected_count.clone() }
            }
        }
    }

    pub fn limit(&self, limit: i32) -> SlotSelector<'s> {
        match self {
            SlotSelector::Filtering(_) => SlotSelector::Tracking { slot_selector: Box::new(self.clone()), limit, selected_count: Rc::new(Cell::new(0)) },
            SlotSelector::Tracking { slot_selector, limit: own_limit, selected_count } => {
                if limit < *own_limit {
                    SlotSelector::Tracking { slot_selector: slot_selector.clone(), limit, selected_count: selected_count.clone() }
                } else {
                    self.clone()
                }
            }
        }
    }
}
