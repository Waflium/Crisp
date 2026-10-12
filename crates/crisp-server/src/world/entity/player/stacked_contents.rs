pub trait IngredientInfo<T> {
    fn accepts_item(&self, item: T) -> bool;
}

pub struct StackedContents<T> {
    pub amounts: Vec<(T, i32)>,
}

impl<T> Default for StackedContents<T> {
    fn default() -> StackedContents<T> {
        StackedContents { amounts: Vec::new() }
    }
}

impl<T: Copy + PartialEq> StackedContents<T> {
    fn get_int(&self, item: T) -> i32 {
        self.amounts.iter().find(|(key, _)| *key == item).map_or(0, |&(_, count)| count)
    }

    fn add_to(&mut self, item: T, count: i32) -> i32 {
        match self.amounts.iter_mut().find(|(key, _)| *key == item) {
            Some((_, value)) => {
                let previous = *value;
                *value += count;
                previous
            }
            None => {
                self.amounts.push((item, count));
                0
            }
        }
    }

    fn has_at_least(&self, item: T, count: i32) -> bool {
        self.get_int(item) >= count
    }

    fn take(&mut self, item: T, amount: i32) {
        let previous = self.add_to(item, -amount);
        if previous < amount {
            panic!("Took {amount} items, but only had {previous}");
        }
    }

    fn put(&mut self, item: T, count: i32) {
        self.add_to(item, count);
    }

    pub fn try_pick(&mut self, ingredients: &[impl IngredientInfo<T>], amount: i32, output: Option<&mut dyn FnMut(T)>) -> bool {
        RecipePicker::new(self, ingredients).try_pick(self, amount, output)
    }

    pub fn try_pick_all(&mut self, ingredients: &[impl IngredientInfo<T>], max_size: i32, output: Option<&mut dyn FnMut(T)>) -> i32 {
        let mut picker = RecipePicker::new(self, ingredients);
        let result_upper_bound = self.get_result_upper_bound(ingredients);
        picker.try_pick_all(self, max_size, result_upper_bound, output)
    }

    pub fn clear(&mut self) {
        self.amounts.clear();
    }

    pub fn account(&mut self, item: T, count: i32) {
        self.put(item, count);
    }

    fn get_unique_available_ingredient_items(&self, ingredients: &[impl IngredientInfo<T>]) -> Vec<T> {
        self.amounts.iter().filter(|&&(item, count)| count > 0 && any_ingredient_matches(ingredients, item)).map(|&(item, _)| item).collect()
    }

    pub fn get_result_upper_bound(&self, ingredients: &[impl IngredientInfo<T>]) -> i32 {
        let mut min = i32::MAX;
        'ingredients: for ingredient in ingredients {
            let mut max = 0;
            for &(item, item_count) in &self.amounts {
                if item_count > max {
                    if ingredient.accepts_item(item) {
                        max = item_count;
                    }
                    if max >= min {
                        continue 'ingredients;
                    }
                }
            }
            min = max;
            if min == 0 {
                break;
            }
        }
        min
    }
}

fn any_ingredient_matches<T: Copy>(ingredients: &[impl IngredientInfo<T>], item: T) -> bool {
    ingredients.iter().any(|ingredient| ingredient.accepts_item(item))
}

struct RecipePicker<T> {
    ingredient_count: usize,
    items: Vec<T>,
    item_count: usize,
    data: Vec<bool>,
    path: Vec<usize>,
}

impl<T: Copy + PartialEq> RecipePicker<T> {
    fn new(contents: &StackedContents<T>, ingredients: &[impl IngredientInfo<T>]) -> RecipePicker<T> {
        let ingredient_count = ingredients.len();
        let items = contents.get_unique_available_ingredient_items(ingredients);
        let item_count = items.len();
        let size = ingredient_count + item_count + ingredient_count + ingredient_count * item_count * 2;
        let mut picker = RecipePicker { ingredient_count, items, item_count, data: vec![false; size], path: Vec::new() };
        picker.set_initial_connections(ingredients);
        picker
    }

    fn set_initial_connections(&mut self, ingredients: &[impl IngredientInfo<T>]) {
        for (ingredient, ingredient_info) in ingredients.iter().enumerate() {
            for item in 0..self.item_count {
                if ingredient_info.accepts_item(self.items[item]) {
                    self.set_connection(item, ingredient);
                }
            }
        }
    }

    fn try_pick(&mut self, contents: &mut StackedContents<T>, capacity: i32, mut output: Option<&mut dyn FnMut(T)>) -> bool {
        if capacity <= 0 {
            return true;
        }
        let mut satisfied_ingredient_count = 0;
        loop {
            let Some(path) = self.try_assigning_new_item(contents, capacity) else {
                let is_valid_assignment = satisfied_ingredient_count == self.ingredient_count;
                let has_output = is_valid_assignment && output.is_some();
                self.clear_all_visited();
                self.clear_satisfied();
                for ingredient in 0..self.ingredient_count {
                    for item in 0..self.item_count {
                        if self.is_assigned(item, ingredient) {
                            self.unassign(item, ingredient);
                            contents.put(self.items[item], capacity);
                            if has_output && let Some(output) = output.as_mut() {
                                output(self.items[item]);
                            }
                            break;
                        }
                    }
                }
                return is_valid_assignment;
            };
            let assigned_item = path[0];
            contents.take(self.items[assigned_item], capacity);
            let satisfied_ingredient = path.len() - 1;
            self.set_satisfied(path[satisfied_ingredient]);
            satisfied_ingredient_count += 1;
            for i in 0..path.len() - 1 {
                if is_path_index_item(i) {
                    self.assign(path[i], path[i + 1]);
                } else {
                    self.unassign(path[i + 1], path[i]);
                }
            }
        }
    }

    fn try_assigning_new_item(&mut self, contents: &StackedContents<T>, capacity: i32) -> Option<Vec<usize>> {
        self.clear_all_visited();
        for item in 0..self.item_count {
            if contents.has_at_least(self.items[item], capacity)
                && let Some(path) = self.find_new_item_assignment_path(item)
            {
                return Some(path);
            }
        }
        None
    }

    fn find_new_item_assignment_path(&mut self, starting_item: usize) -> Option<Vec<usize>> {
        self.path.clear();
        self.visit_item(starting_item);
        self.path.push(starting_item);
        while !self.path.is_empty() {
            let path_length = self.path.len();
            if is_path_index_item(path_length - 1) {
                let item_to_assign = self.path[path_length - 1];
                for ingredient in 0..self.ingredient_count {
                    if !self.has_visited_ingredient(ingredient) && self.has_connection(item_to_assign, ingredient) && !self.is_assigned(item_to_assign, ingredient) {
                        self.visit_ingredient(ingredient);
                        self.path.push(ingredient);
                        break;
                    }
                }
            } else {
                let last_assigned_ingredient = self.path[path_length - 1];
                if !self.is_satisfied(last_assigned_ingredient) {
                    return Some(self.path.clone());
                }
                for item in 0..self.item_count {
                    if !self.has_visited_item(item) && self.is_assigned(item, last_assigned_ingredient) {
                        self.visit_item(item);
                        self.path.push(item);
                        break;
                    }
                }
            }
            let new_length = self.path.len();
            if new_length == path_length {
                self.path.remove(new_length - 1);
            }
        }
        None
    }

    fn visited_ingredient_offset(&self) -> usize {
        0
    }

    fn visited_item_offset(&self) -> usize {
        self.visited_ingredient_offset() + self.ingredient_count
    }

    fn satisfied_offset(&self) -> usize {
        self.visited_item_offset() + self.item_count
    }

    fn connection_offset(&self) -> usize {
        self.satisfied_offset() + self.ingredient_count
    }

    fn residual_offset(&self) -> usize {
        self.connection_offset() + self.ingredient_count * self.item_count
    }

    fn is_satisfied(&self, ingredient: usize) -> bool {
        self.data[self.satisfied_offset() + ingredient]
    }

    fn set_satisfied(&mut self, ingredient: usize) {
        let index = self.satisfied_offset() + ingredient;
        self.data[index] = true;
    }

    fn clear_satisfied(&mut self) {
        let offset = self.satisfied_offset();
        self.clear_range(offset, self.ingredient_count);
    }

    fn set_connection(&mut self, item: usize, ingredient: usize) {
        let index = self.connection_offset() + item * self.ingredient_count + ingredient;
        self.data[index] = true;
    }

    fn has_connection(&self, item: usize, ingredient: usize) -> bool {
        self.data[self.connection_offset() + item * self.ingredient_count + ingredient]
    }

    fn residual_index(&self, item: usize, ingredient: usize) -> usize {
        self.residual_offset() + item * self.ingredient_count + ingredient
    }

    fn is_assigned(&self, item: usize, ingredient: usize) -> bool {
        self.data[self.residual_index(item, ingredient)]
    }

    fn assign(&mut self, item: usize, ingredient: usize) {
        let index = self.residual_index(item, ingredient);
        self.data[index] = true;
    }

    fn unassign(&mut self, item: usize, ingredient: usize) {
        let index = self.residual_index(item, ingredient);
        self.data[index] = false;
    }

    fn visit_ingredient(&mut self, ingredient: usize) {
        let index = self.visited_ingredient_offset() + ingredient;
        self.data[index] = true;
    }

    fn has_visited_ingredient(&self, ingredient: usize) -> bool {
        self.data[self.visited_ingredient_offset() + ingredient]
    }

    fn visit_item(&mut self, item: usize) {
        let index = self.visited_item_offset() + item;
        self.data[index] = true;
    }

    fn has_visited_item(&self, item: usize) -> bool {
        self.data[self.visited_item_offset() + item]
    }

    fn clear_all_visited(&mut self) {
        let (ingredient_offset, item_offset) = (self.visited_ingredient_offset(), self.visited_item_offset());
        self.clear_range(ingredient_offset, self.ingredient_count);
        self.clear_range(item_offset, self.item_count);
    }

    fn clear_range(&mut self, offset: usize, count: usize) {
        self.data[offset..offset + count].fill(false);
    }

    fn try_pick_all(&mut self, contents: &mut StackedContents<T>, max_size: i32, result_upper_bound: i32, output: Option<&mut dyn FnMut(T)>) -> i32 {
        let mut min = 0i32;
        let mut max = max_size.min(result_upper_bound).wrapping_add(1);
        loop {
            let mid = min.wrapping_add(max) / 2;
            if self.try_pick(contents, mid, None) {
                if max.wrapping_sub(min) <= 1 {
                    if mid > 0 {
                        self.try_pick(contents, mid, output);
                    }
                    return mid;
                }
                min = mid;
            } else {
                max = mid;
            }
        }
    }
}

fn is_path_index_item(index: usize) -> bool {
    index & 1 == 0
}
