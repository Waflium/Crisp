use crisp_data::Item;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::component::values::ItemStackTemplate;

#[derive(Clone, Debug)]
pub struct TransmuteResult {
    pub item: Option<String>,
    pub count: i32,
    pub components: Nbt,
}

impl TransmuteResult {
    pub fn decode(tag: &Nbt) -> DataResult<TransmuteResult> {
        if let Nbt::String(_) = tag {
            let template = ItemStackTemplate::decode(tag)?;
            return Ok(TransmuteResult { item: Some(template.item), count: 1, components: Nbt::Compound(Vec::new()) });
        }
        let item = codec::decode_optional(tag, "id", |t| {
            let id = codec::identifier(t)?;
            Item::by_name(&id).map(|_| id.clone()).ok_or_else(|| format!("Unknown item: {id}"))
        })?;
        let count = codec::decode_or(tag, "count", 1, codec::int)?;
        if !(1..=99).contains(&count) {
            return Err(format!("Value {count} outside of range [1:99]"));
        }
        Ok(TransmuteResult { item, count, components: codec::optional_field(tag, "components").cloned().unwrap_or(Nbt::Compound(Vec::new())) })
    }

    pub fn resolve(&self, input_item: Item) -> ItemStackTemplate {
        self.resolve_count(input_item, self.count)
    }

    pub fn resolve_count(&self, input_item: Item, count: i32) -> ItemStackTemplate {
        ItemStackTemplate { item: self.item.clone().unwrap_or_else(|| input_item.name().to_string()), count, components: self.components.clone() }
    }
}
