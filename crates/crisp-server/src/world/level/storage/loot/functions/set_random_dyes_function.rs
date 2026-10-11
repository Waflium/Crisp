use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, codec as component_codec};
use crate::world::item::dye_color::DyeColor;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub struct SetRandomDyesFunction {
    number_of_dyes: Holder<ContextIntProvider>,
}

fn apply_dyes(current_dye: Option<i32>, dyes: &[DyeColor]) -> i32 {
    let (mut red_total, mut green_total, mut blue_total, mut intensity_total, mut color_count) = (0, 0, 0, 0, 0);
    let mut add = |color: i32| {
        let (red, green, blue) = ((color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF);
        intensity_total += red.max(green.max(blue));
        red_total += red;
        green_total += green;
        blue_total += blue;
        color_count += 1;
    };
    if let Some(current_dye) = current_dye {
        add(current_dye);
    }
    for dye in dyes {
        add(dye.texture_diffuse_color());
    }
    let mut red = red_total / color_count;
    let mut green = green_total / color_count;
    let mut blue = blue_total / color_count;
    let average_intensity = intensity_total as f32 / color_count as f32;
    let result_intensity = red.max(green.max(blue)) as f32;
    red = (red as f32 * average_intensity / result_intensity) as i32;
    green = (green as f32 * average_intensity / result_intensity) as i32;
    blue = (blue as f32 * average_intensity / result_intensity) as i32;
    (red << 16) | (green << 8) | blue
}

impl SetRandomDyesFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetRandomDyesFunction> {
        Ok(SetRandomDyesFunction { number_of_dyes: codec::decode_field(tag, "number_of_dyes", ContextIntProvider::decode_holder)? })
    }

    pub fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let rolls = self.number_of_dyes.value().get_int(context);
        if rolls <= 0 {
            return item_stack;
        }
        let dyes: Vec<DyeColor> = (0..rolls).map(|_| DyeColor::VALUES[context.random().next_int_bounded(DyeColor::VALUES.len() as i32) as usize]).collect();
        let current_dye = item_stack.get(dc::DYED_COLOR).map(component_codec::as_i32);
        let mut result = item_stack.copy_with_count(1);
        result.set(dc::DYED_COLOR, Some(Nbt::Int(apply_dyes(current_dye, &dyes))));
        result
    }
}
