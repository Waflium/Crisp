use crisp_data::{EffectInstanceTemplate, MobEffect, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::parse_identifier;
use crate::world::entity::data::ParticleOptions;

pub const INFINITE_DURATION: i32 = -1;

const AMBIENT_ALPHA: i32 = 38;

#[derive(Clone, PartialEq, Debug)]
pub struct MobEffectInstance {
    pub effect: MobEffect,
    pub duration: i32,
    pub amplifier: i32,
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
    pub hidden_effect: Option<Box<MobEffectInstance>>,
}

impl MobEffectInstance {
    pub fn new(effect: MobEffect, duration: i32, amplifier: i32) -> MobEffectInstance {
        MobEffectInstance::with(effect, duration, amplifier, false, true, true)
    }

    pub fn with(effect: MobEffect, duration: i32, amplifier: i32, ambient: bool, visible: bool, show_icon: bool) -> MobEffectInstance {
        MobEffectInstance { effect, duration, amplifier: amplifier.clamp(0, 255), ambient, visible, show_icon, hidden_effect: None }
    }

    pub fn from_template(t: &EffectInstanceTemplate) -> MobEffectInstance {
        MobEffectInstance::with(t.effect, t.duration, t.amplifier as i32, t.ambient, t.show_particles, t.show_icon)
    }

    fn copy(&self) -> MobEffectInstance {
        MobEffectInstance { hidden_effect: None, ..self.clone() }
    }

    fn set_details_from(&mut self, other: &MobEffectInstance) {
        self.duration = other.duration;
        self.amplifier = other.amplifier;
        self.ambient = other.ambient;
        self.visible = other.visible;
        self.show_icon = other.show_icon;
    }

    pub fn update(&mut self, take_over: &MobEffectInstance) -> bool {
        let mut changed = false;
        if take_over.amplifier > self.amplifier {
            if take_over.is_shorter_duration_than(self) {
                let previous_hidden = self.hidden_effect.take();
                let mut hidden = self.copy();
                hidden.hidden_effect = previous_hidden;
                self.hidden_effect = Some(Box::new(hidden));
            }
            self.amplifier = take_over.amplifier;
            self.duration = take_over.duration;
            changed = true;
        } else if self.is_shorter_duration_than(take_over) {
            if take_over.amplifier == self.amplifier {
                self.duration = take_over.duration;
                changed = true;
            } else {
                match &mut self.hidden_effect {
                    None => self.hidden_effect = Some(Box::new(take_over.copy())),
                    Some(hidden) => {
                        hidden.update(take_over);
                    }
                }
            }
        }
        if !take_over.ambient && self.ambient || changed {
            self.ambient = take_over.ambient;
            changed = true;
        }
        if take_over.visible != self.visible {
            self.visible = take_over.visible;
            changed = true;
        }
        if take_over.show_icon != self.show_icon {
            self.show_icon = take_over.show_icon;
            changed = true;
        }
        changed
    }

    fn is_shorter_duration_than(&self, other: &MobEffectInstance) -> bool {
        !self.is_infinite_duration() && (self.duration < other.duration || other.is_infinite_duration())
    }

    pub fn is_infinite_duration(&self) -> bool {
        self.duration == INFINITE_DURATION
    }

    pub fn ends_within(&self, ticks: i32) -> bool {
        !self.is_infinite_duration() && self.duration <= ticks
    }

    pub fn map_duration(&self, mapper: impl FnOnce(i32) -> i32) -> i32 {
        if !self.is_infinite_duration() && self.duration != 0 { mapper(self.duration) } else { self.duration }
    }

    pub fn with_scaled_duration(&self, scale: f32) -> MobEffectInstance {
        let mut copy = self.copy();
        copy.duration = copy.map_duration(|duration| crisp_core::mth::floor_f(duration as f32 * scale).max(1));
        copy
    }

    pub fn has_remaining_duration(&self) -> bool {
        self.is_infinite_duration() || self.duration > 0
    }

    pub fn tick_down_duration(&mut self) {
        if let Some(hidden) = &mut self.hidden_effect {
            hidden.tick_down_duration();
        }
        self.duration = self.map_duration(|d| d - 1);
    }

    pub fn downgrade_to_hidden_effect(&mut self) -> bool {
        if self.duration == 0
            && let Some(hidden) = self.hidden_effect.take()
        {
            self.set_details_from(&hidden);
            self.hidden_effect = hidden.hidden_effect;
            return true;
        }
        false
    }

    pub fn particle_options(&self) -> ParticleOptions {
        let info = self.effect.info();
        match info.particle {
            Some(particle) => ParticleOptions { particle_type: registry_id(registry::PARTICLE_TYPE, particle), color: None },
            None => {
                let alpha = if self.ambient { AMBIENT_ALPHA } else { 255 };
                let color = alpha << 24 | info.color & 0xFFFFFF;
                ParticleOptions { particle_type: registry_id(registry::PARTICLE_TYPE, "minecraft:entity_effect"), color: Some(color) }
            }
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("id".into(), Nbt::String(self.effect.info().name.into()))];
        self.write_details(&mut fields);
        Nbt::Compound(fields)
    }

    fn write_details(&self, fields: &mut Vec<(String, Nbt)>) {
        if self.amplifier != 0 {
            fields.push(("amplifier".into(), Nbt::Byte(self.amplifier as i8)));
        }
        if self.duration != 0 {
            fields.push(("duration".into(), Nbt::Int(self.duration)));
        }
        if self.ambient {
            fields.push(("ambient".into(), Nbt::Byte(1)));
        }
        if !self.visible {
            fields.push(("show_particles".into(), Nbt::Byte(0)));
        }
        fields.push(("show_icon".into(), Nbt::Byte(self.show_icon as i8)));
        if let Some(hidden) = &self.hidden_effect {
            let mut hidden_fields = Vec::new();
            hidden.write_details(&mut hidden_fields);
            fields.push(("hidden_effect".into(), Nbt::Compound(hidden_fields)));
        }
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<MobEffectInstance> {
        let effect = MobEffect::by_name(&parse_identifier(nbt.get("id")?.as_str()?)?)?;
        MobEffectInstance::read_details(effect, nbt)
    }

    fn read_details(effect: MobEffect, nbt: &Nbt) -> Option<MobEffectInstance> {
        let number = |key: &str| match nbt.get(key) {
            None => Some(None),
            Some(value) => numeric(value).map(Some),
        };
        let boolean = |key: &str, default: bool| number(key).map(|v| v.map_or(default, |v| v as i8 != 0));
        let amplifier = number("amplifier")?.map_or(0, |v| v as i8 as u8 as i32);
        let duration = number("duration")?.map_or(0, |v| v as i32);
        let ambient = boolean("ambient", false)?;
        let show_particles = boolean("show_particles", true)?;
        let show_icon = boolean("show_icon", show_particles)?;
        let hidden_effect = match nbt.get("hidden_effect") {
            None => None,
            Some(hidden) => Some(Box::new(MobEffectInstance::read_details(effect, hidden)?)),
        };
        Some(MobEffectInstance { effect, duration, amplifier, ambient, visible: show_particles, show_icon, hidden_effect })
    }
}

fn numeric(nbt: &Nbt) -> Option<i64> {
    match *nbt {
        Nbt::Float(v) => Some(v as i64),
        Nbt::Double(v) => Some(v as i64),
        _ => nbt.as_i64(),
    }
}
