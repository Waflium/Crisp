use crate::world::entity::player::stacked_item_contents::StackedItemContents;
use crate::world::item::ItemStack;

pub struct CraftingInput {
    width: usize,
    height: usize,
    items: Vec<ItemStack>,
    stacked_contents: StackedItemContents,
    ingredient_count: usize,
}

pub struct Positioned {
    pub input: CraftingInput,
    pub left: usize,
    pub top: usize,
}

impl CraftingInput {
    fn new(width: usize, height: usize, items: Vec<ItemStack>) -> CraftingInput {
        let mut stacked_contents = StackedItemContents::default();
        let mut ingredient_count = 0;
        for item in &items {
            if !item.is_empty() {
                ingredient_count += 1;
                stacked_contents.account_stack_with_max(item, 1);
            }
        }
        CraftingInput { width, height, items, stacked_contents, ingredient_count }
    }

    pub fn empty() -> CraftingInput {
        CraftingInput { width: 0, height: 0, items: Vec::new(), stacked_contents: StackedItemContents::default(), ingredient_count: 0 }
    }

    pub fn of(width: usize, height: usize, items: &[ItemStack]) -> CraftingInput {
        CraftingInput::of_positioned(width, height, items).input
    }

    pub fn of_positioned(width: usize, height: usize, items: &[ItemStack]) -> Positioned {
        if width == 0 || height == 0 {
            return Positioned { input: CraftingInput::empty(), left: 0, top: 0 };
        }
        let mut left = width as i32 - 1;
        let mut right = 0i32;
        let mut top = height as i32 - 1;
        let mut bottom = 0i32;
        for y in 0..height {
            let mut row_empty = true;
            for x in 0..width {
                if !items[x + y * width].is_empty() {
                    left = left.min(x as i32);
                    right = right.max(x as i32);
                    row_empty = false;
                }
            }
            if !row_empty {
                top = top.min(y as i32);
                bottom = bottom.max(y as i32);
            }
        }
        let new_width = right - left + 1;
        let new_height = bottom - top + 1;
        if new_width <= 0 || new_height <= 0 {
            return Positioned { input: CraftingInput::empty(), left: 0, top: 0 };
        }
        let (left, top, new_width, new_height) = (left as usize, top as usize, new_width as usize, new_height as usize);
        if new_width == width && new_height == height {
            return Positioned { input: CraftingInput::new(width, height, items.to_vec()), left, top };
        }
        let mut new_items = Vec::with_capacity(new_width * new_height);
        for y in 0..new_height {
            for x in 0..new_width {
                new_items.push(items[x + left + (y + top) * width].clone());
            }
        }
        Positioned { input: CraftingInput::new(new_width, new_height, new_items), left, top }
    }

    pub fn get_item(&self, index: usize) -> &ItemStack {
        &self.items[index]
    }

    pub fn get_item_at(&self, x: usize, y: usize) -> &ItemStack {
        &self.items[x + y * self.width]
    }

    pub fn size(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ingredient_count == 0
    }

    pub fn stacked_contents(&mut self) -> &mut StackedItemContents {
        &mut self.stacked_contents
    }

    pub fn items(&self) -> &[ItemStack] {
        &self.items
    }

    pub fn ingredient_count(&self) -> usize {
        self.ingredient_count
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}
