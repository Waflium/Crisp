use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;

pub const DEFAULT_DECORATION: &str = "minecraft:mansion";
pub const DEFAULT_ZOOM: i8 = 2;
pub const DEFAULT_SEARCH_RADIUS: i32 = 50;
pub const DEFAULT_SKIP_EXISTING: bool = true;

#[allow(dead_code)]
pub struct ExplorationMapFunction {
    destination: String,
    map_decoration: String,
    zoom: i8,
    search_radius: i32,
    skip_known_structures: bool,
}

impl ExplorationMapFunction {
    pub fn decode(tag: &Nbt) -> DataResult<ExplorationMapFunction> {
        Ok(ExplorationMapFunction {
            destination: codec::decode_field(tag, "destination", |t| Ok(codec::string(t)?.to_string()))?,
            map_decoration: codec::decode_or(tag, "decoration", DEFAULT_DECORATION.to_string(), codec::identifier)?,
            zoom: codec::decode_or(tag, "zoom", DEFAULT_ZOOM, |t| Ok(codec::int(t)? as i8))?,
            search_radius: codec::decode_or(tag, "search_radius", DEFAULT_SEARCH_RADIUS, codec::int)?,
            skip_known_structures: codec::decode_or(tag, "skip_existing_chunks", DEFAULT_SKIP_EXISTING, codec::bool_value)?,
        })
    }

    pub fn run(&self, item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        item_stack
    }
}
