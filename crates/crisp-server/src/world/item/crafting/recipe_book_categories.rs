#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecipeBookCategory {
    CraftingBuildingBlocks,
    CraftingRedstone,
    CraftingEquipment,
    CraftingMisc,
    FurnaceFood,
    FurnaceBlocks,
    FurnaceMisc,
    BlastFurnaceBlocks,
    BlastFurnaceMisc,
    SmokerFood,
    Stonecutter,
    Smithing,
    Campfire,
}

impl RecipeBookCategory {
    pub fn name(self) -> &'static str {
        match self {
            RecipeBookCategory::CraftingBuildingBlocks => "minecraft:crafting_building_blocks",
            RecipeBookCategory::CraftingRedstone => "minecraft:crafting_redstone",
            RecipeBookCategory::CraftingEquipment => "minecraft:crafting_equipment",
            RecipeBookCategory::CraftingMisc => "minecraft:crafting_misc",
            RecipeBookCategory::FurnaceFood => "minecraft:furnace_food",
            RecipeBookCategory::FurnaceBlocks => "minecraft:furnace_blocks",
            RecipeBookCategory::FurnaceMisc => "minecraft:furnace_misc",
            RecipeBookCategory::BlastFurnaceBlocks => "minecraft:blast_furnace_blocks",
            RecipeBookCategory::BlastFurnaceMisc => "minecraft:blast_furnace_misc",
            RecipeBookCategory::SmokerFood => "minecraft:smoker_food",
            RecipeBookCategory::Stonecutter => "minecraft:stonecutter",
            RecipeBookCategory::Smithing => "minecraft:smithing",
            RecipeBookCategory::Campfire => "minecraft:campfire",
        }
    }
}
