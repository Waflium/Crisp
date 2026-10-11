use crisp_protocol::nbt::Nbt;

use super::enchantment_attribute_effect::EnchantmentAttributeEffect;
use super::enchantment_entity_effect::EnchantmentEntityEffect;
use crate::util::codec::{self, DataResult};

#[allow(dead_code)]
#[derive(Debug)]
pub enum EnchantmentLocationBasedEffect {
    AllOf(Vec<EnchantmentLocationBasedEffect>),
    Attribute(EnchantmentAttributeEffect),
    Entity(EnchantmentEntityEffect),
}

impl EnchantmentLocationBasedEffect {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentLocationBasedEffect> {
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        Ok(match ty.as_str() {
            "minecraft:all_of" => EnchantmentLocationBasedEffect::AllOf(codec::decode_field(tag, "effects", |t| codec::list_of(t, EnchantmentLocationBasedEffect::decode))?),
            "minecraft:attribute" => EnchantmentLocationBasedEffect::Attribute(EnchantmentAttributeEffect::decode(tag)?),
            _ => EnchantmentLocationBasedEffect::Entity(EnchantmentEntityEffect::decode(tag)?),
        })
    }
}
