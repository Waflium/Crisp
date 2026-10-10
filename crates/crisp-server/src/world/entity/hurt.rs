use crisp_core::{AABB, BlockPos, Vec3, mth};
use crisp_core::random::RandomSource;
use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_data::packets::play::clientbound;
use crisp_data::{EquipmentSlot, blocks, damage_type_tags, damage_types, entity_type_tags, fluid_tags, game_rule, item_tags, mob_effects};

use super::EntityId;
use super::attributes::{Attribute, AttributeModifier, Operation};
use super::data::{DataValue, ids};
use super::movement::sound_event;
use crate::commands::arguments::selector::entity_selector::java_equals_ignore_case;
use crate::network::chat::Component;
use crate::network::connection::encode;
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::damagesource::DamageSource;
use crate::world::damagesource::combat_rules;
use crate::world::difficulty::Difficulty;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::scores::criteria::ObjectiveCriteria;
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::scores::team::Visibility;

pub mod events {
    pub const DEATH: u8 = 3;
    pub const TALISMAN_ACTIVATE: u8 = 35;
    pub const POOF: u8 = 60;
    pub const DROWN_PARTICLES: u8 = 67;
}

pub fn entity_event_for_equipment_break(slot: EquipmentSlot) -> u8 {
    match slot {
        EquipmentSlot::Mainhand => 47,
        EquipmentSlot::Offhand => 48,
        EquipmentSlot::Head => 49,
        EquipmentSlot::Chest => 50,
        EquipmentSlot::Feet => 52,
        EquipmentSlot::Legs => 51,
        EquipmentSlot::Body => 65,
        EquipmentSlot::Saddle => 68,
    }
}

pub const MAX_AIR_SUPPLY: i32 = 300;

impl ServerLevel {
    pub fn equipment_inventory_slot(&self, id: EntityId, slot: EquipmentSlot) -> Option<usize> {
        match slot {
            EquipmentSlot::Mainhand => Some(self.player(id).inventory.selected_slot()),
            EquipmentSlot::Offhand => Some(crate::world::item::OFFHAND),
            EquipmentSlot::Feet => Some(8),
            EquipmentSlot::Legs => Some(7),
            EquipmentSlot::Chest => Some(6),
            EquipmentSlot::Head => Some(5),
            EquipmentSlot::Body | EquipmentSlot::Saddle => None,
        }
    }

    pub fn item_by_slot(&self, id: EntityId, slot: EquipmentSlot) -> Option<ItemStack> {
        let index = self.equipment_inventory_slot(id, slot)?;
        self.player(id).inventory.slots[index].clone().filter(|s| !s.is_empty())
    }

    pub fn max_health(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MaxHealth) as f32
    }

    pub fn max_absorption(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MaxAbsorption) as f32
    }

    pub fn absorption_amount(&self, id: EntityId) -> f32 {
        match self.player(id).entity.data.get(ids::PLAYER_ABSORPTION) {
            DataValue::Float(v) => *v,
            _ => 0.0,
        }
    }

    pub fn set_absorption_amount(&mut self, id: EntityId, amount: f32) {
        let amount = mth::clamp_f(amount, 0.0, self.max_absorption(id));
        self.player_mut(id).entity.data.set(ids::PLAYER_ABSORPTION, DataValue::Float(amount));
    }

    pub fn set_health(&mut self, id: EntityId, health: f32) {
        let health = mth::clamp_f(health, 0.0, self.max_health(id));
        let p = self.player_mut(id);
        p.health = health;
        p.entity.data.set(ids::HEALTH, DataValue::Float(health));
    }

    pub fn heal(&mut self, id: EntityId, amount: f32) {
        let health = self.player(id).health;
        if health > 0.0 {
            self.set_health(id, health + amount);
        }
    }

    pub fn is_dead_or_dying(&self, id: EntityId) -> bool {
        let p = self.player(id);
        p.health <= 0.0 || p.living.dead
    }

    pub fn armor_value(&self, id: EntityId) -> i32 {
        mth::floor(self.attribute_value(id, Attribute::Armor))
    }

    pub fn is_invulnerable_to(&self, id: EntityId, source: &DamageSource) -> bool {
        let p = self.player(id);
        let base = p.entity.is_removed()
            || source.is(&damage_type_tags::IS_FIRE) && self.fire_immune(id)
            || source.is(&damage_type_tags::IS_FALL) && entity_type_tags::FALL_DAMAGE_IMMUNE.contains(p.entity.entity_type as usize);
        let player = base || {
            let rules = &self.game_rules;
            if source.is(&damage_type_tags::IS_DROWNING) {
                !rules.get_bool(game_rule::DROWNING_DAMAGE)
            } else if source.is(&damage_type_tags::IS_FALL) {
                !rules.get_bool(game_rule::FALL_DAMAGE)
            } else if source.is(&damage_type_tags::IS_FIRE) {
                !rules.get_bool(game_rule::FIRE_DAMAGE)
            } else {
                source.is(&damage_type_tags::IS_FREEZING) && !rules.get_bool(game_rule::FREEZE_DAMAGE)
            }
        };
        player || !p.movement.has_client_loaded()
    }

    pub fn hurt(&mut self, id: EntityId, source: DamageSource, damage: f32) {
        self.hurt_server(id, source, damage);
    }

    fn item_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to_base(id, &source) {
            return false;
        }
        let Some(super::LevelEntity::Item(item)) = self.entities.get_mut(&id) else { return false };
        if !item.item.can_be_hurt_by(&source) {
            return false;
        }
        item.entity.sync_velocity = true;
        item.health = (item.health as f32 - damage) as i32;
        if item.health <= 0 {
            item.entity.discard();
        }
        true
    }

    pub(crate) fn is_invulnerable_to_base(&self, id: EntityId, source: &DamageSource) -> bool {
        self.entity(id).is_removed() || source.is(&damage_type_tags::IS_FIRE) && self.fire_immune(id)
    }

    fn should_play_lava_hurt_sound(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(super::LevelEntity::Item(item)) => item.health <= 0 || item.entity.tick_count % 10 == 0,
            _ => true,
        }
    }

    pub fn hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if let Some(super::LevelEntity::ExperienceOrb(_)) = self.entities.get(&id) {
            return self.experience_orb_hurt_server(id, &source, damage);
        }
        if !self.is_player(id) {
            return self.item_hurt_server(id, source, damage);
        }
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        if let Some(attacker) = source.causing_entity.filter(|&e| self.is_player(e))
            && !self.can_harm_player(id, attacker)
        {
            return false;
        }
        if self.player(id).abilities.invulnerable && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            return false;
        }
        if self.is_dead_or_dying(id) {
            return false;
        }
        let mut damage = damage;
        if source.scales_with_difficulty(self) {
            match self.difficulty {
                Difficulty::Peaceful => damage = 0.0,
                Difficulty::Easy => damage = (damage / 2.0 + 1.0).min(damage),
                Difficulty::Hard => damage = damage * 3.0 / 2.0,
                Difficulty::Normal => {}
            }
        }
        if damage == 0.0 { false } else { self.living_hurt_server(id, source, damage) }
    }

    fn living_hurt_server(&mut self, id: EntityId, source: DamageSource, mut damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) || self.is_dead_or_dying(id) {
            return false;
        }
        if source.is(&damage_type_tags::IS_FIRE) && self.has_effect(id, mob_effects::FIRE_RESISTANCE) {
            return false;
        }
        if damage < 0.0 {
            damage = 0.0;
        }
        let blocked = false;
        if source.is(&damage_type_tags::IS_FREEZING) && entity_type_tags::FREEZE_HURTS_EXTRA_TYPES.contains(self.entity(id).entity_type as usize) {
            damage *= 5.0;
        }
        if source.is(&damage_type_tags::DAMAGES_HELMET) && self.item_by_slot(id, EquipmentSlot::Head).is_some() {
            self.do_hurt_equipment(id, &source, damage, &[EquipmentSlot::Head]);
            damage *= 0.75;
        }
        if !damage.is_finite() {
            damage = f32::MAX;
        }
        let mut took_full_damage = true;
        let living = &self.player(id).living;
        if living.damage_cooldown_time as f32 > 10.0 && !source.is(&damage_type_tags::BYPASSES_COOLDOWN) {
            if damage <= living.last_hurt {
                return false;
            }
            let last_hurt = living.last_hurt;
            self.actually_hurt(id, &source, damage - last_hurt);
            self.player_mut(id).living.last_hurt = damage;
            took_full_damage = false;
        } else {
            let living = &mut self.player_mut(id).living;
            living.last_hurt = damage;
            living.damage_cooldown_time = 20;
            self.actually_hurt(id, &source, damage);
            let living = &mut self.player_mut(id).living;
            living.hurt_duration = 10;
            living.hurt_time = living.hurt_duration;
        }
        if took_full_damage {
            self.broadcast_damage_event(id, &source);
            if !source.is(&damage_type_tags::NO_IMPACT) {
                self.entity_mut(id).sync_velocity = true;
            }
            if !source.is(&damage_type_tags::NO_KNOCKBACK) {
                self.deal_default_knockback(id, &source, blocked);
            }
        }
        if self.is_dead_or_dying(id) {
            if !self.check_totem_death_protection(id, &source) {
                if took_full_damage {
                    self.make_sound(id, sound_event("minecraft:entity.player.death"));
                    self.play_secondary_hurt_sound(id, &source);
                }
                self.die(id, source);
            }
        } else if took_full_damage {
            self.make_sound(id, sound_event(source.ty.info().effects.sound()));
            self.play_secondary_hurt_sound(id, &source);
        }
        let success = !blocked || damage > 0.0;
        if success {
            let game_time = self.game_time;
            let living = &mut self.player_mut(id).living;
            living.last_damage_source = Some(source);
            living.last_damage_stamp = game_time;
        }
        success
    }

    fn actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_invulnerable_to(id, source) {
            return;
        }
        let damage = self.damage_after_armor_absorb(id, source, damage);
        let damage = self.damage_after_magic_absorb(id, source, damage);
        let original_damage = damage;
        let absorption = self.absorption_amount(id);
        let damage = (damage - absorption).max(0.0);
        self.set_absorption_amount(id, absorption - (original_damage - damage));
        if damage != 0.0 {
            self.cause_food_exhaustion(id, source.food_exhaustion());
            self.record_damage(id, *source, damage);
            let health = self.player(id).health;
            self.set_health(id, health - damage);
        }
    }

    pub fn damage_after_armor_absorb(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> f32 {
        if source.is(&damage_type_tags::BYPASSES_ARMOR) {
            return damage;
        }
        self.do_hurt_equipment(id, source, damage, &[EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head]);
        let armor = self.armor_value(id) as f32;
        let toughness = self.attribute_value(id, Attribute::ArmorToughness) as f32;
        combat_rules::damage_after_absorb(damage, armor, toughness)
    }

    pub fn damage_after_magic_absorb(&self, id: EntityId, source: &DamageSource, mut damage: f32) -> f32 {
        if source.is(&damage_type_tags::BYPASSES_EFFECTS) {
            return damage;
        }
        if let Some(resistance) = self.player(id).living.effect(mob_effects::RESISTANCE)
            && !source.is(&damage_type_tags::BYPASSES_RESISTANCE)
        {
            let absorb = 25 - (resistance.amplifier + 1) * 5;
            damage = (damage * absorb as f32 / 25.0).max(0.0);
        }
        if damage <= 0.0 {
            return 0.0;
        }
        damage
    }

    fn do_hurt_equipment(&mut self, id: EntityId, source: &DamageSource, damage: f32, slots: &[EquipmentSlot]) {
        if damage <= 0.0 {
            return;
        }
        let durability_damage = (damage / 4.0).max(1.0) as i32;
        for &slot in slots {
            let Some(index) = self.equipment_inventory_slot(id, slot) else { continue };
            let Some(stack) = self.player(id).inventory.slots[index].clone() else { continue };
            let info = stack.item.info();
            let can_be_hurt_by = info.damage_resistant.is_none_or(|tag| !source.is(tag));
            if info.equipment_slot.is_some() && info.damage_on_hurt && stack.is_damageable_item() && can_be_hurt_by {
                self.hurt_and_break(id, index, durability_damage, slot);
            }
        }
    }

    pub fn hurt_and_break(&mut self, id: EntityId, index: usize, amount: i32, slot: EquipmentSlot) {
        let p = self.player_mut(id);
        if p.abilities.instabuild {
            return;
        }
        let Some(stack) = &mut p.inventory.slots[index] else { return };
        if amount == 0 {
            return;
        }
        let broken = stack.hurt(amount);
        let before = stack.clone();
        p.dirty_slots.push(index);
        if broken {
            p.inventory.slots[index] = None;
            self.broadcast_entity_event(id, entity_event_for_equipment_break(slot));
            self.stop_location_based_effects(id, &before, slot);
        }
    }

    fn broadcast_damage_event(&self, id: EntityId, source: &DamageSource) {
        let packet = encode(clientbound::DAMAGE_EVENT, |p| {
            p.varint(id);
            p.varint(source.ty.0 as i32);
            p.varint(source.causing_entity.map_or(-1, |e| e) + 1);
            p.varint(source.direct_entity.map_or(-1, |e| e) + 1);
            match source.damage_source_position {
                Some(pos) => {
                    p.bool(true);
                    p.f64(pos.x);
                    p.f64(pos.y);
                    p.f64(pos.z);
                }
                None => p.bool(false),
            }
        });
        self.send_to_tracking_players_and_self(id, &packet);
    }

    fn deal_default_knockback(&mut self, id: EntityId, source: &DamageSource, blocked: bool) {
        let (xd, zd) = match source.source_position(self) {
            Some(pos) => {
                let e = self.entity(id);
                (pos.x - e.x(), pos.z - e.z())
            }
            None => (0.0, 0.0),
        };
        self.knockback(id, 0.4f32 as f64, xd, zd);
        if !blocked {
            self.indicate_damage(id, xd, zd);
        }
    }

    pub fn knockback(&mut self, id: EntityId, power: f64, mut xd: f64, mut zd: f64) {
        let power = power * (1.0 - self.attribute_value(id, Attribute::KnockbackResistance));
        if power <= 0.0 {
            return;
        }
        let e = self.entity_mut(id);
        e.needs_sync = true;
        let movement = e.delta_movement;
        while xd * xd + zd * zd < 1.0e-5f32 as f64 {
            xd = (e.random.next_double() - e.random.next_double()) * 0.01;
            zd = (e.random.next_double() - e.random.next_double()) * 0.01;
        }
        let delta = Vec3::new(xd, 0.0, zd).normalize().scale(power);
        let y = if e.on_ground { (movement.y / 2.0 + power).min(0.4) } else { movement.y };
        e.set_delta_movement(Vec3::new(movement.x / 2.0 - delta.x, y, movement.z / 2.0 - delta.z));
    }

    fn indicate_damage(&mut self, id: EntityId, xd: f64, zd: f64) {
        let p = self.player_mut(id);
        let hurt_dir = (mth::atan2(zd, xd) * 180.0f32 as f64 / std::f32::consts::PI as f64 - p.entity.y_rot as f64) as f32;
        p.living.hurt_dir = hurt_dir;
        p.connection.send(clientbound::HURT_ANIMATION, |w| {
            w.varint(id);
            w.f32(hurt_dir);
        });
    }

    fn make_sound(&mut self, id: EntityId, sound: i32) {
        self.entity_play_sound(id, sound, 1.0, 1.0);
    }

    fn play_secondary_hurt_sound(&mut self, id: EntityId, source: &DamageSource) {
        if source.is_type(damage_types::THORNS) {
            let pos = self.entity(id).position();
            self.play_sound_at(None, pos.x, pos.y, pos.z, sound_event("minecraft:enchant.thorns.hit"), crate::server::level::SoundSource::Players, 1.0, 1.0);
        }
    }

    fn check_totem_death_protection(&mut self, id: EntityId, source: &DamageSource) -> bool {
        if source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            return false;
        }
        for slot in [EquipmentSlot::Mainhand, EquipmentSlot::Offhand] {
            let Some(index) = self.equipment_inventory_slot(id, slot) else { continue };
            let Some(stack) = self.player(id).inventory.slots[index].clone() else { continue };
            let Some(death_effects) = stack.item.info().death_protection else { continue };
            let p = self.player_mut(id);
            let mut stack = stack;
            stack.count -= 1;
            p.inventory.slots[index] = Some(stack).filter(|s| s.count > 0);
            p.dirty_slots.push(index);
            self.set_health(id, 1.0);
            for effect in death_effects {
                self.apply_consume_effect(id, effect);
            }
            self.broadcast_entity_event(id, events::TALISMAN_ACTIVATE);
            return true;
        }
        false
    }

    pub fn can_harm_player(&self, id: EntityId, target: EntityId) -> bool {
        self.game_rules.get_bool(game_rule::PVP) && self.do_teams_allow_damage(id, target)
    }

    pub fn die(&mut self, id: EntityId, source: DamageSource) {
        if self.game_rules.get_bool(game_rule::SHOW_DEATH_MESSAGES) {
            let message = self.death_message(id);
            self.player(id).connection.send(clientbound::PLAYER_COMBAT_KILL, |p| {
                p.varint(id);
                message.write(p);
            });
            match self.team_of(id).map(|t| t.death_message_visibility()) {
                None | Some(Visibility::Always) => self.broadcast_system_message(&message),
                Some(Visibility::HideForOtherTeams) => self.broadcast_system_to_team(id, &message),
                Some(Visibility::HideForOwnTeam) => self.broadcast_system_to_all_except_team(id, &message),
                Some(Visibility::Never) => {}
            }
        } else {
            self.player(id).connection.send(clientbound::PLAYER_COMBAT_KILL, |p| {
                p.varint(id);
                Component::empty().write(p);
            });
        }
        if !self.is_spectator(id) {
            self.drop_all_death_loot(id, &source);
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&ObjectiveCriteria::DEATH_COUNT, &holder, |level, score| {
            let _ = score.increment(level);
        });
        self.broadcast_entity_event(id, events::DEATH);
        self.clear_fire(id);
        self.entity_mut(id).set_ticks_frozen(0);
        self.entity_mut(id).set_shared_flag(super::flags::ON_FIRE, false);
        self.recheck_combat_status(id);
        let pos = self.entity(id).block_position();
        let dimension = self.dimension;
        let p = self.player_mut(id);
        p.last_death_location = Some((dimension.to_owned(), pos));
        p.movement.waiting_for_respawn = true;
    }

    fn drop_all_death_loot(&mut self, id: EntityId, source: &DamageSource) {
        self.drop_equipment(id);
        let keep = self.game_rules.get_bool(game_rule::KEEP_INVENTORY) || self.is_spectator(id);
        let base = if keep { 0 } else { self.player(id).experience.level.wrapping_mul(7).min(100) };
        let reward = self.process_mob_experience(source.causing_entity, base);
        let pos = self.entity(id).position();
        self.award_experience(pos, reward);
    }

    fn drop_equipment(&mut self, id: EntityId) {
        if self.game_rules.get_bool(game_rule::KEEP_INVENTORY) {
            return;
        }
        let storage = (crate::world::item::HOTBAR_START..crate::world::item::HOTBAR_START + 9).chain(crate::world::item::MAIN_START..crate::world::item::HOTBAR_START);
        let equipment = [crate::world::item::OFFHAND, 8, 7, 6, 5];
        for index in storage.chain(equipment) {
            let p = self.player_mut(id);
            let Some(stack) = p.inventory.slots[index].take() else { continue };
            p.dirty_slots.push(index);
            if !stack.is_empty() {
                self.drop_from_player(id, stack, true);
            }
        }
    }

    pub fn broadcast_system_message(&self, message: &Component) {
        info!("System chat: {}", message.get_string());
        for p in self.players.values() {
            if p.client.chat_visibility == crate::network::configuration::ChatVisibility::Hidden {
                continue;
            }
            p.connection.send(clientbound::SYSTEM_CHAT, |w| {
                message.write(w);
                w.bool(false);
            });
        }
    }

    fn send_system_message(&self, id: EntityId, message: &Component) {
        let p = self.player(id);
        if p.client.chat_visibility == crate::network::configuration::ChatVisibility::Hidden {
            return;
        }
        p.connection.send(clientbound::SYSTEM_CHAT, |w| {
            message.write(w);
            w.bool(false);
        });
    }

    fn broadcast_system_to_team(&self, player: EntityId, message: &Component) {
        let Some(team) = self.team_of(player) else { return };
        for name in team.players() {
            if let Some(member) = self.players.values().find(|p| java_equals_ignore_case(&p.profile.name, name))
                && member.entity.id != player
            {
                self.send_system_message(member.entity.id, message);
            }
        }
    }

    fn broadcast_system_to_all_except_team(&self, player: EntityId, message: &Component) {
        let Some(team) = self.team_of(player).map(|t| t.name().to_string()) else {
            self.broadcast_system_message(message);
            return;
        };
        for p in self.players.values() {
            if self.team_of(p.entity.id).is_none_or(|t| t.name() != team) {
                self.send_system_message(p.entity.id, message);
            }
        }
    }

    pub fn kill(&mut self, id: EntityId) {
        self.hurt_server(id, DamageSource::new(damage_types::GENERIC_KILL), f32::MAX);
    }

    pub fn set_remaining_fire_ticks(&mut self, id: EntityId, ticks: i32) {
        let invulnerable = self.players.get(&id).is_some_and(|p| p.abilities.invulnerable);
        self.entity_mut(id).remaining_fire_ticks = if invulnerable { ticks.min(1) } else { ticks };
    }

    pub fn clear_fire(&mut self, id: EntityId) {
        let ticks = self.entity(id).remaining_fire_ticks.min(0);
        self.set_remaining_fire_ticks(id, ticks);
    }

    pub fn lava_hurt(&mut self, id: EntityId) {
        if self.fire_immune(id) {
            return;
        }
        if self.hurt_server(id, DamageSource::new(damage_types::LAVA), 4.0) && self.should_play_lava_hurt_sound(id) && !self.entity(id).is_silent() {
            let source = if self.is_player(id) { crate::server::level::SoundSource::Players } else { crate::server::level::SoundSource::Neutral };
            let e = self.entity_mut(id);
            let pitch = 2.0 + e.random.next_float() * 0.4;
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.generic.burn"), source, 0.4, pitch);
        }
    }

    pub fn is_in_wall(&mut self, id: EntityId) -> bool {
        let e = self.entity(id);
        if e.no_physics {
            return false;
        }
        let check_width = (e.dimensions.width * 0.8) as f64;
        let eye = e.eye_position();
        let eye_bb = AABB::new(
            eye.x - check_width / 2.0,
            eye.y - 1.0e-6 / 2.0,
            eye.z - check_width / 2.0,
            eye.x + check_width / 2.0,
            eye.y + 1.0e-6 / 2.0,
            eye.z + check_width / 2.0,
        );
        let eye_shape = VoxelShape::from_boxes(&[eye_bb]);
        let (min, max) = (BlockPos::containing(eye_bb.min_x, eye_bb.min_y, eye_bb.min_z), BlockPos::containing(eye_bb.max_x, eye_bb.max_y, eye_bb.max_z));
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = BlockPos::new(x, y, z);
                    let state = self.block_state(pos);
                    if !state.is_air() && state.is_suffocating() {
                        let shape = state.collision_voxel_shape();
                        if join_is_not_empty(&shape.moved(pos.x as f64, pos.y as f64, pos.z as f64), &eye_shape, BooleanOp::And) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn living_base_tick(&mut self, id: EntityId) {
        if self.is_alive(id) {
            if self.is_in_wall(id) {
                self.hurt_server(id, DamageSource::new(damage_types::IN_WALL), 1.0);
            } else if self.is_player(id) && !self.world_border.is_within_bounds_aabb(&self.entity(id).bounding_box()) {
                let border = &self.world_border;
                let dist = border.get_distance_to_border(self.entity(id).x(), self.entity(id).z()) + border.get_safe_zone();
                let damage_per_block = border.get_damage_per_block();
                if dist < 0.0 && damage_per_block > 0.0 {
                    self.hurt_server(id, DamageSource::new(damage_types::OUTSIDE_BORDER), mth::floor(-dist * damage_per_block).max(1) as f32);
                }
            }
            let e = self.entity(id);
            let eye_pos = BlockPos::containing(e.x(), e.eye_y(), e.z());
            if e.is_eye_in_fluid(&fluid_tags::WATER) && !self.block_state(eye_pos).is_block(blocks::BUBBLE_COLUMN) {
                let p = self.player(id);
                let can_drown = !entity_type_tags::CAN_BREATHE_UNDER_WATER.contains(p.entity.entity_type as usize)
                    && !self.has_water_breathing(id)
                    && !p.abilities.invulnerable;
                if can_drown {
                    let supply = self.decrease_air_supply(id, self.entity(id).air_supply());
                    self.entity_mut(id).set_air_supply(supply);
                    if self.entity(id).air_supply() <= -20 {
                        self.entity_mut(id).set_air_supply(0);
                        self.broadcast_entity_event(id, events::DROWN_PARTICLES);
                        self.hurt_server(id, DamageSource::new(damage_types::DROWN), 2.0);
                    }
                } else if self.entity(id).air_supply() < MAX_AIR_SUPPLY && self.should_effects_refill_air_supply(id) {
                    let supply = (self.entity(id).air_supply() + 4).min(MAX_AIR_SUPPLY);
                    self.entity_mut(id).set_air_supply(supply);
                }
            } else if self.entity(id).air_supply() < MAX_AIR_SUPPLY {
                let supply = (self.entity(id).air_supply() + 4).min(MAX_AIR_SUPPLY);
                self.entity_mut(id).set_air_supply(supply);
            }
        }
        let living = &mut self.player_mut(id).living;
        if living.hurt_time > 0 {
            living.hurt_time -= 1;
        }
        if self.is_dead_or_dying(id) {
            self.tick_death(id);
        }
        self.tick_effects(id);
    }

    fn decrease_air_supply(&mut self, id: EntityId, supply: i32) -> i32 {
        let oxygen_bonus = self.attribute_value(id, Attribute::OxygenBonus);
        if oxygen_bonus > 0.0 && self.entity_mut(id).random.next_double() >= 1.0 / (oxygen_bonus + 1.0) { supply } else { supply - 1 }
    }

    fn tick_death(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        p.living.death_time += 1;
        if p.living.death_time >= 20 && !p.entity.is_removed() {
            self.broadcast_entity_event(id, events::POOF);
            self.remove_player_from_level(id, super::RemovalReason::Killed);
        }
    }

    pub fn remove_player_from_level(&mut self, id: EntityId, reason: super::RemovalReason) {
        if self.entity(id).is_removed() {
            return;
        }
        if matches!(reason, super::RemovalReason::Killed | super::RemovalReason::Discarded) {
            self.trigger_on_death_mob_effects(id, reason);
        }
        self.entity_mut(id).removal = Some(reason);
        self.player_left_level(id);
        self.recheck_combat_status(id);
        let p = self.player_mut(id);
        let mut dropped: Vec<ItemStack> = p.carried.take().into_iter().collect();
        for index in 1..5 {
            if let Some(stack) = p.inventory.slots[index].take() {
                p.dirty_slots.push(index);
                dropped.push(stack);
            }
        }
        for stack in dropped {
            self.player_drop(id, stack, false, Prediction::ServerOnly);
        }
    }

    pub fn tick_regeneration(&mut self, id: EntityId) {
        if self.difficulty != Difficulty::Peaceful || !self.game_rules.get_bool(game_rule::NATURAL_HEALTH_REGENERATION) {
            return;
        }
        let tick_count = self.entity(id).tick_count;
        if tick_count % 20 == 0 {
            if self.player(id).health < self.max_health(id) {
                self.heal(id, 1.0);
            }
            let food = &mut self.player_mut(id).food_data;
            if food.saturation_level < 20.0 {
                food.saturation_level += 1.0;
            }
        }
        let food = &mut self.player_mut(id).food_data;
        if tick_count % 10 == 0 && food.needs_food() {
            food.food_level += 1;
        }
    }

    pub fn can_freeze(&self, id: EntityId) -> bool {
        if self.is_spectator(id) {
            return false;
        }
        for slot in [EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head, EquipmentSlot::Body] {
            if self.item_by_slot(id, slot).is_some_and(|s| s.item.is(&item_tags::FREEZE_IMMUNE_WEARABLES)) {
                return false;
            }
        }
        !entity_type_tags::FREEZE_IMMUNE_ENTITY_TYPES.contains(self.entity(id).entity_type as usize)
    }

    pub fn tick_freezing_damage(&mut self, id: EntityId) {
        let e = self.entity(id);
        if e.tick_count % 40 == 0 && e.ticks_frozen() >= 140 && self.can_freeze(id) {
            self.hurt_server(id, DamageSource::new(damage_types::FREEZE), 1.0);
        }
    }

    pub fn send_health_if_changed(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        let food = p.food_data;
        let saturation_zero = food.saturation_level == 0.0;
        if p.health != p.last_sent_health || p.last_sent_food != food.food_level || saturation_zero != p.last_food_saturation_zero {
            let health = p.health;
            p.connection.send(clientbound::SET_HEALTH, |w| {
                w.f32(health);
                w.varint(food.food_level);
                w.f32(food.saturation_level);
            });
            p.last_sent_health = health;
            p.last_sent_food = food.food_level;
            p.last_food_saturation_zero = saturation_zero;
        }
        self.update_criteria_scores(id);
        let p = self.player_mut(id);
        if p.experience.total != p.last_sent_exp {
            p.last_sent_exp = p.experience.total;
            p.send_experience();
        }
    }

    fn update_criteria_scores(&mut self, id: EntityId) {
        let health_and_absorption = self.player(id).health + self.absorption_amount(id);
        if health_and_absorption != self.player(id).last_recorded.health_and_absorption {
            self.player_mut(id).last_recorded.health_and_absorption = health_and_absorption;
            self.update_score_for_criteria(id, &ObjectiveCriteria::HEALTH, mth::ceil(health_and_absorption as f64));
        }
        let food = self.player(id).food_data.food_level;
        if food != self.player(id).last_recorded.food_level {
            self.player_mut(id).last_recorded.food_level = food;
            self.update_score_for_criteria(id, &ObjectiveCriteria::FOOD, food);
        }
        let air = self.player(id).entity.air_supply();
        if air != self.player(id).last_recorded.air_level {
            self.player_mut(id).last_recorded.air_level = air;
            self.update_score_for_criteria(id, &ObjectiveCriteria::AIR, air);
        }
        let armor = self.armor_value(id);
        if armor != self.player(id).last_recorded.armor {
            self.player_mut(id).last_recorded.armor = armor;
            self.update_score_for_criteria(id, &ObjectiveCriteria::ARMOR, armor);
        }
        let experience = self.player(id).experience;
        if experience.total != self.player(id).last_recorded.experience {
            self.player_mut(id).last_recorded.experience = experience.total;
            self.update_score_for_criteria(id, &ObjectiveCriteria::EXPERIENCE, experience.total);
        }
        if experience.level != self.player(id).last_recorded.level {
            self.player_mut(id).last_recorded.level = experience.level;
            self.update_score_for_criteria(id, &ObjectiveCriteria::LEVEL, experience.level);
        }
    }

    pub fn detect_equipment_updates(&mut self, id: EntityId) {
        let mut changed: Vec<(EquipmentSlot, Option<ItemStack>)> = Vec::new();
        for slot in EquipmentSlot::VALUES {
            let previous = self.player(id).living.last_equipment_items[slot as usize].clone();
            let current = self.item_by_slot(id, slot);
            if previous != current {
                changed.push((slot, current.clone()));
                if let Some(previous) = previous {
                    self.stop_location_based_effects(id, &previous, slot);
                }
            }
        }
        if changed.is_empty() {
            return;
        }
        for (slot, current) in &changed {
            if let Some(current) = current
                && !current.is_broken()
            {
                for_each_modifier(current, *slot, |attribute, modifier| {
                    if let Some(instance) = self.player_mut(id).living.attributes.instance(attribute) {
                        instance.remove_modifier(&modifier.id);
                        instance.add_transient_modifier(modifier);
                    }
                });
            }
        }
        let last = self.player(id).living.last_equipment_items.clone();
        let main = changed.iter().find(|c| c.0 == EquipmentSlot::Mainhand).map(|c| c.1.clone());
        let off = changed.iter().find(|c| c.0 == EquipmentSlot::Offhand).map(|c| c.1.clone());
        if let (Some(main), Some(off)) = (main, off)
            && main == last[EquipmentSlot::Offhand as usize]
            && off == last[EquipmentSlot::Mainhand as usize]
        {
            let packet = encode(clientbound::ENTITY_EVENT, |w| {
                w.i32(id);
                w.u8(55);
            });
            self.send_to_tracking_players(id, &packet);
            changed.retain(|c| c.0 != EquipmentSlot::Mainhand && c.0 != EquipmentSlot::Offhand);
            let living = &mut self.player_mut(id).living;
            living.last_equipment_items[EquipmentSlot::Mainhand as usize] = main;
            living.last_equipment_items[EquipmentSlot::Offhand as usize] = off;
        }
        if changed.is_empty() {
            return;
        }
        let living = &mut self.player_mut(id).living;
        for (slot, current) in &changed {
            living.last_equipment_items[*slot as usize] = current.clone();
        }
        let packet = set_equipment(id, &changed);
        self.send_to_tracking_players(id, &packet);
    }

    fn stop_location_based_effects(&mut self, id: EntityId, previous: &ItemStack, slot: EquipmentSlot) {
        for_each_modifier(previous, slot, |attribute, modifier| {
            if let Some(instance) = self.player_mut(id).living.attributes.get_mut(attribute) {
                instance.remove_modifier(&modifier.id);
            }
        });
    }

    pub fn refresh_dirty_attributes(&mut self, id: EntityId) {
        let updated = self.player_mut(id).living.attributes.take_to_update();
        if updated & 1 << Attribute::MaxHealth as u64 != 0 {
            let max_health = self.max_health(id);
            if self.player(id).health > max_health {
                self.set_health(id, max_health);
            }
        }
        if updated & 1 << Attribute::MaxAbsorption as u64 != 0 {
            let max_absorption = self.max_absorption(id);
            if self.absorption_amount(id) > max_absorption {
                self.set_absorption_amount(id, max_absorption);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_block_particles(&self, state: crisp_data::BlockState, x: f64, y: f64, z: f64, count: i32, dist: (f64, f64, f64), speed: f64) {
        let packet = encode(clientbound::LEVEL_PARTICLES, |p| {
            p.varint(crisp_data::registry_id(crisp_data::registry::PARTICLE_TYPE, "minecraft:block"));
            p.varint(state.id() as i32);
            p.bool(false);
            p.bool(false);
            p.f64(x);
            p.f64(y);
            p.f64(z);
            p.f32(dist.0 as f32);
            p.f32(dist.1 as f32);
            p.f32(dist.2 as f32);
            p.f32(speed as f32);
            p.f32(speed as f32);
            p.f32(speed as f32);
            p.varint(count);
            p.varint(0);
        });
        for player in self.players.values() {
            if player.entity.is_removed() {
                continue;
            }
            let center = Vec3::at_center_of(player.entity.block_position());
            if center.distance_to_sqr(Vec3::new(x, y, z)) < 32.0 * 32.0 {
                player.connection.send_raw(packet.clone());
            }
        }
    }
}

fn for_each_modifier(stack: &ItemStack, slot: EquipmentSlot, mut f: impl FnMut(Attribute, AttributeModifier)) {
    for m in stack.item.info().attribute_modifiers {
        if !m.slot.test(slot) {
            continue;
        }
        if let Some(attribute) = Attribute::by_name(m.attribute) {
            f(attribute, AttributeModifier { id: std::borrow::Cow::Borrowed(m.id), amount: m.amount, operation: Operation::by_id(m.operation) });
        }
    }
}

pub fn set_equipment(id: EntityId, slots: &[(EquipmentSlot, Option<ItemStack>)]) -> Vec<u8> {
    encode(clientbound::SET_EQUIPMENT, |p| {
        p.varint(id);
        for (i, (slot, stack)) in slots.iter().enumerate() {
            let last = i == slots.len() - 1;
            p.u8(*slot as u8 | if last { 0 } else { 0x80 });
            crate::world::item::write_slot(p, stack.as_ref());
        }
    })
}
