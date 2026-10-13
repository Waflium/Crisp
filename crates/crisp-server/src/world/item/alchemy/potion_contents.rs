use crisp_protocol::nbt::Nbt;

use super::potions::{self, Potion};
use crate::world::effect::MobEffectInstance;

pub const BASE_POTION_COLOR: i32 = -13083194;

pub struct PotionContents<'a> {
    tag: &'a Nbt,
}

impl<'a> PotionContents<'a> {
    pub fn new(tag: &'a Nbt) -> PotionContents<'a> {
        PotionContents { tag }
    }

    pub fn potion(&self) -> Option<&'static Potion> {
        self.potion_id().and_then(potions::by_id)
    }

    pub fn potion_id(&self) -> Option<&str> {
        match self.tag {
            Nbt::String(potion) => Some(potion),
            tag => tag.get("potion").and_then(Nbt::as_str),
        }
    }

    pub fn custom_color(&self) -> Option<i32> {
        self.tag.get("custom_color").and_then(Nbt::as_i64).map(|color| color as i32)
    }

    pub fn custom_effects(&self) -> Vec<MobEffectInstance> {
        self.tag.get("custom_effects").map_or(Vec::new(), |effects| effects.as_list().iter().filter_map(MobEffectInstance::from_nbt).collect())
    }

    pub fn is_potion_without_custom_effects(&self) -> bool {
        self.potion().is_some() && self.custom_effects().is_empty()
    }

    pub fn is_tag(&self, tag: &str) -> bool {
        self.is_potion_without_custom_effects()
            && self.potion_id().is_some_and(|id| {
                let id = crisp_data::registry_id(crisp_data::registry::POTION, &crate::commands::arguments::parse_identifier(id).unwrap_or_else(|| id.to_string()));
                crate::commands::arguments::registry_tag("minecraft:potion", tag).is_some_and(|ids| ids.contains(&id))
            })
    }

    pub fn get_all_effects(&self) -> Vec<MobEffectInstance> {
        let mut effects = self.potion().map_or(Vec::new(), Potion::effects);
        effects.extend(self.custom_effects());
        effects
    }

    pub fn for_each_effect(&self, duration_scale: f32) -> Vec<MobEffectInstance> {
        self.get_all_effects().iter().map(|effect| effect.with_scaled_duration(duration_scale)).collect()
    }

    pub fn has_effects(&self) -> bool {
        if !self.custom_effects().is_empty() { true } else { self.potion().is_some_and(|potion| !potion.effects().is_empty()) }
    }

    pub fn get_color(&self) -> i32 {
        self.get_color_or(BASE_POTION_COLOR)
    }

    pub fn get_color_or(&self, default_color: i32) -> i32 {
        match self.custom_color() {
            Some(color) => color,
            None => get_color_optional(&self.get_all_effects()).unwrap_or(default_color),
        }
    }
}

pub fn get_color_optional(effects: &[MobEffectInstance]) -> Option<i32> {
    let (mut red, mut green, mut blue, mut total_weight) = (0, 0, 0, 0);
    for effect in effects {
        if effect.visible {
            let color = effect.effect.info().color;
            let amplifier = effect.amplifier + 1;
            red += amplifier * (color >> 16 & 0xFF);
            green += amplifier * (color >> 8 & 0xFF);
            blue += amplifier * (color & 0xFF);
            total_weight += amplifier;
        }
    }
    (total_weight != 0).then(|| (0xFF << 24) | (red / total_weight & 0xFF) << 16 | (green / total_weight & 0xFF) << 8 | blue / total_weight & 0xFF)
}

pub fn has_instant_effects(potion: &Potion) -> bool {
    potion.effects().iter().any(|effect| effect.effect.info().instantaneous)
}

pub fn with_effect_added(contents: Option<&Nbt>, effect: &MobEffectInstance) -> Nbt {
    let mut fields = match contents {
        Some(Nbt::String(potion)) => vec![("potion".to_string(), Nbt::String(potion.clone()))],
        Some(Nbt::Compound(fields)) => fields.clone(),
        _ => Vec::new(),
    };
    match fields.iter_mut().find(|(key, _)| key == "custom_effects") {
        Some((_, Nbt::List(effects))) => effects.push(effect.to_nbt()),
        _ => fields.push(("custom_effects".into(), Nbt::List(vec![effect.to_nbt()]))),
    }
    Nbt::Compound(fields)
}
