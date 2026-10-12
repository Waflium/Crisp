use crisp_data::{registry, registry_id};
use crisp_protocol::Writer;

use super::slot_display::SlotDisplay;

#[derive(Clone, Debug)]
pub enum RecipeDisplay {
    ShapelessCrafting { ingredients: Vec<SlotDisplay>, result: SlotDisplay, crafting_station: SlotDisplay },
    ShapedCrafting { width: i32, height: i32, ingredients: Vec<SlotDisplay>, result: SlotDisplay, crafting_station: SlotDisplay },
    Furnace { ingredient: SlotDisplay, fuel: SlotDisplay, result: SlotDisplay, crafting_station: SlotDisplay, duration: i32, experience: f32 },
    Stonecutter { input: SlotDisplay, result: SlotDisplay, crafting_station: SlotDisplay },
    Smithing { template: SlotDisplay, base: SlotDisplay, addition: SlotDisplay, result: SlotDisplay, crafting_station: SlotDisplay },
}

fn write_list(w: &mut Writer, displays: &[SlotDisplay]) {
    w.varint(displays.len() as i32);
    displays.iter().for_each(|display| display.write(w));
}

impl RecipeDisplay {
    fn type_name(&self) -> &'static str {
        match self {
            RecipeDisplay::ShapelessCrafting { .. } => "minecraft:crafting_shapeless",
            RecipeDisplay::ShapedCrafting { .. } => "minecraft:crafting_shaped",
            RecipeDisplay::Furnace { .. } => "minecraft:furnace",
            RecipeDisplay::Stonecutter { .. } => "minecraft:stonecutter",
            RecipeDisplay::Smithing { .. } => "minecraft:smithing",
        }
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(registry_id(registry::RECIPE_DISPLAY, self.type_name()));
        match self {
            RecipeDisplay::ShapelessCrafting { ingredients, result, crafting_station } => {
                write_list(w, ingredients);
                result.write(w);
                crafting_station.write(w);
            }
            RecipeDisplay::ShapedCrafting { width, height, ingredients, result, crafting_station } => {
                w.varint(*width);
                w.varint(*height);
                write_list(w, ingredients);
                result.write(w);
                crafting_station.write(w);
            }
            RecipeDisplay::Furnace { ingredient, fuel, result, crafting_station, duration, experience } => {
                ingredient.write(w);
                fuel.write(w);
                result.write(w);
                crafting_station.write(w);
                w.varint(*duration);
                w.f32(*experience);
            }
            RecipeDisplay::Stonecutter { input, result, crafting_station } => {
                input.write(w);
                result.write(w);
                crafting_station.write(w);
            }
            RecipeDisplay::Smithing { template, base, addition, result, crafting_station } => {
                template.write(w);
                base.write(w);
                addition.write(w);
                result.write(w);
                crafting_station.write(w);
            }
        }
    }
}
