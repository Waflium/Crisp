use crisp_core::random::RandomSource;
use crisp_data::packets::play::clientbound;
use crisp_data::{ConsumeEffect, Consumable, EquipmentSlot, ItemUseAnimation};

use super::EntityId;
use super::data::{DataValue, ids};
use super::sound_event;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::Prediction;
use crate::world::effect::MobEffectInstance;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values::ItemStackTemplate};
use crate::world::{InteractionHand, InteractionResult};

const FLAG_IS_USING: i8 = 1;
const FLAG_OFF_HAND: i8 = 2;

const EVENT_USE_ITEM_COMPLETE: u8 = 9;

fn random_between(random: &mut impl RandomSource, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

fn triangle(random: &mut impl RandomSource, mean: f32, spread: f32) -> f32 {
    mean + spread * (random.next_float() - random.next_float())
}

pub fn use_duration(stack: &ItemStack) -> i32 {
    stack.item.info().consumable.as_ref().map_or(0, Consumable::consume_ticks)
}

impl ServerLevel {
    fn living_entity_flags(&self, id: EntityId) -> i8 {
        match self.entity(id).data.get(ids::LIVING_ENTITY_FLAGS) {
            DataValue::Byte(b) => *b,
            _ => 0,
        }
    }

    fn set_living_entity_flag(&mut self, id: EntityId, flag: i8, value: bool) {
        let flags = self.living_entity_flags(id);
        let flags = if value { flags | flag } else { flags & !flag };
        self.entity_mut(id).data.set(ids::LIVING_ENTITY_FLAGS, DataValue::Byte(flags));
    }

    pub fn is_using_item(&self, id: EntityId) -> bool {
        self.living_entity_flags(id) & FLAG_IS_USING > 0
    }

    pub fn used_item_hand(&self, id: EntityId) -> InteractionHand {
        if self.living_entity_flags(id) & FLAG_OFF_HAND > 0 { InteractionHand::OffHand } else { InteractionHand::MainHand }
    }

    pub fn start_consuming(&mut self, id: EntityId, hand: InteractionHand, stack: ItemStack, consumable: &'static Consumable) -> (InteractionResult, Option<ItemStack>) {
        let can_consume = match stack.item.info().food {
            Some(food) => self.can_eat(id, food.can_always_eat),
            None => true,
        };
        if !can_consume {
            return (InteractionResult::Fail, None);
        }
        if consumable.consume_ticks() > 0 {
            self.start_using_item(id, hand);
            (InteractionResult::CONSUME, None)
        } else {
            let result = self.on_consume(id, stack, consumable);
            (InteractionResult::CONSUME, Some(result))
        }
    }

    pub fn start_using_item(&mut self, id: EntityId, hand: InteractionHand) {
        let Some(stack) = self.player(id).item_in_hand(hand).filter(|s| !s.is_empty()) else { return };
        if self.is_using_item(id) {
            return;
        }
        let living = &mut self.player_mut(id).living;
        living.use_item = Some(stack.clone());
        living.use_item_remaining = crate::world::item::get_use_duration(&stack);
        let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
        let packet = super::living_entity::set_equipment(id, &[(slot, Some(stack))]);
        self.send_to_tracking_players(id, &packet);
        self.set_living_entity_flag(id, FLAG_IS_USING, true);
        self.set_living_entity_flag(id, FLAG_OFF_HAND, hand == InteractionHand::OffHand);
    }

    pub fn updating_using_item(&mut self, id: EntityId) {
        if !self.is_using_item(id) {
            return;
        }
        let hand = self.used_item_hand(id);
        let in_hand = self.player(id).item_in_hand(hand);
        let use_item = self.player(id).living.use_item.clone();
        if in_hand.is_some() && in_hand.as_ref().map(|s| s.item) == use_item.as_ref().map(|s| s.item) {
            self.player_mut(id).living.use_item = in_hand.clone();
            self.update_using_item(id, in_hand.expect("item in hand"));
        } else {
            self.stop_using_item(id);
        }
    }

    fn update_using_item(&mut self, id: EntityId, mut stack: ItemStack) {
        crate::advancements::triggers::criteria_triggers::USING_ITEM.trigger(self, id, &stack);
        let remaining = self.player(id).living.use_item_remaining;
        if let Some(consumable) = &stack.item.info().consumable
            && consumable.should_emit_particles_and_sounds(remaining)
        {
            self.emit_particles_and_sounds(id, consumable, 5);
        }
        let before = stack.clone();
        crate::world::item::on_use_tick(self, id, &mut stack, remaining);
        if stack != before {
            let hand = self.used_item_hand(id);
            self.player_mut(id).set_item_in_hand(hand, Some(stack.clone()));
            self.player_mut(id).living.use_item = Some(stack.clone());
        }
        let living = &mut self.player_mut(id).living;
        living.use_item_remaining -= 1;
        if living.use_item_remaining == 0 && !crate::world::item::use_on_release(&stack) {
            self.complete_using_item(id);
        }
    }

    fn complete_using_item(&mut self, id: EntityId) {
        let Some(use_item) = self.player(id).living.use_item.clone().filter(|s| !s.is_empty()) else { return };
        if !self.is_using_item(id) {
            return;
        }
        self.player(id).connection.send(clientbound::ENTITY_EVENT, |w| {
            w.i32(id);
            w.u8(EVENT_USE_ITEM_COMPLETE);
        });
        let hand = self.used_item_hand(id);
        if Some(&use_item) != self.player(id).item_in_hand(hand).as_ref() {
            self.release_using_item(id);
            return;
        }
        let result = self.finish_using_item(id, use_item.clone());
        if result != use_item {
            self.player_mut(id).set_item_in_hand(hand, Some(result));
        }
        self.stop_using_item(id);
    }

    fn finish_using_item(&mut self, id: EntityId, stack: ItemStack) -> ItemStack {
        let before = stack.clone();
        let result = match &stack.item.info().consumable {
            Some(consumable) => self.on_consume(id, stack, consumable),
            None => stack,
        };
        self.apply_after_use_component_side_effects(id, result, before)
    }

    pub fn apply_after_use_component_side_effects(&mut self, id: EntityId, result: ItemStack, before: ItemStack) -> ItemStack {
        let result = match before.get(dc::USE_REMAINDER).map(ItemStackTemplate::from_nbt) {
            Some(remainder) => self.convert_into_remainder(id, remainder, result, &before),
            None => result,
        };
        if let Some(use_cooldown) = before.get(dc::USE_COOLDOWN)
            && let Some(player) = self.players.get_mut(&id)
        {
            let ticks = crate::world::item::item_cooldowns::use_cooldown_ticks(use_cooldown);
            player.cooldowns.add_cooldown(&before, ticks, &player.connection);
        }
        result
    }

    fn convert_into_remainder(&mut self, id: EntityId, remainder: ItemStackTemplate, result: ItemStack, before: &ItemStack) -> ItemStack {
        if self.player(id).abilities.instabuild || result.count >= before.count {
            return result;
        }
        let remainder = remainder.create().unwrap_or_else(|| ItemStack::new(crisp_data::items::AIR, 0));
        if result.is_empty() {
            return remainder;
        }
        if !remainder.is_empty() {
            let left = self.player_mut(id).pick_up(remainder.clone());
            if left > 0 {
                self.player_drop(id, remainder.copy_with_count(left), false, Prediction::Predicted);
            }
        }
        result
    }

    pub fn release_using_item(&mut self, id: EntityId) {
        let hand = self.used_item_hand(id);
        let in_hand = self.player(id).item_in_hand(hand);
        let use_item = self.player(id).living.use_item.clone();
        if use_item.as_ref().is_some_and(|s| !s.is_empty())
            && let Some(in_hand) = in_hand.filter(|in_hand| Some(in_hand.item) == use_item.as_ref().map(|s| s.item))
        {
            self.player_mut(id).living.use_item = Some(in_hand.clone());
            let remaining = self.player(id).living.use_item_remaining;
            if crate::world::item::release_using(self, &in_hand, id, remaining) {
                let current = self.player(id).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
                let with_side_effects = self.apply_after_use_component_side_effects(id, current.clone(), in_hand.clone());
                if with_side_effects != current {
                    self.player_mut(id).set_item_in_hand(hand, Some(with_side_effects));
                }
            }
            if crate::world::item::use_on_release(&in_hand) {
                self.updating_using_item(id);
            }
        }
        self.stop_using_item(id);
    }

    pub fn stop_using_item(&mut self, id: EntityId) {
        self.set_living_entity_flag(id, FLAG_IS_USING, false);
        let living = &mut self.player_mut(id).living;
        living.use_item = None;
        living.use_item_remaining = 0;
    }

    fn on_consume(&mut self, id: EntityId, mut stack: ItemStack, consumable: &Consumable) -> ItemStack {
        self.emit_particles_and_sounds(id, consumable, 16);
        self.award_stat(id, crate::stats::stats::ITEM_USED.get(stack.get_item().0), 1);
        if self.is_player(id) {
            crate::advancements::triggers::criteria_triggers::CONSUME_ITEM.trigger(self, id, &stack);
        }
        if let Some(food) = stack.item.info().food {
            let e = self.entity_mut(id);
            let pitch = triangle(&mut e.random, 1.0, 0.4);
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event(consumable.sound), SoundSource::Neutral, 1.0, pitch);
            self.player_mut(id).food_data.eat(&food);
            let e = self.entity_mut(id);
            let pitch = random_between(&mut e.random, 0.9, 1.0);
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.player.burp"), SoundSource::Players, 0.5, pitch);
        }
        for effect in consumable.on_consume_effects {
            self.apply_consume_effect(id, effect);
        }
        if !self.player(id).abilities.instabuild {
            stack.count -= 1;
        }
        stack
    }

    pub fn apply_consume_effect(&mut self, id: EntityId, effect: &ConsumeEffect) {
        match effect {
            ConsumeEffect::ApplyEffects { effects, probability } => {
                if self.entity_mut(id).random.next_float() >= *probability {
                    return;
                }
                for effect in effects.iter() {
                    self.add_effect(id, MobEffectInstance::from_template(effect), None);
                }
            }
            ConsumeEffect::RemoveEffects { effects } => {
                for &effect in effects.iter() {
                    self.remove_effect(id, effect);
                }
            }
            ConsumeEffect::ClearAllEffects => {
                self.remove_all_effects(id);
            }
            ConsumeEffect::PlaySound { sound } => {
                let e = self.entity(id);
                let (x, y, z) = (e.x(), e.y(), e.z());
                self.play_sound_at(None, x, y, z, sound_event(sound), SoundSource::Neutral, 1.0, 1.0);
            }
            ConsumeEffect::TeleportRandomly { .. } => {}
        }
    }

    fn emit_particles_and_sounds(&mut self, id: EntityId, consumable: &Consumable, particle_count: i32) {
        let e = self.entity_mut(id);
        let random = &mut e.random;
        let eat_volume = if random.next_boolean() { 0.5 } else { 1.0 };
        let eat_pitch = triangle(random, 1.0, 0.2);
        let drink_pitch = random_between(random, 0.9, 1.0);
        let drink = consumable.animation == ItemUseAnimation::Drink;
        let (volume, pitch) = if drink { (0.5, drink_pitch) } else { (eat_volume, eat_pitch) };
        if consumable.has_consume_particles {
            for _ in 0..particle_count {
                for _ in 0..4 {
                    random.next_float();
                }
            }
        }
        self.entity_play_sound(id, sound_event(consumable.sound), volume, pitch);
    }
}
