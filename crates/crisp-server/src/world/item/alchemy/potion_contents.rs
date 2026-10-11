use crisp_protocol::nbt::Nbt;

use super::potions::{self, Potion};
use crate::world::effect::MobEffectInstance;

pub struct PotionContents<'a> {
    tag: &'a Nbt,
}

impl<'a> PotionContents<'a> {
    pub fn new(tag: &'a Nbt) -> PotionContents<'a> {
        PotionContents { tag }
    }

    pub fn potion(&self) -> Option<&'static Potion> {
        self.tag.get("potion").and_then(Nbt::as_str).and_then(potions::by_id)
    }

    pub fn potion_id(&self) -> Option<&str> {
        self.tag.get("potion").and_then(Nbt::as_str)
    }

    pub fn custom_effects(&self) -> Vec<MobEffectInstance> {
        self.tag.get("custom_effects").map_or(Vec::new(), |effects| effects.as_list().iter().filter_map(MobEffectInstance::from_nbt).collect())
    }

    pub fn get_all_effects(&self) -> Vec<MobEffectInstance> {
        let mut effects = self.potion().map_or(Vec::new(), Potion::effects);
        effects.extend(self.custom_effects());
        effects
    }
}
