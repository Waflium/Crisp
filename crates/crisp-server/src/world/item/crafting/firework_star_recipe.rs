use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::dye_recipe::get_dye;
use super::ingredient::Ingredient;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemStackTemplate;

const SHAPES: [&str; 5] = ["small_ball", "large_ball", "star", "creeper", "burst"];

pub struct FireworkStarRecipe {
    shapes: Vec<(&'static str, Ingredient)>,
    trail: Ingredient,
    twinkle: Ingredient,
    fuel: Ingredient,
    dye: Ingredient,
    result: ItemStackTemplate,
}

fn decode_shape(name: &str) -> DataResult<&'static str> {
    SHAPES.iter().find(|&&shape| shape == name).copied().ok_or_else(|| format!("Unknown element name:{name}"))
}

pub fn explosion(shape: &str, colors: Vec<i32>, fade_colors: Vec<i32>, has_trail: bool, has_twinkle: bool) -> Nbt {
    Nbt::Compound(vec![
        ("shape".into(), Nbt::String(shape.into())),
        ("colors".into(), Nbt::List(colors.into_iter().map(Nbt::Int).collect())),
        ("fade_colors".into(), Nbt::List(fade_colors.into_iter().map(Nbt::Int).collect())),
        ("has_trail".into(), Nbt::Byte(has_trail as i8)),
        ("has_twinkle".into(), Nbt::Byte(has_twinkle as i8)),
    ])
}

impl FireworkStarRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkStarRecipe> {
        Ok(FireworkStarRecipe {
            shapes: codec::decode_field(tag, "shapes", |shapes| codec::unbounded_map(shapes, decode_shape, Ingredient::decode))?,
            trail: codec::decode_field(tag, "trail", Ingredient::decode)?,
            twinkle: codec::decode_field(tag, "twinkle", Ingredient::decode)?,
            fuel: codec::decode_field(tag, "fuel", Ingredient::decode)?,
            dye: codec::decode_field(tag, "dye", Ingredient::decode)?,
            result: codec::decode_field(tag, "result", ItemStackTemplate::decode)?,
        })
    }

    fn find_shape(&self, item_stack: &ItemStack) -> Option<&'static str> {
        self.shapes.iter().find(|(_, ingredient)| ingredient.test(item_stack)).map(|&(shape, _)| shape)
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() < 2 {
            return false;
        }
        let (mut has_fuel, mut has_dye, mut has_shape, mut has_trail, mut has_twinkle) = (false, false, false, false, false);
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.twinkle.test(item_stack) {
                if has_twinkle {
                    return false;
                }
                has_twinkle = true;
            } else if self.trail.test(item_stack) {
                if has_trail {
                    return false;
                }
                has_trail = true;
            } else if self.fuel.test(item_stack) {
                if has_fuel {
                    return false;
                }
                has_fuel = true;
            } else if self.dye.test(item_stack) && item_stack.has(dc::DYE) {
                has_dye = true;
            } else {
                if self.find_shape(item_stack).is_none() {
                    return false;
                }
                if has_shape {
                    return false;
                }
                has_shape = true;
            }
        }
        has_fuel && has_dye
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut shape = "small_ball";
        let mut has_twinkle = false;
        let mut has_trail = false;
        let mut colors = Vec::new();
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if let Some(maybe_shape) = self.find_shape(item_stack) {
                shape = maybe_shape;
            } else if self.twinkle.test(item_stack) {
                has_twinkle = true;
            } else if self.trail.test(item_stack) {
                has_trail = true;
            } else if self.dye.test(item_stack) {
                colors.push(get_dye(item_stack).get_firework_color());
            }
        }
        let mut star = self.result.create().unwrap_or_else(ItemStack::empty);
        star.set_value(dc::FIREWORK_EXPLOSION, explosion(shape, colors, Vec::new(), has_trail, has_twinkle));
        star
    }
}
