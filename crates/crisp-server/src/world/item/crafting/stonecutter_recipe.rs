use std::ops::Deref;

use crisp_protocol::nbt::Nbt;

use super::single_item_recipe::SingleItemRecipe;
use crate::util::codec::DataResult;

pub struct StonecutterRecipe {
    base: SingleItemRecipe,
}

impl Deref for StonecutterRecipe {
    type Target = SingleItemRecipe;

    fn deref(&self) -> &SingleItemRecipe {
        &self.base
    }
}

impl StonecutterRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<StonecutterRecipe> {
        Ok(StonecutterRecipe { base: SingleItemRecipe::decode(tag)? })
    }
}
