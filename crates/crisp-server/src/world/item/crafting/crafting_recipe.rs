use crisp_protocol::nbt::Nbt;

use super::banner_duplicate_recipe::BannerDuplicateRecipe;
use super::book_cloning_recipe::BookCloningRecipe;
use super::crafting_book_category::CraftingBookCategory;
use super::crafting_input::CraftingInput;
use super::decorated_pot_recipe::DecoratedPotRecipe;
use super::dye_recipe::DyeRecipe;
use super::firework_rocket_recipe::FireworkRocketRecipe;
use super::firework_star_fade_recipe::FireworkStarFadeRecipe;
use super::firework_star_recipe::FireworkStarRecipe;
use super::imbue_recipe::ImbueRecipe;
use super::map_extending_recipe::MapExtendingRecipe;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use super::recipe::NOT_PLACEABLE;
use super::repair_item_recipe::RepairItemRecipe;
use super::shaped_recipe::ShapedRecipe;
use super::shapeless_recipe::ShapelessRecipe;
use super::shield_decoration_recipe::ShieldDecorationRecipe;
use super::transmute_recipe::TransmuteRecipe;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;

#[derive(Clone, Debug)]
pub struct CraftingBookInfo {
    pub category: CraftingBookCategory,
    pub group: String,
}

impl CraftingBookInfo {
    pub fn decode(tag: &Nbt) -> DataResult<CraftingBookInfo> {
        Ok(CraftingBookInfo {
            category: codec::decode_or(tag, "category", CraftingBookCategory::Misc, |t| codec::string_enum(t, &CraftingBookCategory::VALUES))?,
            group: codec::decode_or(tag, "group", String::new(), |t| codec::string(t).map(str::to_string))?,
        })
    }
}

pub enum CraftingRecipe {
    Shaped(ShapedRecipe),
    Shapeless(ShapelessRecipe),
    Transmute(TransmuteRecipe),
    Dye(DyeRecipe),
    Imbue(ImbueRecipe),
    BannerDuplicate(BannerDuplicateRecipe),
    BookCloning(BookCloningRecipe),
    DecoratedPot(DecoratedPotRecipe),
    FireworkRocket(FireworkRocketRecipe),
    FireworkStar(FireworkStarRecipe),
    FireworkStarFade(FireworkStarFadeRecipe),
    MapExtending(MapExtendingRecipe),
    RepairItem(RepairItemRecipe),
    ShieldDecoration(ShieldDecorationRecipe),
}

pub fn default_crafting_reminder(input: &CraftingInput) -> Vec<ItemStack> {
    (0..input.size()).map(|slot| input.get_item(slot).get_item().crafting_remainder().map_or_else(ItemStack::empty, |remainder| ItemStack::new(remainder, 1))).collect()
}

impl CraftingRecipe {
    pub fn decode(ty: &str, tag: &Nbt) -> DataResult<CraftingRecipe> {
        Ok(match ty {
            "minecraft:crafting_shaped" => CraftingRecipe::Shaped(ShapedRecipe::decode(tag)?),
            "minecraft:crafting_shapeless" => CraftingRecipe::Shapeless(ShapelessRecipe::decode(tag)?),
            "minecraft:crafting_transmute" => CraftingRecipe::Transmute(TransmuteRecipe::decode(tag)?),
            "minecraft:crafting_dye" => CraftingRecipe::Dye(DyeRecipe::decode(tag)?),
            "minecraft:crafting_imbue" => CraftingRecipe::Imbue(ImbueRecipe::decode(tag)?),
            "minecraft:crafting_special_bannerduplicate" => CraftingRecipe::BannerDuplicate(BannerDuplicateRecipe::decode(tag)?),
            "minecraft:crafting_special_bookcloning" => CraftingRecipe::BookCloning(BookCloningRecipe::decode(tag)?),
            "minecraft:crafting_decorated_pot" => CraftingRecipe::DecoratedPot(DecoratedPotRecipe::decode(tag)?),
            "minecraft:crafting_special_firework_rocket" => CraftingRecipe::FireworkRocket(FireworkRocketRecipe::decode(tag)?),
            "minecraft:crafting_special_firework_star" => CraftingRecipe::FireworkStar(FireworkStarRecipe::decode(tag)?),
            "minecraft:crafting_special_firework_star_fade" => CraftingRecipe::FireworkStarFade(FireworkStarFadeRecipe::decode(tag)?),
            "minecraft:crafting_special_mapextending" => CraftingRecipe::MapExtending(MapExtendingRecipe::decode(tag)?),
            "minecraft:crafting_special_repairitem" => CraftingRecipe::RepairItem(RepairItemRecipe),
            "minecraft:crafting_special_shielddecoration" => CraftingRecipe::ShieldDecoration(ShieldDecorationRecipe::decode(tag)?),
            _ => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:recipe_serializer]: {ty}")),
        })
    }

    fn normal(&self) -> Option<&NormalCraftingRecipe> {
        Some(match self {
            CraftingRecipe::Shaped(recipe) => &recipe.base,
            CraftingRecipe::Shapeless(recipe) => &recipe.base,
            CraftingRecipe::Transmute(recipe) => &recipe.base,
            CraftingRecipe::Dye(recipe) => &recipe.base,
            CraftingRecipe::Imbue(recipe) => &recipe.base,
            _ => return None,
        })
    }

    pub fn matches(&self, input: &mut CraftingInput, level: &mut ServerLevel) -> bool {
        match self {
            CraftingRecipe::Shaped(recipe) => recipe.matches(input),
            CraftingRecipe::Shapeless(recipe) => recipe.matches(input),
            CraftingRecipe::Transmute(recipe) => recipe.matches(input),
            CraftingRecipe::Dye(recipe) => recipe.matches(input),
            CraftingRecipe::Imbue(recipe) => recipe.matches(input),
            CraftingRecipe::BannerDuplicate(recipe) => recipe.matches(input),
            CraftingRecipe::BookCloning(recipe) => recipe.matches(input),
            CraftingRecipe::DecoratedPot(recipe) => recipe.matches(input),
            CraftingRecipe::FireworkRocket(recipe) => recipe.matches(input),
            CraftingRecipe::FireworkStar(recipe) => recipe.matches(input),
            CraftingRecipe::FireworkStarFade(recipe) => recipe.matches(input),
            CraftingRecipe::MapExtending(recipe) => recipe.matches(input, level),
            CraftingRecipe::RepairItem(recipe) => recipe.matches(input),
            CraftingRecipe::ShieldDecoration(recipe) => recipe.matches(input),
        }
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        match self {
            CraftingRecipe::Shaped(recipe) => recipe.assemble(),
            CraftingRecipe::Shapeless(recipe) => recipe.assemble(),
            CraftingRecipe::Transmute(recipe) => recipe.assemble(input),
            CraftingRecipe::Dye(recipe) => recipe.assemble(input),
            CraftingRecipe::Imbue(recipe) => recipe.assemble(input),
            CraftingRecipe::BannerDuplicate(recipe) => recipe.assemble(input),
            CraftingRecipe::BookCloning(recipe) => recipe.assemble(input),
            CraftingRecipe::DecoratedPot(recipe) => recipe.assemble(input),
            CraftingRecipe::FireworkRocket(recipe) => recipe.assemble(input),
            CraftingRecipe::FireworkStar(recipe) => recipe.assemble(input),
            CraftingRecipe::FireworkStarFade(recipe) => recipe.assemble(input),
            CraftingRecipe::MapExtending(recipe) => recipe.assemble(input),
            CraftingRecipe::RepairItem(recipe) => recipe.assemble(input),
            CraftingRecipe::ShieldDecoration(recipe) => recipe.assemble(input),
        }
    }

    pub fn get_remaining_items(&self, input: &CraftingInput) -> Vec<ItemStack> {
        match self {
            CraftingRecipe::BannerDuplicate(recipe) => recipe.get_remaining_items(input),
            CraftingRecipe::BookCloning(recipe) => recipe.get_remaining_items(input),
            _ => default_crafting_reminder(input),
        }
    }

    pub fn is_special(&self) -> bool {
        self.normal().is_none()
    }

    pub fn show_notification(&self) -> bool {
        self.normal().is_some_and(|normal| normal.common_info.show_notification)
    }

    pub fn group(&self) -> &str {
        self.normal().map_or("", |normal| &normal.book_info.group)
    }

    pub fn category(&self) -> CraftingBookCategory {
        self.normal().map_or(CraftingBookCategory::Misc, |normal| normal.book_info.category)
    }

    pub fn placement_info(&self) -> &PlacementInfo {
        self.normal().map_or(&NOT_PLACEABLE, |normal| &normal.placement_info)
    }
}
