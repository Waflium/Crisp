use crisp_protocol::nbt::Nbt;

use super::list_operation::StandAlone;
use super::set_written_book_pages_function::put;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetFireworksFunction {
    explosions: Option<StandAlone<Nbt>>,
    flight_duration: Option<i32>,
}

impl SetFireworksFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetFireworksFunction> {
        let explosion = |t: &Nbt| dc::FIREWORK_EXPLOSION.decode(t);
        let flight_duration = codec::decode_optional(tag, "flight_duration", |t| {
            let value = codec::int(t)?;
            if !(0..=255).contains(&value) {
                return Err(format!("Value must be within range [0;255]: {value}"));
            }
            Ok(value)
        })?;
        let explosions = match codec::optional_field(tag, "explosions") {
            Some(t) => Some(StandAlone::decode(t, explosion, 256)?),
            None => None,
        };
        Ok(SetFireworksFunction { explosions, flight_duration })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let mut fireworks = match item_stack.get(dc::FIREWORKS) {
            Some(Nbt::Compound(fields)) => fields.clone(),
            _ => Vec::new(),
        };
        if let Some(flight_duration) = self.flight_duration {
            put(&mut fireworks, "flight_duration", Nbt::Int(flight_duration));
        }
        if let Some(explosions) = &self.explosions {
            let old = fireworks.iter().find(|(k, _)| k == "explosions").map_or(Vec::new(), |(_, e)| e.as_list().to_vec());
            put(&mut fireworks, "explosions", Nbt::List(explosions.apply(&old)));
        }
        let value = Nbt::Compound(fireworks);
        let value = dc::FIREWORKS.decode(&value).unwrap_or(value);
        item_stack.set(dc::FIREWORKS, Some(value));
        item_stack
    }
}
