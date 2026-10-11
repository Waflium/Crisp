use crisp_protocol::nbt::Nbt;

use super::any_value::AnyValue;
use super::attribute_modifiers_predicate::AttributeModifiersPredicate;
use super::bundle_predicate::BundlePredicate;
use super::container_predicate::ContainerPredicate;
use super::custom_data_predicate::CustomDataPredicate;
use super::damage_predicate::DamagePredicate;
use super::enchantments_predicate::EnchantmentsPredicate;
use super::firework_explosion_predicate::FireworkExplosionPredicate;
use super::fireworks_predicate::FireworksPredicate;
use super::jukebox_playable_predicate::JukeboxPlayablePredicate;
use super::potions_predicate::PotionsPredicate;
use super::trim_predicate::TrimPredicate;
use super::villager_type_predicate::VillagerTypePredicate;
use super::writable_book_predicate::WritableBookPredicate;
use super::written_book_predicate::WrittenBookPredicate;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::{self as dc, DataComponentType};

#[derive(Debug)]
pub enum DataComponentPredicate {
    AnyValue(AnyValue),
    Damage(DamagePredicate),
    Enchantments(EnchantmentsPredicate),
    StoredEnchantments(EnchantmentsPredicate),
    Potions(PotionsPredicate),
    CustomData(CustomDataPredicate),
    Container(ContainerPredicate),
    BundleContents(BundlePredicate),
    FireworkExplosion(FireworkExplosionPredicate),
    Fireworks(FireworksPredicate),
    WritableBook(WritableBookPredicate),
    WrittenBook(WrittenBookPredicate),
    AttributeModifiers(AttributeModifiersPredicate),
    ArmorTrim(TrimPredicate),
    JukeboxPlayable(JukeboxPlayablePredicate),
    VillagerVariant(VillagerTypePredicate),
}

impl DataComponentPredicate {
    pub fn decode_map(tag: &Nbt) -> DataResult<Vec<DataComponentPredicate>> {
        codec::map(tag)?.iter().map(|(key, value)| DataComponentPredicate::decode(key, value)).collect()
    }

    pub fn decode(key: &str, tag: &Nbt) -> DataResult<DataComponentPredicate> {
        let id = crate::resources::identifier::read(key)?;
        Ok(match id.as_str() {
            "minecraft:damage" => DataComponentPredicate::Damage(DamagePredicate::decode(tag)?),
            "minecraft:enchantments" => DataComponentPredicate::Enchantments(EnchantmentsPredicate::decode(dc::ENCHANTMENTS, tag)?),
            "minecraft:stored_enchantments" => DataComponentPredicate::StoredEnchantments(EnchantmentsPredicate::decode(dc::STORED_ENCHANTMENTS, tag)?),
            "minecraft:potion_contents" => DataComponentPredicate::Potions(PotionsPredicate::decode(tag)?),
            "minecraft:custom_data" => DataComponentPredicate::CustomData(CustomDataPredicate::decode(tag)?),
            "minecraft:container" => DataComponentPredicate::Container(ContainerPredicate::decode(tag)?),
            "minecraft:bundle_contents" => DataComponentPredicate::BundleContents(BundlePredicate::decode(tag)?),
            "minecraft:firework_explosion" => DataComponentPredicate::FireworkExplosion(FireworkExplosionPredicate::decode(tag)?),
            "minecraft:fireworks" => DataComponentPredicate::Fireworks(FireworksPredicate::decode(tag)?),
            "minecraft:writable_book_content" => DataComponentPredicate::WritableBook(WritableBookPredicate::decode(tag)?),
            "minecraft:written_book_content" => DataComponentPredicate::WrittenBook(WrittenBookPredicate::decode(tag)?),
            "minecraft:attribute_modifiers" => DataComponentPredicate::AttributeModifiers(AttributeModifiersPredicate::decode(tag)?),
            "minecraft:trim" => DataComponentPredicate::ArmorTrim(TrimPredicate::decode(tag)?),
            "minecraft:jukebox_playable" => DataComponentPredicate::JukeboxPlayable(JukeboxPlayablePredicate::decode(tag)?),
            "minecraft:villager/variant" => DataComponentPredicate::VillagerVariant(VillagerTypePredicate::decode(tag)?),
            other => match DataComponentType::by_name(other) {
                Some(ty) => DataComponentPredicate::AnyValue(AnyValue { ty }),
                None => return Err(format!("Failed to parse either. First: Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_predicate_type]: {other}; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {other}")),
            },
        })
    }

    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        let single = |ty: DataComponentType, matches: &dyn Fn(&Nbt) -> bool| components.get(ty).is_some_and(|value| matches(&value));
        match self {
            DataComponentPredicate::AnyValue(p) => p.matches(components),
            DataComponentPredicate::Damage(p) => p.matches(components),
            DataComponentPredicate::Enchantments(p) | DataComponentPredicate::StoredEnchantments(p) => single(p.component_type(), &|v| p.matches(v)),
            DataComponentPredicate::Potions(p) => single(dc::POTION_CONTENTS, &|v| p.matches(v)),
            DataComponentPredicate::CustomData(p) => p.matches(components),
            DataComponentPredicate::Container(p) => single(dc::CONTAINER, &|v| p.matches(v)),
            DataComponentPredicate::BundleContents(p) => single(dc::BUNDLE_CONTENTS, &|v| p.matches(v)),
            DataComponentPredicate::FireworkExplosion(p) => single(dc::FIREWORK_EXPLOSION, &|v| p.matches(v)),
            DataComponentPredicate::Fireworks(p) => single(dc::FIREWORKS, &|v| p.matches(v)),
            DataComponentPredicate::WritableBook(p) => single(dc::WRITABLE_BOOK_CONTENT, &|v| p.matches(v)),
            DataComponentPredicate::WrittenBook(p) => single(dc::WRITTEN_BOOK_CONTENT, &|v| p.matches(v)),
            DataComponentPredicate::AttributeModifiers(p) => single(dc::ATTRIBUTE_MODIFIERS, &|v| p.matches(v)),
            DataComponentPredicate::ArmorTrim(p) => single(dc::TRIM, &|v| p.matches(v)),
            DataComponentPredicate::JukeboxPlayable(p) => single(dc::JUKEBOX_PLAYABLE, &|v| p.matches(v)),
            DataComponentPredicate::VillagerVariant(p) => single(dc::VILLAGER_VARIANT, &|v| p.matches(v)),
        }
    }
}
