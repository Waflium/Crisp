use crisp_core::random::RandomSource;
use crisp_data::Item;
use crisp_protocol::nbt::Nbt;

use super::ItemStack;
use super::component as dc;
use super::crafting::crafting_input::CraftingInput;
use super::crafting::recipe_manager::RecipeManager;
use crate::server::level::ServerLevel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DyeColor {
    pub id: i32,
    pub name: &'static str,
    texture_diffuse_color: i32,
    firework_color: i32,
}

const fn dye(id: i32, name: &'static str, texture_diffuse_color: i32, firework_color: i32) -> DyeColor {
    DyeColor { id, name, texture_diffuse_color: texture_diffuse_color | 0xFF00_0000u32 as i32, firework_color }
}

impl DyeColor {
    pub const VALUES: [DyeColor; 16] = [
        dye(0, "white", 16383998, 15790320),
        dye(1, "orange", 16351261, 15435844),
        dye(2, "magenta", 13061821, 12801229),
        dye(3, "light_blue", 3847130, 6719955),
        dye(4, "yellow", 16701501, 14602026),
        dye(5, "lime", 8439583, 4312372),
        dye(6, "pink", 15961002, 14188952),
        dye(7, "gray", 4673362, 4408131),
        dye(8, "light_gray", 10329495, 11250603),
        dye(9, "cyan", 1481884, 2651799),
        dye(10, "purple", 8991416, 8073150),
        dye(11, "blue", 3949738, 2437522),
        dye(12, "brown", 8606770, 5320730),
        dye(13, "green", 6192150, 3887386),
        dye(14, "red", 11546150, 11743532),
        dye(15, "black", 1908001, 1973019),
    ];

    pub fn texture_diffuse_color(self) -> i32 {
        self.texture_diffuse_color
    }

    pub fn get_firework_color(self) -> i32 {
        self.firework_color
    }

    pub fn by_name(name: &str) -> Option<DyeColor> {
        DyeColor::VALUES.iter().find(|color| color.name == name).copied()
    }

    pub fn by_id(id: i32) -> DyeColor {
        DyeColor::VALUES.get(id as usize).copied().unwrap_or(DyeColor::VALUES[0])
    }

    pub fn get_mixed_color(level: &mut ServerLevel, dye_color1: DyeColor, dye_color2: DyeColor) -> DyeColor {
        match find_color_mix_in_recipes(level, dye_color1, dye_color2) {
            Some(mixed_color) => mixed_color,
            None => {
                if level.random.next_boolean() {
                    dye_color1
                } else {
                    dye_color2
                }
            }
        }
    }
}

fn items_with_dye(color: DyeColor) -> Vec<Item> {
    (0..Item::count())
        .map(|i| Item(i as u16))
        .filter(|&item| ItemStack::new(item, 1).get(dc::DYE).and_then(Nbt::as_str) == Some(color.name))
        .collect()
}

fn find_color_mix_in_recipes(level: &mut ServerLevel, dye_color1: DyeColor, dye_color2: DyeColor) -> Option<DyeColor> {
    let dye1_items = items_with_dye(dye_color1);
    if dye1_items.is_empty() {
        return None;
    }
    let dye2_items = items_with_dye(dye_color2);
    if dye2_items.is_empty() {
        return None;
    }
    for &dye1_item in &dye1_items {
        for &dye2_item in &dye2_items {
            let mut input = CraftingInput::of(2, 1, &[ItemStack::new(dye1_item, 1), ItemStack::new(dye2_item, 1)]);
            if let Some(found_recipe) = RecipeManager::get().get_crafting_recipe_for(&mut input, level, None) {
                let crafting_result = found_recipe.crafting().expect("a crafting recipe").assemble(&input);
                if let Some(crafted_dye_color) = crafting_result.get(dc::DYE).and_then(Nbt::as_str).and_then(DyeColor::by_name) {
                    return Some(crafted_dye_color);
                }
            }
        }
    }
    None
}
