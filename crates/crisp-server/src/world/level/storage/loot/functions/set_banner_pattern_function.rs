use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetBannerPatternFunction {
    patterns: Nbt,
    append: bool,
}

impl SetBannerPatternFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetBannerPatternFunction> {
        Ok(SetBannerPatternFunction {
            patterns: codec::decode_field(tag, "patterns", |t| dc::BANNER_PATTERNS.decode(t))?,
            append: codec::decode_field(tag, "append", codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        if self.append {
            let mut layers = item_stack.get(dc::BANNER_PATTERNS).map_or(Vec::new(), |p| p.as_list().to_vec());
            layers.extend_from_slice(self.patterns.as_list());
            item_stack.set(dc::BANNER_PATTERNS, Some(Nbt::List(layers)));
        } else {
            item_stack.set(dc::BANNER_PATTERNS, Some(self.patterns.clone()));
        }
        item_stack
    }
}
