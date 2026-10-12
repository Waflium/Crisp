use crisp_protocol::nbt::Nbt;

use super::all_of_condition::AllOfCondition;
use super::any_of_condition::AnyOfCondition;
use super::bonus_level_table_condition::BonusLevelTableCondition;
use super::damage_source_condition::DamageSourceCondition;
use super::enchantment_active_check::EnchantmentActiveCheck;
use super::entity_has_score_condition::EntityHasScoreCondition;
use super::environment_attribute_check::EnvironmentAttributeCheck;
use super::explosion_condition::ExplosionCondition;
use super::float_value_check::FloatValueCheck;
use super::int_value_check::IntValueCheck;
use super::inverted_loot_item_condition::InvertedLootItemCondition;
use super::location_check::LocationCheck;
use super::loot_item_entity_property_condition::LootItemEntityPropertyCondition;
use super::loot_item_killed_by_player_condition::LootItemKilledByPlayerCondition;
use super::loot_item_random_chance_condition::LootItemRandomChanceCondition;
use super::loot_item_random_chance_with_enchanted_bonus_condition::LootItemRandomChanceWithEnchantedBonusCondition;
use super::match_block::MatchBlock;
use super::match_tool::MatchTool;
use super::time_check::TimeCheck;
use super::weather_check::WeatherCheck;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub enum LootItemCondition {
    Inverted(InvertedLootItemCondition),
    AnyOf(AnyOfCondition),
    AllOf(AllOfCondition),
    RandomChance(LootItemRandomChanceCondition),
    RandomChanceWithEnchantedBonus(LootItemRandomChanceWithEnchantedBonusCondition),
    EntityProperties(LootItemEntityPropertyCondition),
    KilledByPlayer(LootItemKilledByPlayerCondition),
    EntityScores(EntityHasScoreCondition),
    MatchBlock(MatchBlock),
    MatchTool(MatchTool),
    TableBonus(BonusLevelTableCondition),
    SurvivesExplosion(ExplosionCondition),
    DamageSourceProperties(DamageSourceCondition),
    LocationCheck(LocationCheck),
    WeatherCheck(WeatherCheck),
    TimeCheck(TimeCheck),
    IntValueCheck(IntValueCheck),
    FloatValueCheck(FloatValueCheck),
    EnchantmentActiveCheck(EnchantmentActiveCheck),
    EnvironmentAttributeCheck(EnvironmentAttributeCheck),
}

impl RegistryElement for LootItemCondition {
    const REGISTRY: &'static str = "minecraft:predicate";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static LootItemCondition> {
        reloadable_server_registries::get().predicates.get(key)
    }
}

impl LootItemCondition {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<LootItemCondition>> {
        Holder::decode(tag, LootItemCondition::decode)
    }

    pub fn decode_holder_list(tag: &Nbt) -> DataResult<Vec<Holder<LootItemCondition>>> {
        crate::world::level::storage::loot::providers::number::decode_holder_list(tag, LootItemCondition::decode_holder)
    }

    pub fn decode(tag: &Nbt) -> DataResult<LootItemCondition> {
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        Ok(match ty.as_str() {
            "minecraft:inverted" => LootItemCondition::Inverted(InvertedLootItemCondition::decode(tag)?),
            "minecraft:any_of" => LootItemCondition::AnyOf(AnyOfCondition::decode(tag)?),
            "minecraft:all_of" => LootItemCondition::AllOf(AllOfCondition::decode(tag)?),
            "minecraft:random_chance" => LootItemCondition::RandomChance(LootItemRandomChanceCondition::decode(tag)?),
            "minecraft:random_chance_with_enchanted_bonus" => {
                LootItemCondition::RandomChanceWithEnchantedBonus(LootItemRandomChanceWithEnchantedBonusCondition::decode(tag)?)
            }
            "minecraft:entity_properties" => LootItemCondition::EntityProperties(LootItemEntityPropertyCondition::decode(tag)?),
            "minecraft:killed_by_player" => LootItemCondition::KilledByPlayer(LootItemKilledByPlayerCondition),
            "minecraft:entity_scores" => LootItemCondition::EntityScores(EntityHasScoreCondition::decode(tag)?),
            "minecraft:match_block" => LootItemCondition::MatchBlock(MatchBlock::decode(tag)?),
            "minecraft:match_tool" => LootItemCondition::MatchTool(MatchTool::decode(tag)?),
            "minecraft:table_bonus" => LootItemCondition::TableBonus(BonusLevelTableCondition::decode(tag)?),
            "minecraft:survives_explosion" => LootItemCondition::SurvivesExplosion(ExplosionCondition),
            "minecraft:damage_source_properties" => LootItemCondition::DamageSourceProperties(DamageSourceCondition::decode(tag)?),
            "minecraft:location_check" => LootItemCondition::LocationCheck(LocationCheck::decode(tag)?),
            "minecraft:weather_check" => LootItemCondition::WeatherCheck(WeatherCheck::decode(tag)?),
            "minecraft:time_check" => LootItemCondition::TimeCheck(TimeCheck::decode(tag)?),
            "minecraft:int_value_check" => LootItemCondition::IntValueCheck(IntValueCheck::decode(tag)?),
            "minecraft:float_value_check" => LootItemCondition::FloatValueCheck(FloatValueCheck::decode(tag)?),
            "minecraft:enchantment_active_check" => LootItemCondition::EnchantmentActiveCheck(EnchantmentActiveCheck::decode(tag)?),
            "minecraft:environment_attribute_check" => LootItemCondition::EnvironmentAttributeCheck(EnvironmentAttributeCheck::decode(tag)?),
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_condition_type]: {other}")),
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        match self {
            LootItemCondition::Inverted(c) => c.test(context),
            LootItemCondition::AnyOf(c) => c.test(context),
            LootItemCondition::AllOf(c) => c.test(context),
            LootItemCondition::RandomChance(c) => c.test(context),
            LootItemCondition::RandomChanceWithEnchantedBonus(c) => c.test(context),
            LootItemCondition::EntityProperties(c) => c.test(context),
            LootItemCondition::KilledByPlayer(c) => c.test(context),
            LootItemCondition::EntityScores(c) => c.test(context),
            LootItemCondition::MatchBlock(c) => c.test(context),
            LootItemCondition::MatchTool(c) => c.test(context),
            LootItemCondition::TableBonus(c) => c.test(context),
            LootItemCondition::SurvivesExplosion(c) => c.test(context),
            LootItemCondition::DamageSourceProperties(c) => c.test(context),
            LootItemCondition::LocationCheck(c) => c.test(context),
            LootItemCondition::WeatherCheck(c) => c.test(context),
            LootItemCondition::TimeCheck(c) => c.test(context),
            LootItemCondition::IntValueCheck(c) => c.test(context),
            LootItemCondition::FloatValueCheck(c) => c.test(context),
            LootItemCondition::EnchantmentActiveCheck(c) => c.test(context),
            LootItemCondition::EnvironmentAttributeCheck(c) => c.test(context),
        }
    }
}
