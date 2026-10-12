use crisp_core::{AABB, Axis, BlockPos, Vec3, mth};
use crisp_data::{DamageType, EquipmentSlot, damage_type_tags, damage_types, game_rule, item_tags, mob_effects};
use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::server::level::server_entity::set_entity_motion;
use crate::util::Prediction;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityDimensions, EntityId, RemovalReason, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, codec as component_codec};
use crate::world::level::BlockGetter;
use crate::world::level::block::abstract_bed_block::{self, BedKind};
use crate::world::attribute::bed_rule::BedRule;
use crate::network::chat::Component;
use crate::world::{InteractionHand, InteractionResult};

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
        if self.is_sleeping(id) {
            let p = self.player_mut(id);
            p.sleep_counter = (p.sleep_counter + 1).min(100);
            let in_block_state = self.in_block_state(id);
            if let Some(kind) = abstract_bed_block::kind(in_block_state.block()) {
                let pos = self.entity(id).block_position();
                if !abstract_bed_block::get_bed_rule(kind, self, pos).can_sleep(self) {
                    self.stop_sleep_in_bed(id, false, true);
                }
            }
        } else if self.player(id).sleep_counter > 0 {
            let p = self.player_mut(id);
            p.sleep_counter += 1;
            if p.sleep_counter >= 110 {
                p.sleep_counter = 0;
            }
        }
        self.living_entity_tick(id);
        let e = self.entity_mut(id);
        let nx = mth::clamp(e.x(), -2.9999999e7, 2.9999999e7);
        let nz = mth::clamp(e.z(), -2.9999999e7, 2.9999999e7);
        if nx != e.x() || nz != e.z() {
            let y = e.y();
            e.set_pos(Vec3::new(nx, y, nz));
        }
        let main_hand_item_stack = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        let p = self.player_mut(id);
        p.attack_strength_ticker += 1;
        p.item_swap_ticker += 1;
        if !ItemStack::matches(&p.last_item_in_main_hand, &main_hand_item_stack) {
            if !ItemStack::is_same_item(&p.last_item_in_main_hand, &main_hand_item_stack) {
                self.reset_attack_strength_ticker(id);
            }
            self.player_mut(id).last_item_in_main_hand = main_hand_item_stack.copy();
        }
        self.update_player_pose(id);
    }

    pub fn get_current_item_attack_strength_delay(&self, id: EntityId) -> f32 {
        (1.0 / self.attribute_value(id, Attribute::AttackSpeed) * 20.0) as f32
    }

    pub fn cannot_attack_with_item(&self, id: EntityId, item_stack: &ItemStack, tolerance: i32) -> bool {
        let required_strength = item_stack.get(dc::MINIMUM_ATTACK_CHARGE).map_or(0.0, component_codec::as_f32);
        let optimistic_strength = (self.player(id).attack_strength_ticker + tolerance) as f32 / self.get_current_item_attack_strength_delay(id);
        required_strength > 0.0 && optimistic_strength < required_strength
    }

    pub fn get_attack_strength_scale(&self, id: EntityId, a: f32) -> f32 {
        mth::clamp_f((self.player(id).attack_strength_ticker as f32 + a) / self.get_current_item_attack_strength_delay(id), 0.0, 1.0)
    }

    pub fn reset_attack_strength_ticker(&mut self, id: EntityId) {
        let p = self.player_mut(id);
        p.attack_strength_ticker = 0;
        p.item_swap_ticker = 0;
    }

    fn reset_only_attack_strength_ticker(&mut self, id: EntityId) {
        self.player_mut(id).attack_strength_ticker = 0;
    }

    pub fn has_infinite_materials(&self, id: EntityId) -> bool {
        self.players.get(&id).is_some_and(|p| p.abilities.instabuild)
    }

    pub fn can_use_game_master_blocks(&self, id: EntityId) -> bool {
        self.players.get(&id).is_some_and(|p| p.abilities.instabuild && self.spawn_protection.ops.contains(&p.profile.uuid))
    }

    pub fn interact_on(&mut self, id: EntityId, entity: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        if self.is_spectator(id) {
            return InteractionResult::Pass;
        }
        let item_stack_clone = self.player(id).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let interact = self.interact(entity, id, hand, location);
        if interact.consumes_action() {
            let item_stack = self.player(id).item_in_hand(hand);
            let count = item_stack.as_ref().map_or(0, |s| s.count);
            if self.has_infinite_materials(id)
                && item_stack.is_none_or(|s| ItemStack::is_same_item_same_components(&s, &item_stack_clone))
                && count < item_stack_clone.count
            {
                self.player_mut(id).set_item_in_hand(hand, Some(item_stack_clone));
            }
            return interact;
        }
        let mut item_stack = self.player(id).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is_empty() && self.is_living(entity) {
            let has_infinite_materials = self.has_infinite_materials(id);
            if has_infinite_materials {
                item_stack = item_stack_clone;
            }
            let interaction_result = crate::world::item::interact_living_entity(self, &mut item_stack, id, entity, hand);
            if !has_infinite_materials {
                self.player_mut(id).update_item_in_hand(hand, &item_stack);
            }
            if interaction_result.consumes_action() {
                return interaction_result;
            }
        }
        InteractionResult::Pass
    }

    pub fn is_within_attack_range(&self, id: EntityId, weapon_item: &ItemStack, aabb: &AABB, buffer: f64) -> bool {
        self.get_attack_range_with(id, weapon_item).is_in_range(self, id, aabb, buffer)
    }

    fn play_server_side_sound(&mut self, id: EntityId, sound: &str) {
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let source = self.sound_source(id);
        self.play_sound_at(None, x, y, z, sound_event(sound), source, 1.0, 1.0);
    }

    fn create_attack_source(&self, id: EntityId, attacking_item_stack: &ItemStack) -> DamageSource {
        let ty = attacking_item_stack.get(dc::DAMAGE_TYPE).and_then(Nbt::as_str).and_then(DamageType::by_name).unwrap_or(damage_types::PLAYER_ATTACK);
        DamageSource { causing_entity: Some(id), direct_entity: Some(id), ..DamageSource::new(ty) }
    }

    fn cannot_attack(&self, entity: EntityId) -> bool {
        !self.is_attackable(entity)
    }

    fn base_damage_scale_factor(&self, id: EntityId) -> f32 {
        let attack_strength_scale = self.get_attack_strength_scale(id, 0.5);
        0.2 + attack_strength_scale * attack_strength_scale * 0.8
    }

    pub fn player_attack(&mut self, id: EntityId, entity: EntityId) {
        if self.cannot_attack(entity) {
            return;
        }
        let mut base_damage = self.attribute_value(id, Attribute::AttackDamage) as f32;
        let attacking_item_stack = self.get_weapon_item(id);
        let damage_source = self.create_attack_source(id, &attacking_item_stack);
        let attack_strength_scale = self.get_attack_strength_scale(id, 0.5);
        let magic_boost = attack_strength_scale * (self.get_enchanted_damage(id, entity, base_damage, &damage_source) - base_damage);
        base_damage *= self.base_damage_scale_factor(id);
        self.reset_only_attack_strength_ticker(id);
        if base_damage > 0.0 || magic_boost > 0.0 {
            let full_strength_attack = attack_strength_scale > 0.9;
            let knockback_attack = if self.entity(id).is_sprinting() && full_strength_attack {
                self.play_server_side_sound(id, "minecraft:entity.player.attack.knockback");
                true
            } else {
                false
            };
            let critical_attack = full_strength_attack && self.can_critical_attack(id, entity);
            if critical_attack {
                base_damage *= 1.5;
            }
            let total_damage = base_damage + magic_boost;
            let sweep_attack = self.is_sweep_attack(id, full_strength_attack, critical_attack, knockback_attack);
            let old_living_entity_health = if self.is_living(entity) { self.health(entity) } else { 0.0 };
            let old_movement = self.entity(entity).delta_movement;
            let was_hurt = self.hurt_server(entity, damage_source, total_damage);
            if was_hurt {
                let knockback = self.get_knockback(id, entity, &damage_source) + if knockback_attack { 0.5 } else { 0.0 };
                self.cause_extra_knockback(id, entity, knockback, old_movement);
                if sweep_attack {
                    self.do_sweep_attack(id, entity, base_damage, &damage_source, attack_strength_scale);
                }
                self.attack_visual_effects(id, entity, critical_attack, sweep_attack, full_strength_attack, false, magic_boost);
                self.set_last_hurt_mob(id, entity);
                self.item_attack_interaction(id, entity, &attacking_item_stack, &damage_source, true);
                self.damage_stats_and_hearts(entity, old_living_entity_health);
                self.cause_food_exhaustion(id, 0.1);
            } else {
                self.play_server_side_sound(id, "minecraft:entity.player.attack.nodamage");
            }
        }
        self.post_piercing_attack(id);
    }

    fn can_critical_attack(&mut self, id: EntityId, entity: EntityId) -> bool {
        let e = self.entity(id);
        e.fall_distance > 0.0
            && !e.on_ground
            && !self.on_climbable(id)
            && !self.entity(id).is_in_water()
            && !self.has_effect(id, mob_effects::BLINDNESS)
            && self.is_living(entity)
            && !self.entity(id).is_sprinting()
    }

    fn is_sweep_attack(&self, id: EntityId, full_strength_attack: bool, critical_attack: bool, knockback_attack: bool) -> bool {
        if full_strength_attack && !critical_attack && !knockback_attack && self.entity(id).on_ground {
            let approximate_speed_sq = self.known_movement(id).horizontal_distance_sqr();
            let max_speed_for_sweep_attack = self.get_speed(id) as f64 * 2.5;
            if approximate_speed_sq < mth::square(max_speed_for_sweep_attack) {
                return self.item_by_slot(id, EquipmentSlot::Mainhand).is_some_and(|stack| stack.item.is(&item_tags::SWORDS));
            }
        }
        false
    }

    fn attack_visual_effects(&mut self, id: EntityId, entity: EntityId, critical_attack: bool, sweep_attack: bool, full_strength_attack: bool, stab_attack: bool, magic_boost: f32) {
        if critical_attack {
            self.play_server_side_sound(id, "minecraft:entity.player.attack.crit");
            self.crit(id, entity);
        }
        if !critical_attack && !sweep_attack && !stab_attack {
            let sound = if full_strength_attack { "minecraft:entity.player.attack.strong" } else { "minecraft:entity.player.attack.weak" };
            self.play_server_side_sound(id, sound);
        }
        if magic_boost > 0.0 {
            self.magic_crit(id, entity);
        }
    }

    fn damage_stats_and_hearts(&mut self, entity: EntityId, old_living_entity_health: f32) {
        if self.is_living(entity) {
            let actual_damage = old_living_entity_health - self.health(entity);
            if actual_damage > 2.0 {
                let count = (actual_damage as f64 * 0.5) as i32;
                let e = self.entity(entity);
                let (x, y, z) = (e.x(), e.y() + e.dimensions.height as f64 * 0.5, e.z());
                self.send_particles("minecraft:damage_indicator", |_| {}, x, y, z, count, (0.1, 0.0, 0.1), 0.2);
            }
        }
    }

    fn item_attack_interaction(&mut self, id: EntityId, entity: EntityId, attacking_item_stack: &ItemStack, damage_source: &DamageSource, apply_to_target: bool) {
        let hurt_target = entity;
        let item_hurt_enemy = self.is_living(hurt_target) && attacking_item_stack.has(dc::WEAPON);
        if apply_to_target {
            self.do_post_attack_effects_with_item_source(entity, damage_source, Some(attacking_item_stack));
        }
        if !attacking_item_stack.is_empty() && self.is_living(hurt_target) && item_hurt_enemy {
            let item_damage_per_attack = attacking_item_stack.get(dc::WEAPON).and_then(|weapon| weapon.get("item_damage_per_attack")).map_or(1, component_codec::as_i32);
            self.hurt_and_break(id, EquipmentSlot::Mainhand, item_damage_per_attack);
        }
    }

    fn cause_extra_knockback(&mut self, id: EntityId, entity: EntityId, knockback_amount: f32, old_movement: Vec3) {
        if knockback_amount > 0.0 {
            let y_rot = self.entity(id).y_rot * mth::DEG_TO_RAD;
            let (sin, cos) = (mth::sin(y_rot as f64) as f64, mth::cos(y_rot as f64) as f64);
            if self.is_living(entity) {
                self.knockback(entity, knockback_amount as f64, sin, -cos);
            } else {
                self.entity_mut(entity).push(-sin * knockback_amount as f64, 0.1, cos * knockback_amount as f64);
            }
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.multiply(0.6, 1.0, 0.6));
            self.set_sprinting(id, false);
        }
        if let Some(target) = self.players.get_mut(&entity)
            && target.entity.sync_velocity
        {
            let packet = set_entity_motion(entity, target.entity.delta_movement);
            target.connection.send_raw(packet);
            target.entity.sync_velocity = false;
            target.entity.set_delta_movement(old_movement);
        }
    }

    fn do_sweep_attack(&mut self, id: EntityId, entity: EntityId, base_damage: f32, damage_source: &DamageSource, attack_strength_scale: f32) {
        self.play_server_side_sound(id, "minecraft:entity.player.attack.sweep");
        let sweep_damage = 1.0 + self.attribute_value(id, Attribute::SweepingDamageRatio) as f32 * base_damage;
        let area = self.entity(entity).bounding_box().inflate(1.0, 0.25, 1.0);
        for nearby in self.get_entities(None, &area) {
            if !self.is_living(nearby) || nearby == id || nearby == entity || self.is_allied_to(id, nearby) {
                continue;
            }
            if self.entity(id).position().distance_to_sqr(self.entity(nearby).position()) < 9.0 {
                let enchanted_damage = self.get_enchanted_damage(id, nearby, sweep_damage, damage_source) * attack_strength_scale;
                if self.hurt_server(nearby, *damage_source, enchanted_damage) {
                    let y_rot = self.entity(id).y_rot * mth::DEG_TO_RAD;
                    let (sin, cos) = (mth::sin(y_rot as f64) as f64, mth::cos(y_rot as f64) as f64);
                    self.knockback(nearby, 0.4f32 as f64, sin, -cos);
                    self.do_post_attack_effects(nearby, damage_source);
                }
            }
        }
        let e = self.entity(id);
        let y_rot = e.y_rot * mth::DEG_TO_RAD;
        let dx = -mth::sin(y_rot as f64) as f64;
        let dz = mth::cos(y_rot as f64) as f64;
        let (x, y, z) = (e.x() + dx, e.y() + e.dimensions.height as f64 * 0.5, e.z() + dz);
        self.send_particles("minecraft:sweep_attack", |_| {}, x, y, z, 0, (dx, 0.0, dz), 0.0);
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

    pub fn player_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        if self.living_entity_is_invulnerable_to(id, source) {
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
        self.remove_player_menus(id);
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

    pub fn player_start_sleep_in_bed(&mut self, id: EntityId, pos: BlockPos) -> Result<(), Option<Component>> {
        if !self.start_sleeping(id, pos) {
            return Err(None);
        }
        self.player_mut(id).sleep_counter = 0;
        Ok(())
    }

    pub fn start_sleep_in_bed(&mut self, id: EntityId, _kind: BedKind, bed_block_state: crisp_data::BlockState, rule: &BedRule, pos: BlockPos) -> Result<(), Option<Component>> {
        let direction = bed_block_state.get(crisp_data::properties::HORIZONTAL_FACING);
        if self.is_sleeping(id) || !self.is_alive(id) {
            return Err(None);
        }
        let can_sleep = rule.can_sleep(self);
        let can_set_spawn = rule.can_set_spawn(self);
        if !self.bed_in_range(id, pos, direction) {
            return Err(Some(Component::translatable("block.minecraft.bed.too_far_away")));
        }
        if self.bed_blocked(id, pos, direction) {
            return Err(Some(Component::translatable("block.minecraft.bed.obstructed")));
        }
        if can_set_spawn {
            let e = self.entity(id);
            let respawn_data = crate::world::level::storage::RespawnData { dimension: self.dimension.to_string(), pos, yaw: e.y_rot, pitch: e.x_rot };
            self.set_respawn_position(id, Some(crate::server::level::server_player::RespawnConfig { respawn_data, forced: false }), true);
        }
        if !can_sleep {
            return Err(abstract_bed_block::error_message(rule));
        }
        let result = self.player_start_sleep_in_bed(id, pos);
        if !self.can_sleep_through_nights() {
            self.send_overlay_message(id, &Component::translatable("sleep.not_possible"));
        }
        self.update_sleeping_player_list();
        result
    }

    pub fn start_sleeping(&mut self, id: EntityId, bed_position: BlockPos) -> bool {
        if !self.living_entity_start_sleeping(id, bed_position) {
            return false;
        }
        let e = &self.player(id).entity;
        let (pos, y_rot, x_rot) = (e.position(), e.y_rot, e.x_rot);
        self.player_mut(id).teleport(pos, y_rot, x_rot);
        true
    }

    fn bed_in_range(&self, id: EntityId, pos: BlockPos, direction: crisp_core::Direction) -> bool {
        self.is_reachable_bed_block(id, pos) || self.is_reachable_bed_block(id, pos.relative(direction.opposite()))
    }

    fn is_reachable_bed_block(&self, id: EntityId, bed_block_pos: BlockPos) -> bool {
        let bed_block_center = Vec3::at_bottom_center_of(bed_block_pos);
        let e = self.entity(id);
        (e.x() - bed_block_center.x).abs() <= 3.0 && (e.y() - bed_block_center.y).abs() <= 2.0 && (e.z() - bed_block_center.z).abs() <= 3.0
    }

    fn bed_blocked(&mut self, _id: EntityId, pos: BlockPos, direction: crisp_core::Direction) -> bool {
        let above = pos.above();
        !self.free_at(above) || !self.free_at(above.relative(direction.opposite()))
    }

    fn free_at(&mut self, pos: BlockPos) -> bool {
        !self.block_state(pos).is_suffocating()
    }

    pub fn stop_sleep_in_bed(&mut self, id: EntityId, forceful_wake_up: bool, update_level_list: bool) {
        if self.is_sleeping(id) {
            let packet = crate::network::connection::encode(crisp_data::packets::play::clientbound::ANIMATE, |w| {
                w.varint(id);
                w.u8(0);
            });
            self.send_to_tracking_players_and_self(id, &packet);
        }
        self.living_entity_stop_sleeping(id);
        if update_level_list {
            self.update_sleeping_player_list();
        }
        self.player_mut(id).sleep_counter = if forceful_wake_up { 0 } else { 100 };
        let e = &self.player(id).entity;
        let (pos, y_rot, x_rot) = (e.position(), e.y_rot, e.x_rot);
        self.player_mut(id).teleport(pos, y_rot, x_rot);
    }

    pub fn is_sleeping_long_enough(&self, id: EntityId) -> bool {
        self.is_sleeping(id) && self.player(id).sleep_counter >= 100
    }

    pub fn set_respawn_position(&mut self, id: EntityId, respawn_config: Option<crate::server::level::server_player::RespawnConfig>, show_message: bool) {
        if show_message
            && let Some(config) = &respawn_config
            && !self.player(id).respawn_config.as_ref().is_some_and(|other| other.respawn_data.dimension == config.respawn_data.dimension && other.respawn_data.pos == config.respawn_data.pos)
        {
            self.send_system_message(id, &Component::translatable("block.minecraft.set_spawn"));
        }
        self.player_mut(id).respawn_config = respawn_config;
    }

    fn update_player_pose(&mut self, id: EntityId) {
        if !self.can_player_fit_within_blocks_and_entities_when(id, Pose::Swimming) {
            return;
        }
        let sleeping = self.is_sleeping(id);
        let p = self.player(id);
        let e = &p.entity;
        let desired = if sleeping {
            Pose::Sleeping
        } else if e.is_swimming() {
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

#[cfg(test)]
mod tests {
    use crisp_core::Vec3;
    use crisp_data::{EquipmentSlot, items};

    use crate::server::level::{ServerLevel, testing};
    use crate::world::entity::animal::chicken::Chicken;
    use crate::world::entity::{EntityId, LevelEntity};
    use crate::world::item::ItemStack;
    use crate::world::item::enchantment::enchantment_helper::enchant;

    fn setup(weapon: Option<ItemStack>) -> (ServerLevel, EntityId, EntityId) {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        if let Some(weapon) = weapon {
            level.set_item_slot(player, EquipmentSlot::Mainhand, weapon);
            level.detect_equipment_updates(player);
        }
        let mut mob = Chicken::create();
        mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
        mob.set_health(4.0);
        let chicken = mob.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
        level.player_mut(player).attack_strength_ticker = 100;
        (level, player, chicken)
    }

    fn sword(enchantments: &[(&str, i32)]) -> ItemStack {
        let mut sword = ItemStack::new(items::WOODEN_SWORD, 1);
        for &(enchantment, level) in enchantments {
            enchant(&mut sword, enchantment, level);
        }
        sword
    }

    #[test]
    fn a_full_strength_punch_deals_one_damage() {
        let (mut level, player, chicken) = setup(None);
        level.player_attack(player, chicken);
        assert_eq!(level.health(chicken), 3.0);
        assert_eq!(level.player(player).attack_strength_ticker, 0);
        assert_eq!(level.living(player).last_hurt_mob, Some(chicken));
    }

    #[test]
    fn swords_deal_their_attack_damage_and_wear_out() {
        let (mut level, player, chicken) = setup(Some(sword(&[])));
        level.player_attack(player, chicken);
        assert_eq!(level.health(chicken), 0.0);
        assert_eq!(level.item_by_slot(player, EquipmentSlot::Mainhand).unwrap().damage_value(), 1);
    }

    #[test]
    fn attacks_right_after_a_swing_are_weak() {
        let (mut level, player, chicken) = setup(None);
        level.player_mut(player).attack_strength_ticker = 0;
        level.player_attack(player, chicken);
        let scale = 0.5f32 / 5.0;
        assert_eq!(level.health(chicken), 4.0 - (0.2 + scale * scale * 0.8));
    }

    #[test]
    fn sharpness_adds_magic_damage() {
        let (mut level, player, chicken) = setup(Some(ItemStack::new(items::STICK, 1)));
        let mut stick = ItemStack::new(items::STICK, 1);
        enchant(&mut stick, "minecraft:sharpness", 1);
        level.set_item_slot(player, EquipmentSlot::Mainhand, stick);
        level.player_attack(player, chicken);
        assert_eq!(level.health(chicken), 2.0);
    }

    #[test]
    fn fire_aspect_sets_the_target_on_fire() {
        let (mut level, player, chicken) = setup(Some(sword(&[("minecraft:fire_aspect", 1)])));
        level.mob_mut(chicken).unwrap().set_health(4.0);
        level.player_attack(player, chicken);
        assert_eq!(level.entity(chicken).remaining_fire_ticks, 80);
    }

    #[test]
    fn falling_players_land_critical_hits() {
        let (mut level, player, chicken) = setup(None);
        let e = level.entity_mut(player);
        e.on_ground = false;
        e.fall_distance = 1.0;
        level.player_attack(player, chicken);
        assert_eq!(level.health(chicken), 2.5);
    }
}
