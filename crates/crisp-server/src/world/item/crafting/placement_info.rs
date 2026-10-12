use super::ingredient::Ingredient;

pub const EMPTY_SLOT: i32 = -1;

#[derive(Clone, Debug, Default)]
pub struct PlacementInfo {
    ingredients: Vec<Ingredient>,
    slots_to_ingredient_index: Vec<i32>,
}

impl PlacementInfo {
    pub const NOT_PLACEABLE: PlacementInfo = PlacementInfo { ingredients: Vec::new(), slots_to_ingredient_index: Vec::new() };

    pub fn create(ingredient: &Ingredient) -> PlacementInfo {
        if ingredient.is_empty() { PlacementInfo::NOT_PLACEABLE } else { PlacementInfo { ingredients: vec![ingredient.clone()], slots_to_ingredient_index: vec![0] } }
    }

    pub fn create_from_optionals(ingredients: &[Option<Ingredient>]) -> PlacementInfo {
        let mut present_ingredients = Vec::with_capacity(ingredients.len());
        let mut slots_to_ingredient_index = Vec::with_capacity(ingredients.len());
        let mut placement_index = 0;
        for maybe_ingredient in ingredients {
            match maybe_ingredient {
                Some(ingredient) => {
                    if ingredient.is_empty() {
                        return PlacementInfo::NOT_PLACEABLE;
                    }
                    present_ingredients.push(ingredient.clone());
                    slots_to_ingredient_index.push(placement_index);
                    placement_index += 1;
                }
                None => slots_to_ingredient_index.push(EMPTY_SLOT),
            }
        }
        PlacementInfo { ingredients: present_ingredients, slots_to_ingredient_index }
    }

    pub fn create_list(ingredients: Vec<Ingredient>) -> PlacementInfo {
        let mut slots_to_ingredient_index = Vec::with_capacity(ingredients.len());
        for (i, ingredient) in ingredients.iter().enumerate() {
            if ingredient.is_empty() {
                return PlacementInfo::NOT_PLACEABLE;
            }
            slots_to_ingredient_index.push(i as i32);
        }
        PlacementInfo { ingredients, slots_to_ingredient_index }
    }

    pub fn slots_to_ingredient_index(&self) -> &[i32] {
        &self.slots_to_ingredient_index
    }

    pub fn ingredients(&self) -> &[Ingredient] {
        &self.ingredients
    }

    pub fn is_impossible_to_place(&self) -> bool {
        self.slots_to_ingredient_index.is_empty()
    }
}
