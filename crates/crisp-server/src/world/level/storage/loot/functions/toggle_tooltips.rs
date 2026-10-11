use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct ToggleTooltips {
    values: Vec<(DataComponentType, bool)>,
}

impl ToggleTooltips {
    pub fn decode(tag: &Nbt) -> DataResult<ToggleTooltips> {
        let values = codec::decode_field(tag, "toggles", |t| {
            codec::unbounded_map(
                t,
                |k| {
                    let id = codec::identifier(&Nbt::String(k.to_string()))?;
                    DataComponentType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"))
                },
                codec::bool_value,
            )
        })?;
        Ok(ToggleTooltips { values })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let display = item_stack.get(dc::TOOLTIP_DISPLAY);
        let hide_tooltip = display.and_then(|d| d.get("hide_tooltip")).cloned();
        let mut hidden: Vec<String> =
            display.and_then(|d| d.get("hidden_components")).map_or(Vec::new(), |h| h.as_list().iter().filter_map(Nbt::as_str).map(str::to_string).collect());
        for &(ty, shown) in &self.values {
            let name = ty.name();
            let is_hidden = hidden.iter().any(|h| h == name);
            if is_hidden == !shown {
                continue;
            }
            if !shown {
                hidden.push(name.to_string());
            } else {
                hidden.retain(|h| h != name);
            }
        }
        let mut fields = Vec::new();
        if let Some(hide_tooltip) = hide_tooltip {
            fields.push(("hide_tooltip".to_string(), hide_tooltip));
        }
        fields.push(("hidden_components".to_string(), Nbt::List(hidden.into_iter().map(Nbt::String).collect())));
        let value = Nbt::Compound(fields);
        let value = dc::TOOLTIP_DISPLAY.decode(&value).unwrap_or(value);
        item_stack.set(dc::TOOLTIP_DISPLAY, Some(value));
        item_stack
    }
}
