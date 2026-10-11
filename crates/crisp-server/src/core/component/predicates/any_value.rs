use crate::core::component::data_component_getter::DataComponentGetter;
use crate::world::item::component::DataComponentType;

#[derive(Debug)]
pub struct AnyValue {
    pub ty: DataComponentType,
}

impl AnyValue {
    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        components.get(self.ty).is_some()
    }
}
