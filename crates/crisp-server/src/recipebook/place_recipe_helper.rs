use crisp_core::mth;

use crate::world::item::crafting::crafting_recipe::CraftingRecipe;
use crate::world::item::crafting::recipe::Recipe;

pub fn place_recipe<T>(grid_width: i32, grid_height: i32, recipe: &Recipe, entries: impl IntoIterator<Item = T>, output: impl FnMut(T, i32, i32, i32)) {
    if let Recipe::Crafting(CraftingRecipe::Shaped(shaped_recipe)) = recipe {
        place_recipe_sized(grid_width, grid_height, shaped_recipe.get_width() as i32, shaped_recipe.get_height() as i32, entries, output);
    } else {
        place_recipe_sized(grid_width, grid_height, grid_width, grid_height, entries, output);
    }
}

pub fn place_recipe_sized<T>(grid_width: i32, grid_height: i32, recipe_width: i32, recipe_height: i32, entries: impl IntoIterator<Item = T>, mut output: impl FnMut(T, i32, i32, i32)) {
    let mut iterator = entries.into_iter().peekable();
    let mut grid_index = 0;
    let mut grid_y_pos = 0;
    while grid_y_pos < grid_height {
        let should_center_recipe = (recipe_height as f32) < grid_height as f32 / 2.0;
        let start_pos_center_recipe = mth::floor_f(grid_height as f32 / 2.0 - recipe_height as f32 / 2.0);
        if should_center_recipe && start_pos_center_recipe > grid_y_pos {
            grid_index += grid_width;
            grid_y_pos += 1;
        }
        for grid_x_pos in 0..grid_width {
            if iterator.peek().is_none() {
                return;
            }
            let should_center_recipe = (recipe_width as f32) < grid_width as f32 / 2.0;
            let start_pos_center_recipe = mth::floor_f(grid_width as f32 / 2.0 - recipe_width as f32 / 2.0);
            let mut total_recipe_width_in_grid = recipe_width;
            let mut add_ingredient_to_slot = grid_x_pos < recipe_width;
            if should_center_recipe {
                total_recipe_width_in_grid = start_pos_center_recipe + recipe_width;
                add_ingredient_to_slot = start_pos_center_recipe <= grid_x_pos && grid_x_pos < start_pos_center_recipe + recipe_width;
            }
            if add_ingredient_to_slot {
                let item = iterator.next().expect("peeked an entry");
                output(item, grid_index, grid_x_pos, grid_y_pos);
            } else if total_recipe_width_in_grid == grid_x_pos {
                grid_index += grid_width - grid_x_pos;
                break;
            }
            grid_index += 1;
        }
        grid_y_pos += 1;
    }
}
