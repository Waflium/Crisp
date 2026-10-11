use std::borrow::Cow;

use crisp_protocol::nbt::Nbt;

use crate::world::item::ItemStack;
use crate::world::item::component::{DataComponentMap, DataComponentType};

pub trait DataComponentGetter {
    fn get(&self, ty: DataComponentType) -> Option<Cow<'_, Nbt>>;
}

impl DataComponentGetter for ItemStack {
    fn get(&self, ty: DataComponentType) -> Option<Cow<'_, Nbt>> {
        ItemStack::get(self, ty).map(Cow::Borrowed)
    }
}

impl DataComponentGetter for DataComponentMap {
    fn get(&self, ty: DataComponentType) -> Option<Cow<'_, Nbt>> {
        DataComponentMap::get(self, ty).map(Cow::Borrowed)
    }
}

impl DataComponentGetter for [(DataComponentType, Option<Nbt>)] {
    fn get(&self, ty: DataComponentType) -> Option<Cow<'_, Nbt>> {
        self.iter().find(|(t, _)| *t == ty).and_then(|(_, v)| v.as_ref()).map(Cow::Borrowed)
    }
}
