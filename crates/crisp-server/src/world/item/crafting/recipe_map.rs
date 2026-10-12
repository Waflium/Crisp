use crisp_core::hash_map::FastMap;

use super::recipe_holder::RecipeHolder;
use super::recipe_type::RecipeType;

pub struct RecipeMap {
    values: Vec<RecipeHolder>,
    by_type: Vec<Vec<usize>>,
    by_key: FastMap<&'static str, usize>,
}

impl RecipeMap {
    pub fn create(recipes: Vec<RecipeHolder>) -> RecipeMap {
        let mut by_type = vec![Vec::new(); RecipeType::VALUES.len()];
        let mut by_key = FastMap::default();
        for (index, recipe) in recipes.iter().enumerate() {
            by_type[recipe.value.get_type() as usize].push(index);
            by_key.insert(recipe.id, index);
        }
        RecipeMap { values: recipes, by_type, by_key }
    }

    pub fn by_type(&self, ty: RecipeType) -> impl Iterator<Item = &RecipeHolder> {
        self.by_type[ty as usize].iter().map(|&index| &self.values[index])
    }

    pub fn values(&self) -> &[RecipeHolder] {
        &self.values
    }

    pub fn by_key(&self, recipe_id: &str) -> Option<&RecipeHolder> {
        self.by_key.get(recipe_id).map(|&index| &self.values[index])
    }
}
