use crisp_protocol::nbt::Nbt;

use super::list_operation::ListOperation;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetWritableBookPagesFunction {
    pages: Vec<Nbt>,
    page_operation: ListOperation,
}

impl SetWritableBookPagesFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetWritableBookPagesFunction> {
        let pages = codec::decode_field(tag, "pages", |t| {
            let content = dc::WRITABLE_BOOK_CONTENT.decode(&Nbt::Compound(vec![("pages".into(), t.clone())]))?;
            Ok(content.get("pages").map_or(Vec::new(), |p| p.as_list().to_vec()))
        })?;
        Ok(SetWritableBookPagesFunction { pages, page_operation: ListOperation::decode(tag, 100)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let original = item_stack.get(dc::WRITABLE_BOOK_CONTENT).and_then(|c| c.get("pages")).map_or(Vec::new(), |p| p.as_list().to_vec());
        let new_pages = self.page_operation.apply(&original, &self.pages, 100);
        let value = Nbt::Compound(vec![("pages".into(), Nbt::List(new_pages))]);
        let value = dc::WRITABLE_BOOK_CONTENT.decode(&value).unwrap_or(value);
        item_stack.set(dc::WRITABLE_BOOK_CONTENT, Some(value));
        item_stack
    }
}
