use crisp_data::packets::play::clientbound;
use crisp_data::{MobEffect, entity_type_tags, mob_effects};

use super::data::{DataValue, ids};
use super::{EntityId, RemovalReason, flags};
use crate::network::connection::encode;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::effect::mob_effect::{add_attribute_modifiers, remove_attribute_modifiers, should_apply_effect_tick_this_tick};

pub fn update_mob_effect(entity_id: EntityId, effect: &MobEffectInstance, blend: bool) -> Vec<u8> {
    encode(clientbound::UPDATE_MOB_EFFECT, |p| {
        p.varint(entity_id);
        p.varint(effect.effect.protocol_id());
        p.varint(effect.amplifier);
        p.varint(effect.duration);
        p.u8(effect.ambient as u8 | (effect.visible as u8) << 1 | (effect.show_icon as u8) << 2 | (blend as u8) << 3);
    })
}

fn remove_mob_effect(entity_id: EntityId, effect: MobEffect) -> Vec<u8> {
    encode(clientbound::REMOVE_MOB_EFFECT, |p| {
        p.varint(entity_id);
        p.varint(effect.protocol_id());
    })
}

fn are_all_effects_ambient(effects: &[MobEffectInstance]) -> bool {
    effects.iter().all(|e| !e.visible || e.ambient)
}

impl super::living::LivingEntity {
    fn effect_index(&self, effect: MobEffect) -> Result<usize, usize> {
        self.active_effects.binary_search_by_key(&effect, |e| e.effect)
    }

    pub fn has_effect(&self, effect: MobEffect) -> bool {
        self.effect_index(effect).is_ok()
    }

    pub fn effect(&self, effect: MobEffect) -> Option<&MobEffectInstance> {
        self.effect_index(effect).ok().map(|i| &self.active_effects[i])
    }

    fn remove_effect_no_update(&mut self, effect: MobEffect) -> Option<MobEffectInstance> {
        let index = self.effect_index(effect).ok()?;
        self.effects_mod_count += 1;
        Some(self.active_effects.remove(index))
    }
}

impl ServerLevel {
    pub fn has_effect(&self, id: EntityId, effect: MobEffect) -> bool {
        self.living(id).has_effect(effect)
    }

    pub fn add_effect(&mut self, id: EntityId, new_effect: MobEffectInstance, source: Option<EntityId>) -> bool {
        if !self.can_be_affected(id, &new_effect) {
            return false;
        }
        let (effect, amplifier) = (new_effect.effect, new_effect.amplifier);
        let living = &mut self.living_mut(id);
        let changed = match living.effect_index(effect) {
            Err(index) => {
                living.active_effects.insert(index, new_effect);
                living.effects_mod_count += 1;
                self.on_effect_added(id, effect, source);
                self.on_mob_effect_added(id, effect);
                true
            }
            Ok(index) => {
                let changed = living.active_effects[index].update(&new_effect);
                if changed {
                    self.on_effect_updated(id, effect, true, source);
                }
                changed
            }
        };
        self.on_effect_started(id, effect, amplifier);
        changed
    }

    pub fn can_be_affected(&self, id: EntityId, effect: &MobEffectInstance) -> bool {
        let entity_type = self.entity(id).entity_type as usize;
        if entity_type_tags::IMMUNE_TO_INFESTED.contains(entity_type) {
            effect.effect != mob_effects::INFESTED
        } else if entity_type_tags::IMMUNE_TO_OOZING.contains(entity_type) {
            effect.effect != mob_effects::OOZING
        } else {
            !entity_type_tags::IGNORES_POISON_AND_REGEN.contains(entity_type) || effect.effect != mob_effects::REGENERATION && effect.effect != mob_effects::POISON
        }
    }

    pub fn remove_effect(&mut self, id: EntityId, effect: MobEffect) -> bool {
        match self.living_mut(id).remove_effect_no_update(effect) {
            Some(removed) => {
                self.on_effects_removed(id, &[removed]);
                true
            }
            None => false,
        }
    }

    pub fn remove_all_effects(&mut self, id: EntityId) -> bool {
        let living = &mut self.living_mut(id);
        if living.active_effects.is_empty() {
            return false;
        }
        let removed = std::mem::take(&mut living.active_effects);
        living.effects_mod_count += 1;
        self.on_effects_removed(id, &removed);
        true
    }

    fn on_effect_added(&mut self, id: EntityId, effect: MobEffect, _source: Option<EntityId>) {
        let living = self.living_mut(id);
        living.effects_dirty = true;
        let amplifier = living.effect(effect).expect("added effect").amplifier;
        add_attribute_modifiers(effect, &mut living.attributes, amplifier);
        if let Some(p) = self.players.get(&id) {
            let instance = p.living.effect(effect).expect("added effect");
            p.connection.send_raw(update_mob_effect(id, instance, true));
        }
    }

    fn on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, _source: Option<EntityId>) {
        let living = &mut self.living_mut(id);
        living.effects_dirty = true;
        if refresh_attributes {
            let amplifier = living.effect(effect).expect("updated effect").amplifier;
            remove_attribute_modifiers(effect, &mut living.attributes);
            add_attribute_modifiers(effect, &mut living.attributes, amplifier);
            self.refresh_dirty_attributes(id);
        }
        if let Some(p) = self.players.get(&id) {
            let instance = p.living.effect(effect).expect("updated effect");
            p.connection.send_raw(update_mob_effect(id, instance, false));
        }
    }

    fn on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        let living = &mut self.living_mut(id);
        living.effects_dirty = true;
        for effect in effects {
            remove_attribute_modifiers(effect.effect, &mut living.attributes);
        }
        self.refresh_dirty_attributes(id);
        let Some(p) = self.players.get(&id) else { return };
        for effect in effects {
            p.connection.send_raw(remove_mob_effect(id, effect.effect));
        }
    }

    pub fn send_active_player_effects(&self, id: EntityId) {
        let p = self.player(id);
        for effect in &p.living.active_effects {
            p.connection.send_raw(update_mob_effect(id, effect, false));
        }
    }

    pub fn tick_effects(&mut self, id: EntityId) {
        let mut index = 0;
        loop {
            let (e, living) = self.entity_and_living(id);
            let Some(instance) = living.active_effects.get(index) else { return };
            let (effect, amplifier, mod_count) = (instance.effect, instance.amplifier, living.effects_mod_count);
            let tick_count = if instance.is_infinite_duration() { e.tick_count } else { instance.duration };
            let mut keep = instance.has_remaining_duration();
            if keep {
                if should_apply_effect_tick_this_tick(effect, tick_count, amplifier) && !self.apply_effect_tick(id, effect, amplifier) {
                    keep = false;
                } else if self.living(id).effects_mod_count == mod_count {
                    let instance = &mut self.living_mut(id).active_effects[index];
                    instance.tick_down_duration();
                    if instance.downgrade_to_hidden_effect() {
                        self.on_effect_updated(id, effect, true, None);
                    }
                    keep = self.living(id).active_effects[index].has_remaining_duration();
                }
            }
            let living = &mut self.living_mut(id);
            if living.effects_mod_count != mod_count {
                return;
            }
            if !keep {
                let removed = living.active_effects.remove(index);
                living.effects_mod_count += 1;
                self.on_effects_removed(id, &[removed]);
                continue;
            }
            if living.active_effects[index].duration % 600 == 0 {
                self.on_effect_updated(id, effect, false, None);
            }
            index += 1;
        }
    }

    pub fn trigger_on_death_mob_effects(&mut self, id: EntityId, reason: RemovalReason) {
        for index in 0..self.living(id).active_effects.len() {
            let effect = self.living(id).active_effects[index].effect;
            self.on_mob_removed(id, effect, reason);
        }
        let living = &mut self.living_mut(id);
        living.active_effects.clear();
        living.effects_mod_count += 1;
    }

    pub fn update_effect_visibility(&mut self, id: EntityId) {
        self.living_mut(id).effects_dirty = true;
    }

    pub fn update_dirty_effects(&mut self, id: EntityId) {
        if !self.living(id).effects_dirty {
            return;
        }
        self.update_invisibility_status(id);
        let glowing = self.is_currently_glowing(id);
        let (e, living) = self.entity_and_living_mut(id);
        if e.shared_flag(flags::GLOWING) != glowing {
            e.set_shared_flag(flags::GLOWING, glowing);
        }
        living.effects_dirty = false;
    }

    fn update_invisibility_status(&mut self, id: EntityId) {
        let spectator = self.is_spectator(id);
        let (e, living) = self.entity_and_living_mut(id);
        let effects = &living.active_effects;
        if spectator || effects.is_empty() {
            e.data.set(ids::EFFECT_PARTICLES, DataValue::Particles(Vec::new()));
            e.set_shared_flag(flags::INVISIBLE, spectator);
        } else {
            let invisible = living.has_effect(mob_effects::INVISIBILITY);
            e.set_shared_flag(flags::INVISIBLE, invisible);
            let particles = effects.iter().filter(|e| e.visible).map(MobEffectInstance::particle_options).collect();
            let ambient = are_all_effects_ambient(effects);
            e.data.set(ids::EFFECT_PARTICLES, DataValue::Particles(particles));
            e.data.set(ids::EFFECT_AMBIENCE, DataValue::Boolean(ambient));
        }
    }

    pub fn is_currently_glowing(&self, id: EntityId) -> bool {
        let (e, living) = self.entity_and_living(id);
        living.has_effect(mob_effects::GLOWING) || e.has_glowing_tag
    }

    pub fn has_water_breathing(&self, id: EntityId) -> bool {
        let living = &self.living(id);
        living.has_effect(mob_effects::WATER_BREATHING) || living.has_effect(mob_effects::CONDUIT_POWER) || living.has_effect(mob_effects::BREATH_OF_THE_NAUTILUS)
    }

    pub fn should_effects_refill_air_supply(&self, id: EntityId) -> bool {
        let living = &self.living(id);
        !living.has_effect(mob_effects::BREATH_OF_THE_NAUTILUS) || living.has_effect(mob_effects::WATER_BREATHING) || living.has_effect(mob_effects::CONDUIT_POWER)
    }
}
