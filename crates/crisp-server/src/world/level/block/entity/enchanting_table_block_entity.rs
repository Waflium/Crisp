use crisp_protocol::nbt::Nbt;

use crate::world::item::component::{self as dc, DataComponentType};

#[derive(Clone, Default)]
pub struct EnchantingTableBlockEntity {
    name: Option<Nbt>,
}

impl EnchantingTableBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if let Some(name) = &self.name {
            output.push(("CustomName".into(), name.clone()));
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.name = input.get("CustomName").and_then(|name| dc::CUSTOM_NAME.decode(name).ok());
    }

    pub fn set_custom_name(&mut self, name: Option<Nbt>) {
        self.name = name;
    }

    pub fn get_custom_name(&self) -> Option<&Nbt> {
        self.name.as_ref()
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.name = get(dc::CUSTOM_NAME).cloned();
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        components.retain(|(t, _)| *t != dc::CUSTOM_NAME);
        components.push((dc::CUSTOM_NAME, self.name.clone()));
    }
}
