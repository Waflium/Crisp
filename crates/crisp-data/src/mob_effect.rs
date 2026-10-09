use crate::MOB_EFFECTS;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct MobEffect(pub u8);

impl MobEffect {
    pub fn info(self) -> &'static MobEffectInfo {
        &MOB_EFFECTS[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<MobEffect> {
        MOB_EFFECTS.iter().position(|e| e.name == name).map(|id| MobEffect(id as u8))
    }

    pub fn protocol_id(self) -> i32 {
        self.0 as i32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobEffectCategory {
    Beneficial,
    Harmful,
    Neutral,
}

#[derive(Debug)]
pub struct MobEffectInfo {
    pub name: &'static str,
    pub description_id: &'static str,
    pub category: MobEffectCategory,
    pub color: i32,
    pub instantaneous: bool,
    pub blend_in_duration_ticks: i32,
    pub blend_out_duration_ticks: i32,
    pub blend_out_advance_ticks: i32,
    pub particle: Option<&'static str>,
    pub sound_on_added: Option<&'static str>,
    pub modifiers: &'static [EffectAttributeModifier],
}

#[derive(Debug)]
pub struct EffectAttributeModifier {
    pub attribute: u8,
    pub id: &'static str,
    pub amount: f64,
    pub operation: u8,
}

#[derive(Debug)]
pub struct EffectInstanceTemplate {
    pub effect: MobEffect,
    pub duration: i32,
    pub amplifier: u8,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sentiment {
    Positive,
    Neutral,
    Negative,
}

#[derive(Debug)]
pub struct AttributeInfo {
    pub name: &'static str,
    pub description_id: &'static str,
    pub default_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub syncable: bool,
    pub sentiment: Sentiment,
}
