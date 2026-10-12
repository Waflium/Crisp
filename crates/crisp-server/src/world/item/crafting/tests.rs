use crisp_data::{Item, items};
use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::recipe_manager::RecipeManager;
use crate::server::level::testing;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

fn stack(item: Item) -> ItemStack {
    ItemStack::new(item, 1)
}

fn craft(width: usize, grid: &[Option<Item>]) -> (Option<&'static str>, ItemStack) {
    let mut level = testing::level();
    let items: Vec<ItemStack> = grid.iter().map(|item| item.map_or_else(ItemStack::empty, stack)).collect();
    let mut input = CraftingInput::of(width, grid.len() / width, &items);
    match RecipeManager::get().get_crafting_recipe_for(&mut input, &mut level, None) {
        Some(holder) => (Some(holder.id), holder.crafting().expect("a crafting recipe").assemble(&input)),
        None => (None, ItemStack::empty()),
    }
}

pub fn vanilla_data_files_with_extension(directory: &str, extension: &str) -> usize {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(crisp_data::VANILLA_DATA)).unwrap();
    archive.file_names().filter(|name| name.starts_with(&format!("data/minecraft/{directory}/")) && name.ends_with(extension)).count()
}

pub fn vanilla_data_files(directory: &str) -> usize {
    vanilla_data_files_with_extension(directory, ".json")
}

#[test]
fn every_vanilla_recipe_decodes() {
    assert_eq!(RecipeManager::get().get_recipes().len(), vanilla_data_files("recipe"));
}

#[test]
fn crafts_shaped_and_shapeless_recipes() {
    let (id, result) = craft(2, &[None, None, None, Some(items::OAK_LOG)]);
    assert_eq!((id, result.item, result.count), (Some("minecraft:oak_planks"), items::OAK_PLANKS, 4));
    let p = Some(items::OAK_PLANKS);
    let s = Some(items::STICK);
    let (id, result) = craft(3, &[p, p, p, None, s, None, None, s, None]);
    assert_eq!((id, result.item), (Some("minecraft:wooden_pickaxe"), items::WOODEN_PICKAXE));
    let (_, axe) = craft(3, &[p, p, None, p, s, None, None, s, None]);
    let (_, mirrored) = craft(3, &[None, p, p, None, s, p, None, s, None]);
    assert_eq!((axe.item, mirrored.item), (items::WOODEN_AXE, items::WOODEN_AXE));
    assert_eq!(craft(2, &[Some(items::DIRT), None, None, None]).0, None);
}

#[test]
fn crafts_special_recipes() {
    let (id, helmet) = craft(2, &[Some(items::LEATHER_HELMET), Some(items::RED_DYE), None, None]);
    assert_eq!(id, Some("minecraft:leather_helmet_dyed"));
    assert_eq!(helmet.get(dc::DYED_COLOR), Some(&Nbt::Int(11546150)));
    let (_, rocket) = craft(2, &[Some(items::PAPER), Some(items::GUNPOWDER), None, None]);
    assert_eq!((rocket.item, rocket.count), (items::FIREWORK_ROCKET, 3));
    let (_, shulker_box) = craft(2, &[Some(items::SHULKER_BOX), Some(items::RED_DYE), None, None]);
    assert_eq!(shulker_box.item, items::RED_SHULKER_BOX);
}
