use crisp_protocol::nbt::Nbt;

use super::set_written_book_pages_function::put;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

const SHAPES: &[(&str, &str)] = &[("small_ball", "small_ball"), ("large_ball", "large_ball"), ("star", "star"), ("creeper", "creeper"), ("burst", "burst")];

pub struct SetFireworkExplosionFunction {
    shape: Option<&'static str>,
    colors: Option<Nbt>,
    fade_colors: Option<Nbt>,
    trail: Option<bool>,
    twinkle: Option<bool>,
}

fn color_list(tag: &Nbt) -> DataResult<Nbt> {
    Ok(Nbt::IntArray(codec::list_of(tag, codec::int)?))
}

impl SetFireworkExplosionFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetFireworkExplosionFunction> {
        Ok(SetFireworkExplosionFunction {
            shape: codec::decode_optional(tag, "shape", |t| codec::string_enum(t, SHAPES))?,
            colors: codec::decode_optional(tag, "colors", color_list)?,
            fade_colors: codec::decode_optional(tag, "fade_colors", color_list)?,
            trail: codec::decode_optional(tag, "trail", codec::bool_value)?,
            twinkle: codec::decode_optional(tag, "twinkle", codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let mut explosion = match item_stack.get(dc::FIREWORK_EXPLOSION) {
            Some(Nbt::Compound(fields)) => fields.clone(),
            _ => vec![("shape".into(), Nbt::String("small_ball".into()))],
        };
        if let Some(shape) = self.shape {
            put(&mut explosion, "shape", Nbt::String(shape.to_string()));
        }
        if let Some(colors) = &self.colors {
            put(&mut explosion, "colors", colors.clone());
        }
        if let Some(fade_colors) = &self.fade_colors {
            put(&mut explosion, "fade_colors", fade_colors.clone());
        }
        if let Some(trail) = self.trail {
            put(&mut explosion, "has_trail", Nbt::Byte(trail as i8));
        }
        if let Some(twinkle) = self.twinkle {
            put(&mut explosion, "has_twinkle", Nbt::Byte(twinkle as i8));
        }
        let value = Nbt::Compound(explosion);
        let value = dc::FIREWORK_EXPLOSION.decode(&value).unwrap_or(value);
        item_stack.set(dc::FIREWORK_EXPLOSION, Some(value));
        item_stack
    }
}
