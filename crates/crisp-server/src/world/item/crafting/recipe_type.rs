#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RecipeType {
    Crafting,
    Smelting,
    Blasting,
    Smoking,
    CampfireCooking,
    Stonecutting,
    Smithing,
    Brewing,
}

impl RecipeType {
    pub const VALUES: [RecipeType; 8] = [
        RecipeType::Crafting,
        RecipeType::Smelting,
        RecipeType::Blasting,
        RecipeType::Smoking,
        RecipeType::CampfireCooking,
        RecipeType::Stonecutting,
        RecipeType::Smithing,
        RecipeType::Brewing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RecipeType::Crafting => "crafting",
            RecipeType::Smelting => "smelting",
            RecipeType::Blasting => "blasting",
            RecipeType::Smoking => "smoking",
            RecipeType::CampfireCooking => "campfire_cooking",
            RecipeType::Stonecutting => "stonecutting",
            RecipeType::Smithing => "smithing",
            RecipeType::Brewing => "brewing",
        }
    }
}
