#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecipeBookType {
    Crafting,
    Furnace,
    BlastFurnace,
    Smoker,
}

impl RecipeBookType {
    pub fn by_id(id: i32) -> Option<RecipeBookType> {
        Some(match id {
            0 => RecipeBookType::Crafting,
            1 => RecipeBookType::Furnace,
            2 => RecipeBookType::BlastFurnace,
            3 => RecipeBookType::Smoker,
            _ => return None,
        })
    }
}
