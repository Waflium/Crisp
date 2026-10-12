use crisp_data::{Item, items};
use crisp_protocol::nbt::Nbt;

use super::display::slot_display::SlotDisplay;
use crate::core::holder_set::HolderSet;
use crate::util::codec::DataResult;
use crate::world::entity::player::stacked_contents::IngredientInfo;
use crate::world::item::ItemStack;

#[derive(Clone, Debug, PartialEq)]
pub struct Ingredient {
    values: HolderSet,
}

impl Ingredient {
    pub fn decode(tag: &Nbt) -> DataResult<Ingredient> {
        let values = HolderSet::decode("minecraft:item", tag)?;
        if let HolderSet::Direct(direct_values) = &values {
            if direct_values.is_empty() {
                return Err("List must have contents".into());
            }
            if direct_values.contains(&(items::AIR.0 as i32)) {
                return Err("Ingredient can't contain air".into());
            }
        }
        Ok(Ingredient { values })
    }

    pub fn test_optional_ingredient(ingredient: Option<&Ingredient>, stack: &ItemStack) -> bool {
        match ingredient {
            Some(value) => value.test(stack),
            None => stack.is_empty(),
        }
    }

    pub fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.values.contents().iter().map(|&id| Item(id as u16))
    }

    pub fn is_empty(&self) -> bool {
        self.values.contents().is_empty()
    }

    pub fn test(&self, input: &ItemStack) -> bool {
        self.values.contains(input.get_item().0 as i32)
    }

    pub fn accepts_item(&self, item: Item) -> bool {
        self.values.contains(item.0 as i32)
    }

    pub fn of(item: Item) -> Ingredient {
        Ingredient { values: HolderSet::Direct(vec![item.0 as i32]) }
    }

    pub fn values(&self) -> &HolderSet {
        &self.values
    }

    pub fn display(&self) -> SlotDisplay {
        SlotDisplay::TagSlotDisplay(self.values.clone())
    }

    pub fn optional_ingredient_to_display(ingredient: Option<&Ingredient>) -> SlotDisplay {
        ingredient.map_or(SlotDisplay::Empty, Ingredient::display)
    }

    pub fn get_single_item(&self) -> Option<Item> {
        match self.values.contents() {
            [single] => Some(Item(*single as u16)),
            _ => None,
        }
    }
}

impl IngredientInfo<Item> for Ingredient {
    fn accepts_item(&self, item: Item) -> bool {
        Ingredient::accepts_item(self, item)
    }
}
