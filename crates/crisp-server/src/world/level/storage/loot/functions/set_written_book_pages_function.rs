use crisp_protocol::nbt::Nbt;

use super::list_operation::ListOperation;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub fn written_book_content(item_stack: &ItemStack) -> Vec<(String, Nbt)> {
    match item_stack.get(dc::WRITTEN_BOOK_CONTENT) {
        Some(Nbt::Compound(fields)) => fields.clone(),
        _ => vec![
            ("title".into(), Nbt::String(String::new())),
            ("author".into(), Nbt::String(String::new())),
            ("generation".into(), Nbt::Int(0)),
            ("pages".into(), Nbt::List(Vec::new())),
            ("resolved".into(), Nbt::Byte(1)),
        ],
    }
}

pub fn put(fields: &mut Vec<(String, Nbt)>, key: &str, value: Nbt) {
    match fields.iter_mut().find(|(k, _)| k == key) {
        Some((_, existing)) => *existing = value,
        None => fields.push((key.to_string(), value)),
    }
}

pub struct SetWrittenBookPagesFunction {
    pages: Vec<Nbt>,
    page_operation: ListOperation,
}

impl SetWrittenBookPagesFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetWrittenBookPagesFunction> {
        let pages = codec::decode_field(tag, "pages", |t| {
            let probe = Nbt::Compound(vec![("title".into(), Nbt::String(String::new())), ("author".into(), Nbt::String(String::new())), ("pages".into(), t.clone())]);
            let content = dc::WRITTEN_BOOK_CONTENT.decode(&probe)?;
            Ok(content.get("pages").map_or(Vec::new(), |p| p.as_list().to_vec()))
        })?;
        Ok(SetWrittenBookPagesFunction { pages, page_operation: ListOperation::decode(tag, i32::MAX)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let mut content = written_book_content(&item_stack);
        let original = content.iter().find(|(k, _)| k == "pages").map_or(Vec::new(), |(_, p)| p.as_list().to_vec());
        let new_pages = self.page_operation.apply(&original, &self.pages, i32::MAX as usize);
        put(&mut content, "pages", Nbt::List(new_pages));
        put(&mut content, "resolved", Nbt::Byte(0));
        let value = Nbt::Compound(content);
        let value = dc::WRITTEN_BOOK_CONTENT.decode(&value).unwrap_or(value);
        item_stack.set(dc::WRITTEN_BOOK_CONTENT, Some(value));
        item_stack
    }
}
