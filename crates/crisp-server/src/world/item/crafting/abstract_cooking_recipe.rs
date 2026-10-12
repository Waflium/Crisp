use std::ops::Deref;

use crisp_protocol::nbt::Nbt;

use super::cooking_book_category::CookingBookCategory;
use super::recipe_type::RecipeType;
use super::single_item_recipe::SingleItemRecipe;
use crate::util::codec::{self, DataResult};

#[derive(Clone, Debug)]
pub struct CookingBookInfo {
    pub category: CookingBookCategory,
    pub group: String,
}

impl CookingBookInfo {
    pub fn decode(tag: &Nbt) -> DataResult<CookingBookInfo> {
        Ok(CookingBookInfo {
            category: codec::decode_or(tag, "category", CookingBookCategory::Misc, |t| codec::string_enum(t, &CookingBookCategory::VALUES))?,
            group: codec::decode_or(tag, "group", String::new(), |t| codec::string(t).map(str::to_string))?,
        })
    }
}

pub struct AbstractCookingRecipe {
    base: SingleItemRecipe,
    ty: RecipeType,
    book_info: CookingBookInfo,
    experience: f32,
    cooking_time: i32,
}

impl Deref for AbstractCookingRecipe {
    type Target = SingleItemRecipe;

    fn deref(&self) -> &SingleItemRecipe {
        &self.base
    }
}

impl AbstractCookingRecipe {
    pub fn decode(ty: RecipeType, tag: &Nbt) -> DataResult<AbstractCookingRecipe> {
        let book_info = CookingBookInfo::decode(tag)?;
        let base = SingleItemRecipe::decode(tag)?;
        let experience = codec::decode_or(tag, "experience", 0.0, codec::float)?;
        let cooking_time = codec::decode_field(tag, "cookingtime", codec::int)?;
        Ok(AbstractCookingRecipe { base, ty, book_info, experience, cooking_time })
    }

    pub fn get_type(&self) -> RecipeType {
        self.ty
    }

    pub fn experience(&self) -> f32 {
        self.experience
    }

    pub fn cooking_time(&self) -> i32 {
        self.cooking_time
    }

    pub fn category(&self) -> CookingBookCategory {
        self.book_info.category
    }

    pub fn group(&self) -> &str {
        &self.book_info.group
    }
}
