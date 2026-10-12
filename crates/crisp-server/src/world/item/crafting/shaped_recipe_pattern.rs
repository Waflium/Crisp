use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use crate::util::codec::{self, DataResult};

const MAX_SIZE: usize = 3;
pub const EMPTY_SLOT: char = ' ';

pub struct ShapedRecipePattern {
    width: usize,
    height: usize,
    ingredients: Vec<Option<Ingredient>>,
    ingredient_count: usize,
    symmetrical: bool,
}

fn is_symmetrical(width: usize, height: usize, ingredients: &[Option<Ingredient>]) -> bool {
    if width == 1 {
        return true;
    }
    let center_x = width / 2;
    for y in 0..height {
        for left_x in 0..center_x {
            let right_x = width - 1 - left_x;
            if ingredients[left_x + y * width] != ingredients[right_x + y * width] {
                return false;
            }
        }
    }
    true
}

fn decode_pattern(tag: &Nbt) -> DataResult<Vec<String>> {
    let strings = codec::list_of(tag, |line| codec::string(line).map(str::to_string))?;
    if strings.len() > MAX_SIZE {
        return Err("Invalid pattern: too many rows, 3 is maximum".into());
    }
    let Some(first) = strings.first() else { return Err("Invalid pattern: empty pattern not allowed".into()) };
    let first_length = first.chars().count();
    for line in &strings {
        let length = line.chars().count();
        if length > MAX_SIZE {
            return Err("Invalid pattern: too many columns, 3 is maximum".into());
        }
        if first_length != length {
            return Err("Invalid pattern: each row must be the same width".into());
        }
    }
    Ok(strings)
}

fn decode_symbol(symbol: &str) -> DataResult<char> {
    let mut chars = symbol.chars();
    match (chars.next(), chars.next()) {
        (Some(' '), None) => Err("Invalid key entry: ' ' is a reserved symbol.".into()),
        (Some(c), None) => Ok(c),
        _ => Err(format!("Invalid key entry: '{symbol}' is an invalid symbol (must be 1 character only).")),
    }
}

fn first_non_empty(line: &[char]) -> usize {
    line.iter().take_while(|&&c| c == ' ').count()
}

fn last_non_empty(line: &[char]) -> i32 {
    let mut index = line.len() as i32 - 1;
    while index >= 0 && line[index as usize] == ' ' {
        index -= 1;
    }
    index
}

pub fn shrink(pattern: &[Vec<char>]) -> Vec<Vec<char>> {
    let mut left = usize::MAX;
    let mut right = 0i32;
    let mut top = 0;
    let mut bottom = 0;
    for (i, line) in pattern.iter().enumerate() {
        left = left.min(first_non_empty(line));
        let last_non_space = last_non_empty(line);
        right = right.max(last_non_space);
        if last_non_space < 0 {
            if top == i {
                top += 1;
            }
            bottom += 1;
        } else {
            bottom = 0;
        }
    }
    if pattern.len() == bottom {
        return Vec::new();
    }
    (0..pattern.len() - bottom - top).map(|line| pattern[line + top][left..right as usize + 1].to_vec()).collect()
}

impl ShapedRecipePattern {
    pub fn new(width: usize, height: usize, ingredients: Vec<Option<Ingredient>>) -> ShapedRecipePattern {
        let ingredient_count = ingredients.iter().flatten().count();
        let symmetrical = is_symmetrical(width, height, &ingredients);
        ShapedRecipePattern { width, height, ingredients, ingredient_count, symmetrical }
    }

    pub fn of(key: &[(char, Ingredient)], pattern: &[&str]) -> ShapedRecipePattern {
        let pattern: Vec<Vec<char>> = pattern.iter().map(|line| line.chars().collect()).collect();
        ShapedRecipePattern::unpack(key, &pattern).expect("valid pattern")
    }

    pub fn decode(tag: &Nbt) -> DataResult<ShapedRecipePattern> {
        let key = codec::decode_field(tag, "key", |key| codec::unbounded_map(key, decode_symbol, Ingredient::decode))?;
        let pattern = codec::decode_field(tag, "pattern", decode_pattern)?;
        let pattern: Vec<Vec<char>> = pattern.iter().map(|line| line.chars().collect()).collect();
        ShapedRecipePattern::unpack(&key, &pattern)
    }

    fn unpack(key: &[(char, Ingredient)], pattern: &[Vec<char>]) -> DataResult<ShapedRecipePattern> {
        let shrunk_pattern = shrink(pattern);
        let Some(first) = shrunk_pattern.first() else { return Err("Invalid pattern: empty pattern not allowed".into()) };
        let width = first.len();
        let height = shrunk_pattern.len();
        let mut ingredients = Vec::with_capacity(width * height);
        let mut unused_symbols: Vec<char> = key.iter().map(|&(symbol, _)| symbol).collect();
        for line in &shrunk_pattern {
            for &symbol in line {
                let ingredient = if symbol == EMPTY_SLOT {
                    None
                } else {
                    let Some((_, ingredient_for_symbol)) = key.iter().find(|(s, _)| *s == symbol) else {
                        return Err(format!("Pattern references symbol '{symbol}' but it's not defined in the key"));
                    };
                    Some(ingredient_for_symbol.clone())
                };
                unused_symbols.retain(|&s| s != symbol);
                ingredients.push(ingredient);
            }
        }
        if !unused_symbols.is_empty() {
            return Err(format!("Key defines symbols that aren't used in pattern: {unused_symbols:?}"));
        }
        Ok(ShapedRecipePattern::new(width, height, ingredients))
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() != self.ingredient_count {
            return false;
        }
        if input.width() == self.width && input.height() == self.height {
            if !self.symmetrical && self.matches_flip(input, true) {
                return true;
            }
            if self.matches_flip(input, false) {
                return true;
            }
        }
        false
    }

    fn matches_flip(&self, input: &CraftingInput, x_flip: bool) -> bool {
        for y in 0..self.height {
            for x in 0..self.width {
                let expected = if x_flip { &self.ingredients[self.width - x - 1 + y * self.width] } else { &self.ingredients[x + y * self.width] };
                if !Ingredient::test_optional_ingredient(expected.as_ref(), input.get_item_at(x, y)) {
                    return false;
                }
            }
        }
        true
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn ingredients(&self) -> &[Option<Ingredient>] {
        &self.ingredients
    }
}
