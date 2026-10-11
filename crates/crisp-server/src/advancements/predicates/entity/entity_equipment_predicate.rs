use crisp_data::EquipmentSlot;
use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct EntityEquipmentPredicate {
    head: Option<ItemPredicate>,
    chest: Option<ItemPredicate>,
    legs: Option<ItemPredicate>,
    feet: Option<ItemPredicate>,
    body: Option<ItemPredicate>,
    mainhand: Option<ItemPredicate>,
    offhand: Option<ItemPredicate>,
}

impl EntityEquipmentPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityEquipmentPredicate> {
        codec::map(tag)?;
        let slot = |key| codec::decode_optional(tag, key, ItemPredicate::decode);
        Ok(EntityEquipmentPredicate {
            head: slot("head")?,
            chest: slot("chest")?,
            legs: slot("legs")?,
            feet: slot("feet")?,
            body: slot("body")?,
            mainhand: slot("mainhand")?,
            offhand: slot("offhand")?,
        })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        if !level.is_living(entity) {
            return false;
        }
        [
            (&self.head, EquipmentSlot::Head),
            (&self.chest, EquipmentSlot::Chest),
            (&self.legs, EquipmentSlot::Legs),
            (&self.feet, EquipmentSlot::Feet),
            (&self.body, EquipmentSlot::Body),
            (&self.mainhand, EquipmentSlot::Mainhand),
            (&self.offhand, EquipmentSlot::Offhand),
        ]
        .into_iter()
        .all(|(predicate, slot)| predicate.as_ref().is_none_or(|p| p.test(&level.item_by_slot(entity, slot).unwrap_or_else(ItemStack::empty))))
    }
}
