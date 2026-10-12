use crisp_protocol::nbt::Nbt;

use super::apply_bonus_count::ApplyBonusCount;
use super::apply_explosion_decay::ApplyExplosionDecay;
use super::copy_block_state::CopyBlockState;
use super::copy_components_function::CopyComponentsFunction;
use super::copy_custom_data_function::CopyCustomDataFunction;
use super::copy_name_function::CopyNameFunction;
use super::enchant_randomly_function::EnchantRandomlyFunction;
use super::enchant_with_levels_function::EnchantWithLevelsFunction;
use super::enchanted_count_increase_function::EnchantedCountIncreaseFunction;
use super::exploration_map_function::ExplorationMapFunction;
use super::fill_player_head::FillPlayerHead;
use super::filtered_function::FilteredFunction;
use super::limit_count::LimitCount;
use super::modify_container_contents::ModifyContainerContents;
use super::sequence_function::SequenceFunction;
use super::set_attributes_function::SetAttributesFunction;
use super::set_banner_pattern_function::SetBannerPatternFunction;
use super::set_book_cover_function::SetBookCoverFunction;
use super::set_components_function::SetComponentsFunction;
use super::set_container_contents::SetContainerContents;
use super::set_container_loot_table::SetContainerLootTable;
use super::set_custom_data_function::SetCustomDataFunction;
use super::set_custom_model_data_function::SetCustomModelDataFunction;
use super::set_enchantments_function::SetEnchantmentsFunction;
use super::set_firework_explosion_function::SetFireworkExplosionFunction;
use super::set_fireworks_function::SetFireworksFunction;
use super::set_instrument_function::SetInstrumentFunction;
use super::set_item_count_function::SetItemCountFunction;
use super::set_item_damage_function::SetItemDamageFunction;
use super::set_item_function::SetItemFunction;
use super::set_lore_function::SetLoreFunction;
use super::set_name_function::SetNameFunction;
use super::set_ominous_bottle_amplifier_function::SetOminousBottleAmplifierFunction;
use super::set_potion_function::SetPotionFunction;
use super::set_random_dyes_function::SetRandomDyesFunction;
use super::set_random_potion_function::SetRandomPotionFunction;
use super::set_stew_effect_function::SetStewEffectFunction;
use super::set_writable_book_pages_function::SetWritableBookPagesFunction;
use super::set_written_book_pages_function::SetWrittenBookPagesFunction;
use super::smelt_item_function::SmeltItemFunction;
use super::toggle_tooltips::ToggleTooltips;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

pub enum LootItemFunctionType {
    SetCount(SetItemCountFunction),
    SetItem(SetItemFunction),
    EnchantWithLevels(EnchantWithLevelsFunction),
    EnchantRandomly(EnchantRandomlyFunction),
    SetEnchantments(SetEnchantmentsFunction),
    SetCustomData(SetCustomDataFunction),
    SetComponents(SetComponentsFunction),
    FurnaceSmelt(SmeltItemFunction),
    EnchantedCountIncrease(EnchantedCountIncreaseFunction),
    SetDamage(SetItemDamageFunction),
    SetAttributes(SetAttributesFunction),
    SetName(SetNameFunction),
    ExplorationMap(ExplorationMapFunction),
    SetStewEffect(SetStewEffectFunction),
    CopyName(CopyNameFunction),
    SetContents(SetContainerContents),
    ModifyContents(ModifyContainerContents),
    Filtered(FilteredFunction),
    LimitCount(LimitCount),
    ApplyBonus(ApplyBonusCount),
    SetLootTable(SetContainerLootTable),
    ExplosionDecay(ApplyExplosionDecay),
    SetLore(SetLoreFunction),
    FillPlayerHead(FillPlayerHead),
    CopyCustomData(CopyCustomDataFunction),
    CopyState(CopyBlockState),
    SetBannerPattern(SetBannerPatternFunction),
    SetPotion(SetPotionFunction),
    SetRandomDyes(SetRandomDyesFunction),
    SetRandomPotion(SetRandomPotionFunction),
    SetInstrument(SetInstrumentFunction),
    Sequence(SequenceFunction),
    CopyComponents(CopyComponentsFunction),
    SetFireworks(SetFireworksFunction),
    SetFireworkExplosion(SetFireworkExplosionFunction),
    SetBookCover(SetBookCoverFunction),
    SetWrittenBookPages(SetWrittenBookPagesFunction),
    SetWritableBookPages(SetWritableBookPagesFunction),
    ToggleTooltips(ToggleTooltips),
    SetOminousBottleAmplifier(SetOminousBottleAmplifierFunction),
    SetCustomModelData(SetCustomModelDataFunction),
    Discard,
}

pub struct LootItemFunction {
    condition: Option<Holder<LootItemCondition>>,
    function: LootItemFunctionType,
}

impl RegistryElement for LootItemFunction {
    const REGISTRY: &'static str = "minecraft:item_modifier";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static LootItemFunction> {
        reloadable_server_registries::get().item_modifiers.get(key)
    }
}

impl LootItemFunction {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<LootItemFunction>> {
        Holder::decode(tag, LootItemFunction::decode_direct)
    }

    pub fn decode_direct(tag: &Nbt) -> DataResult<LootItemFunction> {
        match codec::either_or_partial(tag, LootItemFunction::decode_typed, SequenceFunction::decode_inline)? {
            Ok(function) => Ok(function),
            Err(sequence) => Ok(LootItemFunction { condition: None, function: LootItemFunctionType::Sequence(sequence) }),
        }
    }

    pub fn decode_list(tag: &Nbt) -> DataResult<Vec<Holder<LootItemFunction>>> {
        LootItemFunction::decode_holder_set(tag).map_err(|error| error.message)
    }

    pub fn decode_holder_set(tag: &Nbt) -> Result<Vec<Holder<LootItemFunction>>, codec::DecodeError> {
        codec::holder_set(tag, Self::REGISTRY, |t| Holder::decode(t, LootItemFunction::decode_typed))
    }

    pub fn decode_typed(tag: &Nbt) -> DataResult<LootItemFunction> {
        codec::map(tag)?;
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let function = match ty.as_str() {
            "minecraft:set_count" => LootItemFunctionType::SetCount(SetItemCountFunction::decode(tag)?),
            "minecraft:set_item" => LootItemFunctionType::SetItem(SetItemFunction::decode(tag)?),
            "minecraft:enchant_with_levels" => LootItemFunctionType::EnchantWithLevels(EnchantWithLevelsFunction::decode(tag)?),
            "minecraft:enchant_randomly" => LootItemFunctionType::EnchantRandomly(EnchantRandomlyFunction::decode(tag)?),
            "minecraft:set_enchantments" => LootItemFunctionType::SetEnchantments(SetEnchantmentsFunction::decode(tag)?),
            "minecraft:set_custom_data" => LootItemFunctionType::SetCustomData(SetCustomDataFunction::decode(tag)?),
            "minecraft:set_components" => LootItemFunctionType::SetComponents(SetComponentsFunction::decode(tag)?),
            "minecraft:furnace_smelt" => LootItemFunctionType::FurnaceSmelt(SmeltItemFunction::decode(tag)?),
            "minecraft:enchanted_count_increase" => LootItemFunctionType::EnchantedCountIncrease(EnchantedCountIncreaseFunction::decode(tag)?),
            "minecraft:set_damage" => LootItemFunctionType::SetDamage(SetItemDamageFunction::decode(tag)?),
            "minecraft:set_attributes" => LootItemFunctionType::SetAttributes(SetAttributesFunction::decode(tag)?),
            "minecraft:set_name" => LootItemFunctionType::SetName(SetNameFunction::decode(tag)?),
            "minecraft:exploration_map" => LootItemFunctionType::ExplorationMap(ExplorationMapFunction::decode(tag)?),
            "minecraft:set_stew_effect" => LootItemFunctionType::SetStewEffect(SetStewEffectFunction::decode(tag)?),
            "minecraft:copy_name" => LootItemFunctionType::CopyName(CopyNameFunction::decode(tag)?),
            "minecraft:set_contents" => LootItemFunctionType::SetContents(SetContainerContents::decode(tag)?),
            "minecraft:modify_contents" => LootItemFunctionType::ModifyContents(ModifyContainerContents::decode(tag)?),
            "minecraft:filtered" => LootItemFunctionType::Filtered(FilteredFunction::decode(tag)?),
            "minecraft:limit_count" => LootItemFunctionType::LimitCount(LimitCount::decode(tag)?),
            "minecraft:apply_bonus" => LootItemFunctionType::ApplyBonus(ApplyBonusCount::decode(tag)?),
            "minecraft:set_loot_table" => LootItemFunctionType::SetLootTable(SetContainerLootTable::decode(tag)?),
            "minecraft:explosion_decay" => LootItemFunctionType::ExplosionDecay(ApplyExplosionDecay),
            "minecraft:set_lore" => LootItemFunctionType::SetLore(SetLoreFunction::decode(tag)?),
            "minecraft:fill_player_head" => LootItemFunctionType::FillPlayerHead(FillPlayerHead::decode(tag)?),
            "minecraft:copy_custom_data" => LootItemFunctionType::CopyCustomData(CopyCustomDataFunction::decode(tag)?),
            "minecraft:copy_state" => LootItemFunctionType::CopyState(CopyBlockState::decode(tag)?),
            "minecraft:set_banner_pattern" => LootItemFunctionType::SetBannerPattern(SetBannerPatternFunction::decode(tag)?),
            "minecraft:set_potion" => LootItemFunctionType::SetPotion(SetPotionFunction::decode(tag)?),
            "minecraft:set_random_dyes" => LootItemFunctionType::SetRandomDyes(SetRandomDyesFunction::decode(tag)?),
            "minecraft:set_random_potion" => LootItemFunctionType::SetRandomPotion(SetRandomPotionFunction::decode(tag)?),
            "minecraft:set_instrument" => LootItemFunctionType::SetInstrument(SetInstrumentFunction::decode(tag)?),
            "minecraft:sequence" => LootItemFunctionType::Sequence(SequenceFunction::decode(tag)?),
            "minecraft:copy_components" => LootItemFunctionType::CopyComponents(CopyComponentsFunction::decode(tag)?),
            "minecraft:set_fireworks" => LootItemFunctionType::SetFireworks(SetFireworksFunction::decode(tag)?),
            "minecraft:set_firework_explosion" => LootItemFunctionType::SetFireworkExplosion(SetFireworkExplosionFunction::decode(tag)?),
            "minecraft:set_book_cover" => LootItemFunctionType::SetBookCover(SetBookCoverFunction::decode(tag)?),
            "minecraft:set_written_book_pages" => LootItemFunctionType::SetWrittenBookPages(SetWrittenBookPagesFunction::decode(tag)?),
            "minecraft:set_writable_book_pages" => LootItemFunctionType::SetWritableBookPages(SetWritableBookPagesFunction::decode(tag)?),
            "minecraft:toggle_tooltips" => LootItemFunctionType::ToggleTooltips(ToggleTooltips::decode(tag)?),
            "minecraft:set_ominous_bottle_amplifier" => LootItemFunctionType::SetOminousBottleAmplifier(SetOminousBottleAmplifierFunction::decode(tag)?),
            "minecraft:set_custom_model_data" => LootItemFunctionType::SetCustomModelData(SetCustomModelDataFunction::decode(tag)?),
            "minecraft:discard" => LootItemFunctionType::Discard,
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_function_type]: {other}")),
        };
        Ok(LootItemFunction { condition: codec::decode_optional(tag, "condition", LootItemCondition::decode_holder)?, function })
    }

    pub fn apply(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if let Some(condition) = &self.condition
            && !condition.value().test(context)
        {
            return item_stack;
        }
        self.run(item_stack, context)
    }

    fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        match &self.function {
            LootItemFunctionType::SetCount(f) => f.run(item_stack, context),
            LootItemFunctionType::SetItem(f) => f.run(item_stack, context),
            LootItemFunctionType::EnchantWithLevels(f) => f.run(item_stack, context),
            LootItemFunctionType::EnchantRandomly(f) => f.run(item_stack, context),
            LootItemFunctionType::SetEnchantments(f) => f.run(item_stack, context),
            LootItemFunctionType::SetCustomData(f) => f.run(item_stack, context),
            LootItemFunctionType::SetComponents(f) => f.run(item_stack, context),
            LootItemFunctionType::FurnaceSmelt(f) => f.run(item_stack, context),
            LootItemFunctionType::EnchantedCountIncrease(f) => f.run(item_stack, context),
            LootItemFunctionType::SetDamage(f) => f.run(item_stack, context),
            LootItemFunctionType::SetAttributes(f) => f.run(item_stack, context),
            LootItemFunctionType::SetName(f) => f.run(item_stack, context),
            LootItemFunctionType::ExplorationMap(f) => f.run(item_stack, context),
            LootItemFunctionType::SetStewEffect(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyName(f) => f.run(item_stack, context),
            LootItemFunctionType::SetContents(f) => f.run(item_stack, context),
            LootItemFunctionType::ModifyContents(f) => f.run(item_stack, context),
            LootItemFunctionType::Filtered(f) => f.run(item_stack, context),
            LootItemFunctionType::LimitCount(f) => f.run(item_stack, context),
            LootItemFunctionType::ApplyBonus(f) => f.run(item_stack, context),
            LootItemFunctionType::SetLootTable(f) => f.run(item_stack, context),
            LootItemFunctionType::ExplosionDecay(f) => f.run(item_stack, context),
            LootItemFunctionType::SetLore(f) => f.run(item_stack, context),
            LootItemFunctionType::FillPlayerHead(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyCustomData(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyState(f) => f.run(item_stack, context),
            LootItemFunctionType::SetBannerPattern(f) => f.run(item_stack, context),
            LootItemFunctionType::SetPotion(f) => f.run(item_stack, context),
            LootItemFunctionType::SetRandomDyes(f) => f.run(item_stack, context),
            LootItemFunctionType::SetRandomPotion(f) => f.run(item_stack, context),
            LootItemFunctionType::SetInstrument(f) => f.run(item_stack, context),
            LootItemFunctionType::Sequence(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyComponents(f) => f.run(item_stack, context),
            LootItemFunctionType::SetFireworks(f) => f.run(item_stack, context),
            LootItemFunctionType::SetFireworkExplosion(f) => f.run(item_stack, context),
            LootItemFunctionType::SetBookCover(f) => f.run(item_stack, context),
            LootItemFunctionType::SetWrittenBookPages(f) => f.run(item_stack, context),
            LootItemFunctionType::SetWritableBookPages(f) => f.run(item_stack, context),
            LootItemFunctionType::ToggleTooltips(f) => f.run(item_stack, context),
            LootItemFunctionType::SetOminousBottleAmplifier(f) => f.run(item_stack, context),
            LootItemFunctionType::SetCustomModelData(f) => f.run(item_stack, context),
            LootItemFunctionType::Discard => ItemStack::empty(),
        }
    }
}

pub fn decorate<'a>(
    function: &Option<Holder<LootItemFunction>>,
    item_stack: ItemStack,
    context: &mut LootContext<'a>,
    output: &mut dyn FnMut(&mut LootContext<'a>, ItemStack),
) {
    match function {
        Some(function) => {
            let item_stack = function.value().apply(item_stack, context);
            output(context, item_stack);
        }
        None => output(context, item_stack),
    }
}
