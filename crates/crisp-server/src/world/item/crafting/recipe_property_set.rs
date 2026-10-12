use crisp_data::Item;

use super::ingredient::Ingredient;
use crate::world::item::ItemStack;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecipePropertySetKey {
    SmithingBase,
    SmithingTemplate,
    SmithingAddition,
    FurnaceInput,
    BlastFurnaceInput,
    SmokerInput,
    CampfireInput,
    BrewingInputs,
    BrewingReagents,
}

impl RecipePropertySetKey {
    pub const VALUES: [RecipePropertySetKey; 9] = [
        RecipePropertySetKey::SmithingBase,
        RecipePropertySetKey::SmithingTemplate,
        RecipePropertySetKey::SmithingAddition,
        RecipePropertySetKey::FurnaceInput,
        RecipePropertySetKey::BlastFurnaceInput,
        RecipePropertySetKey::SmokerInput,
        RecipePropertySetKey::CampfireInput,
        RecipePropertySetKey::BrewingInputs,
        RecipePropertySetKey::BrewingReagents,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RecipePropertySetKey::SmithingBase => "minecraft:smithing_base",
            RecipePropertySetKey::SmithingTemplate => "minecraft:smithing_template",
            RecipePropertySetKey::SmithingAddition => "minecraft:smithing_addition",
            RecipePropertySetKey::FurnaceInput => "minecraft:furnace_input",
            RecipePropertySetKey::BlastFurnaceInput => "minecraft:blast_furnace_input",
            RecipePropertySetKey::SmokerInput => "minecraft:smoker_input",
            RecipePropertySetKey::CampfireInput => "minecraft:campfire_input",
            RecipePropertySetKey::BrewingInputs => "minecraft:brewing_input",
            RecipePropertySetKey::BrewingReagents => "minecraft:brewing_reagent",
        }
    }
}

pub const SMITHING_BASE: RecipePropertySetKey = RecipePropertySetKey::SmithingBase;
pub const SMITHING_TEMPLATE: RecipePropertySetKey = RecipePropertySetKey::SmithingTemplate;
pub const SMITHING_ADDITION: RecipePropertySetKey = RecipePropertySetKey::SmithingAddition;
pub const FURNACE_INPUT: RecipePropertySetKey = RecipePropertySetKey::FurnaceInput;
pub const BLAST_FURNACE_INPUT: RecipePropertySetKey = RecipePropertySetKey::BlastFurnaceInput;
pub const SMOKER_INPUT: RecipePropertySetKey = RecipePropertySetKey::SmokerInput;
pub const CAMPFIRE_INPUT: RecipePropertySetKey = RecipePropertySetKey::CampfireInput;
pub const BREWING_INPUTS: RecipePropertySetKey = RecipePropertySetKey::BrewingInputs;
pub const BREWING_REAGENTS: RecipePropertySetKey = RecipePropertySetKey::BrewingReagents;

pub struct RecipePropertySet {
    items: Vec<bool>,
}

impl RecipePropertySet {
    pub fn empty() -> RecipePropertySet {
        RecipePropertySet { items: vec![false; Item::count()] }
    }

    pub fn test(&self, item_stack: &ItemStack) -> bool {
        self.items[item_stack.get_item().0 as usize]
    }

    pub fn create(ingredients: &[Ingredient]) -> RecipePropertySet {
        let mut set = RecipePropertySet::empty();
        for item in ingredients.iter().flat_map(Ingredient::items) {
            set.items[item.0 as usize] = true;
        }
        set
    }

    pub fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.items.iter().enumerate().filter(|&(_, present)| *present).map(|(id, _)| Item(id as u16))
    }
}
