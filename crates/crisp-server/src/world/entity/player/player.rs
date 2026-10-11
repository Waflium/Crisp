use crisp_core::{Axis, BlockPos, Vec3, mth};
use crisp_data::{EquipmentSlot, damage_type_tags, game_rule};

use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::data::Pose;
use crate::world::entity::living_entity::entity_event_for_equipment_break;
use crate::world::entity::{EntityDimensions, EntityId, RemovalReason};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;

pub fn get_default_dimensions(pose: Pose) -> EntityDimensions {
    match pose {
        Pose::Sleeping => EntityDimensions { width: 0.2, height: 0.2, eye_height: 0.2 },
        Pose::FallFlying | Pose::Swimming | Pose::SpinAttack => EntityDimensions { width: 0.6, height: 0.6, eye_height: 0.4 },
        Pose::Crouching => EntityDimensions { width: 0.6, height: 1.5, eye_height: 1.27 },
        Pose::Dying => EntityDimensions { width: 0.2, height: 0.2, eye_height: 1.62 },
        _ => EntityDimensions { width: 0.6, height: 1.8, eye_height: 1.62 },
    }
}

impl ServerLevel {
    pub fn player_tick(&mut self, id: EntityId) {
        let spectator = self.is_spectator(id);
        self.entity_mut(id).no_physics = spectator;
        if spectator {
            self.set_on_ground(id, false);
        }
        let experience = &mut self.player_mut(id).experience;
        if experience.take_xp_delay > 0 {
            experience.take_xp_delay -= 1;
        }
        self.living_entity_tick(id);
        let e = self.entity_mut(id);
        let nx = mth::clamp(e.x(), -2.9999999e7, 2.9999999e7);
        let nz = mth::clamp(e.z(), -2.9999999e7, 2.9999999e7);
        if nx != e.x() || nz != e.z() {
            let y = e.y();
            e.set_pos(Vec3::new(nx, y, nz));
        }
        self.update_player_pose(id);
    }

    pub fn player_ai_step(&mut self, id: EntityId) {
        self.tick_regeneration(id);
        if self.player(id).abilities_flying() {
            self.reset_fall_distance(id);
        }
        self.living_entity_ai_step(id);
        let speed = self.attribute_value(id, crate::world::entity::attributes::Attribute::MovementSpeed) as f32;
        let p = self.player_mut(id);
        p.entity.y_head_rot = p.entity.y_rot;
        p.living.speed = speed;
        if p.health > 0.0 && !self.is_spectator(id) {
            self.pick_up_items(id);
            self.touch_experience_orbs(id);
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

    pub fn player_travel(&mut self, id: EntityId, input: Vec3) {
        let e = self.entity(id);
        if e.is_swimming() {
            let look_y = e.look_angle().y;
            let multiplier = if look_y < -0.2 { 0.085 } else { 0.06 };
            let above = BlockPos::containing(e.x(), e.y() + 1.0 - 0.1, e.z());
            if look_y <= 0.0 || self.living(id).jumping || !self.fluid_state(above).is_empty() {
                let e = self.entity_mut(id);
                let m = e.delta_movement;
                e.set_delta_movement(m.add(0.0, (look_y - m.y) * multiplier, 0.0));
            }
        }
        if self.player(id).abilities_flying() {
            let original_y = self.entity(id).delta_movement.y;
            self.living_entity_travel(id, input);
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.with(Axis::Y, original_y * 0.6));
        } else {
            self.living_entity_travel(id, input);
        }
    }

    pub fn player_get_flying_speed(&self, id: EntityId) -> f32 {
        let p = self.player(id);
        let sprinting = p.entity.is_sprinting();
        if p.abilities_flying() {
            let flying_speed = 0.05f32;
            if sprinting { flying_speed * 2.0 } else { flying_speed }
        } else if sprinting {
            0.025999999
        } else {
            0.02
        }
    }

    pub fn player_get_speed(&self, id: EntityId) -> f32 {
        self.attribute_value(id, crate::world::entity::attributes::Attribute::MovementSpeed) as f32
    }

    pub fn player_hurt_server(&mut self, id: EntityId, source: DamageSource, mut damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        if self.player(id).abilities.invulnerable && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            return false;
        }
        self.living_mut(id).no_action_time = 0;
        if self.is_dead_or_dying(id) {
            return false;
        }
        if source.scales_with_difficulty(self) {
            match self.difficulty {
                Difficulty::Peaceful => damage = 0.0,
                Difficulty::Easy => damage = (damage / 2.0 + 1.0).min(damage),
                Difficulty::Hard => damage = damage * 3.0 / 2.0,
                Difficulty::Normal => {}
            }
        }
        if damage == 0.0 { false } else { self.living_entity_hurt_server(id, source, damage) }
    }

    pub fn player_is_invulnerable_to(&self, id: EntityId, source: &DamageSource) -> bool {
        if self.is_invulnerable_to_base(id, source) {
            return true;
        }
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
    }

    pub fn player_actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
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
            let health = self.health(id);
            self.set_health(id, health - damage);
        }
    }

    pub fn player_set_remaining_fire_ticks(&mut self, id: EntityId, ticks: i32) {
        let invulnerable = self.player(id).abilities.invulnerable;
        self.entity_mut(id).remaining_fire_ticks = if invulnerable { ticks.min(1) } else { ticks };
    }

    pub fn player_drop_equipment(&mut self, id: EntityId) {
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

    pub fn player_get_base_experience_reward(&self, id: EntityId) -> i32 {
        if !self.game_rules.get_bool(game_rule::KEEP_INVENTORY) && !self.is_spectator(id) { self.player(id).experience.level.wrapping_mul(7).min(100) } else { 0 }
    }

    pub fn player_remove(&mut self, id: EntityId, reason: RemovalReason) {
        if self.entity(id).is_removed() {
            return;
        }
        self.living_entity_remove(id, reason);
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

    pub fn player_drop(&mut self, player: EntityId, stack: ItemStack, thrown_from_hand: bool, prediction: Prediction) {
        if stack.is_empty() {
            return;
        }
        self.swing(player, crate::world::InteractionHand::MainHand, crate::world::item::component::swing_animation::SwingAnimation::DEFAULT, prediction != Prediction::Predicted);
        let item = self.create_item_stack_to_drop(player, stack, false, thrown_from_hand);
        self.add_fresh_entity(crate::world::entity::LevelEntity::Item(item));
    }

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

    fn update_player_pose(&mut self, id: EntityId) {
        if !self.can_player_fit_within_blocks_and_entities_when(id, Pose::Swimming) {
            return;
        }
        let p = self.player(id);
        let e = &p.entity;
        let desired = if e.is_swimming() {
            Pose::Swimming
        } else if e.is_fall_flying() {
            Pose::FallFlying
        } else if e.is_shift_key_down() && !p.abilities_flying() {
            Pose::Crouching
        } else {
            Pose::Standing
        };
        let actual = if self.is_spectator(id) || self.can_player_fit_within_blocks_and_entities_when(id, desired) {
            desired
        } else if self.can_player_fit_within_blocks_and_entities_when(id, Pose::Crouching) {
            Pose::Crouching
        } else {
            Pose::Swimming
        };
        self.set_player_pose(id, actual);
    }

    pub fn set_player_pose(&mut self, id: EntityId, pose: Pose) {
        let e = self.entity_mut(id);
        if e.pose() != pose {
            e.set_pose(pose);
            e.dimensions = get_default_dimensions(pose);
            e.reapply_position();
        }
    }

    fn can_player_fit_within_blocks_and_entities_when(&self, id: EntityId, pose: Pose) -> bool {
        let box_ = get_default_dimensions(pose).make_bounding_box(self.entity(id).position()).deflate_all(1.0e-7);
        self.no_collision_for(id, &box_)
    }
}
