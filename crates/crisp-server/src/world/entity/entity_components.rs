use std::borrow::Cow;

use crisp_protocol::nbt::Nbt;

use super::EntityId;
use super::animal::chicken;
use super::animal::cow::{cow, mushroom_cow};
use super::variant::variant_utils;
use super::data::DataValue;
use super::mob::MobKind;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::server::level::ServerLevel;
use crate::world::item::component::{self as dc, DataComponentType};

pub struct EntityComponents<'a> {
    pub level: &'a ServerLevel,
    pub id: EntityId,
}

impl DataComponentGetter for EntityComponents<'_> {
    fn get(&self, ty: DataComponentType) -> Option<Cow<'_, Nbt>> {
        if let Some(mob) = self.level.mob(self.id) {
            match &mob.kind {
                MobKind::Chicken(_) if ty == dc::CHICKEN_VARIANT => {
                    if let DataValue::ChickenVariant(variant) = mob.entity.data.get(chicken::DATA_VARIANT) {
                        return Some(Cow::Owned(Nbt::String(chicken::variant_name(*variant).to_string())));
                    }
                }
                MobKind::Chicken(_) if ty == dc::CHICKEN_SOUND_VARIANT => {
                    if let DataValue::ChickenSoundVariant(variant) = mob.entity.data.get(chicken::DATA_SOUND_VARIANT) {
                        return Some(Cow::Owned(Nbt::String(chicken::sound_variant_name(*variant).to_string())));
                    }
                }
                MobKind::Cow(_) if ty == dc::COW_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(cow::VARIANT_REGISTRY, cow::get_variant(&mob.entity)).to_string())));
                }
                MobKind::Cow(_) if ty == dc::COW_SOUND_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(variant_utils::variant_name(cow::SOUND_VARIANT_REGISTRY, cow::get_sound_variant(&mob.entity)).to_string())));
                }
                MobKind::MushroomCow(_) if ty == dc::MOOSHROOM_VARIANT => {
                    return Some(Cow::Owned(Nbt::String(mushroom_cow::get_variant(&mob.entity).get_serialized_name().to_string())));
                }
                _ => {}
            }
        }
        let entity = self.level.entity(self.id);
        if ty == dc::CUSTOM_NAME {
            return match entity.data.get(super::data::ids::CUSTOM_NAME) {
                DataValue::OptionalComponent(Some(name)) => Some(Cow::Borrowed(name)),
                _ => None,
            };
        }
        if ty == dc::CUSTOM_DATA {
            return entity.custom_data.as_ref().map(Cow::Borrowed);
        }
        None
    }
}
