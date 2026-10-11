use crisp_protocol::nbt::Nbt;

use super::list_operation::StandAlone;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, codec as component_codec};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::providers::number::ints::{self, ContextIntProvider};

pub struct SetCustomModelDataFunction {
    floats: Option<StandAlone<Holder<ContextFloatProvider>>>,
    flags: Option<StandAlone<bool>>,
    strings: Option<StandAlone<String>>,
    colors: Option<StandAlone<Holder<ContextIntProvider>>>,
}

fn color_provider(tag: &Nbt) -> DataResult<Holder<ContextIntProvider>> {
    match ContextIntProvider::decode_holder(tag) {
        Ok(provider) => Ok(provider),
        Err(error) => match tag.as_list() {
            [r, g, b] => {
                let channel = |c: &Nbt| codec::float(c).map(|v| (v * 255.0) as i32 & 0xFF);
                Ok(ints::exactly((channel(r)? << 16) | (channel(g)? << 8) | channel(b)?))
            }
            _ => Err(error),
        },
    }
}

impl SetCustomModelDataFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetCustomModelDataFunction> {
        let stand_alone = |key: &str| codec::optional_field(tag, key);
        Ok(SetCustomModelDataFunction {
            floats: stand_alone("floats").map(|t| StandAlone::decode(t, ContextFloatProvider::decode_holder, i32::MAX)).transpose()?,
            flags: stand_alone("flags").map(|t| StandAlone::decode(t, codec::bool_value, i32::MAX)).transpose()?,
            strings: stand_alone("strings").map(|t| StandAlone::decode(t, |s| Ok(codec::string(s)?.to_string()), i32::MAX)).transpose()?,
            colors: stand_alone("colors").map(|t| StandAlone::decode(t, color_provider, i32::MAX)).transpose()?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let component = item_stack.get(dc::CUSTOM_MODEL_DATA).cloned().unwrap_or(Nbt::Compound(Vec::new()));
        let list = |key: &str| component.get(key).map_or(Vec::new(), |v| v.as_list().to_vec());
        let floats = match &self.floats {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|provider| Nbt::Float(provider.value().get_float(context))).collect();
                operation.operation.apply(&list("floats"), &replacement, i32::MAX as usize)
            }
            None => list("floats"),
        };
        let flags = match &self.flags {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|&flag| Nbt::Byte(flag as i8)).collect();
                operation.operation.apply(&list("flags"), &replacement, i32::MAX as usize)
            }
            None => list("flags"),
        };
        let strings = match &self.strings {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|s| Nbt::String(s.clone())).collect();
                operation.operation.apply(&list("strings"), &replacement, i32::MAX as usize)
            }
            None => list("strings"),
        };
        let colors = match &self.colors {
            Some(operation) => {
                let replacement: Vec<Nbt> = operation.value.iter().map(|provider| Nbt::Int(provider.value().get_int(context))).collect();
                let current: Vec<Nbt> = list("colors").iter().map(|c| Nbt::Int(component_codec::as_i32(c))).collect();
                operation.operation.apply(&current, &replacement, i32::MAX as usize)
            }
            None => list("colors"),
        };
        let value = Nbt::Compound(vec![
            ("floats".into(), Nbt::List(floats)),
            ("flags".into(), Nbt::List(flags)),
            ("strings".into(), Nbt::List(strings)),
            ("colors".into(), Nbt::List(colors)),
        ]);
        let value = dc::CUSTOM_MODEL_DATA.decode(&value).unwrap_or(value);
        item_stack.set(dc::CUSTOM_MODEL_DATA, Some(value));
        item_stack
    }
}
