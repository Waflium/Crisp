use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_data::properties::{HORIZONTAL_FACING, OCCUPIED, OPEN};
use crisp_protocol::nbt::Nbt;
use crisp_data::{EquipmentSlot, EquipmentSlotGroup, MobEffect, block_tags, blocks, damage_type_tags, damage_types, entity_type_tags, fluid_tags, game_rule, item_tags, items, mob_effects};

use super::attributes::{Attribute, AttributeMap, AttributeModifier, AttributeSupplier, Operation, PLAYER_ATTRIBUTES, SPEED_MODIFIER_SPRINTING};
use super::data::{DataValue, Pose, ids};
use super::mob::MobKind;
use super::{Entity, EntityId, MoverType, RemovalReason, TOTAL_AIR_SUPPLY, compute_modified_friction, entity_event, flags, sound_event};
use crate::core::holder_set::HolderSet;
use crate::network::connection::encode;
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::item::component::codec as component_codec;
use crate::world::InteractionHand;
use crate::world::damagesource::DamageSource;
use crate::world::damagesource::combat_rules;
use crate::world::damagesource::combat_tracker::CombatTracker;
use crate::world::effect::MobEffectInstance;
use crate::world::effect::mob_effect::{add_attribute_modifiers, remove_attribute_modifiers, should_apply_effect_tick_this_tick};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::attack_range::AttackRange;
use crate::world::item::component::swing_animation::SwingAnimation;
use crate::world::item::enchantment::{enchantment, enchantment_helper};
use crate::world::level::BlockGetter;
use crate::world::level::block::abstract_bed_block;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::scores::team::CollisionRule;

const SPEED_MODIFIER_POWDER_SNOW_ID: &str = "minecraft:powder_snow";
const TICKS_REQUIRED_TO_FREEZE: i32 = 140;

pub struct LivingEntity {
    pub attributes: AttributeMap,
    pub y_body_rot: f32,
    pub no_action_time: i32,
    pub last_hurt_by_player: Option<u128>,
    pub last_hurt_by_player_memory_time: i32,
    pub last_hurt_by_mob: Option<u128>,
    pub last_hurt_by_mob_timestamp: i32,
    pub last_hurt_mob: Option<EntityId>,
    pub last_hurt_mob_timestamp: i32,
    pub jumping: bool,
    pub xxa: f32,
    pub yya: f32,
    pub zza: f32,
    pub speed: f32,
    pub no_jump_delay: i32,
    pub last_climbable_pos: Option<BlockPos>,
    pub current_impulse_impact_pos: Option<Vec3>,
    pub current_impulse_context_reset_grace_time: i32,
    pub hurt_time: i32,
    pub hurt_duration: i32,
    pub death_time: i32,
    pub damage_cooldown_time: i32,
    pub last_hurt: f32,
    pub dead: bool,
    pub last_damage_source: Option<DamageSource>,
    pub last_damage_stamp: i64,
    pub combat_tracker: CombatTracker,
    pub hurt_dir: f32,
    pub use_item: Option<ItemStack>,
    pub use_item_remaining: i32,
    pub last_equipment_items: [Option<ItemStack>; 8],
    pub swing_state: SwingState,
    pub active_effects: Vec<MobEffectInstance>,
    pub effects_dirty: bool,
    pub effects_mod_count: u32,
}

impl LivingEntity {
    pub fn player() -> LivingEntity {
        LivingEntity::new(&PLAYER_ATTRIBUTES)
    }

    pub fn new(attributes: &'static AttributeSupplier) -> LivingEntity {
        LivingEntity {
            attributes: AttributeMap::new(attributes),
            y_body_rot: 0.0,
            no_action_time: 0,
            last_hurt_by_player: None,
            last_hurt_by_player_memory_time: 0,
            last_hurt_by_mob: None,
            last_hurt_by_mob_timestamp: 0,
            last_hurt_mob: None,
            last_hurt_mob_timestamp: 0,
            jumping: false,
            xxa: 0.0,
            yya: 0.0,
            zza: 0.0,
            speed: 0.0,
            no_jump_delay: 0,
            last_climbable_pos: None,
            current_impulse_impact_pos: None,
            current_impulse_context_reset_grace_time: 0,
            hurt_time: 0,
            hurt_duration: 0,
            death_time: 0,
            damage_cooldown_time: 0,
            last_hurt: 0.0,
            dead: false,
            last_damage_source: None,
            last_damage_stamp: 0,
            combat_tracker: Default::default(),
            hurt_dir: 0.0,
            use_item: None,
            use_item_remaining: 0,
            last_equipment_items: Default::default(),
            swing_state: SwingState::default(),
            active_effects: Vec::new(),
            effects_dirty: true,
            effects_mod_count: 0,
        }
    }

    pub fn reset_current_impulse_context(&mut self) {
        self.current_impulse_context_reset_grace_time = 0;
        self.current_impulse_impact_pos = None;
    }

    pub fn try_reset_current_impulse_context(&mut self) {
        if self.current_impulse_context_reset_grace_time == 0 {
            self.reset_current_impulse_context();
        }
    }

    pub fn is_in_post_impulse_grace_time(&self) -> bool {
        self.current_impulse_context_reset_grace_time > 0
    }

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwingDescription {
    pub hand: InteractionHand,
    pub animation: SwingAnimation,
    pub duration_ticks: i32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct SwingState {
    pub current_swing: Option<SwingDescription>,
    ticks: i32,
    old_animation: f32,
    pub animation: f32,
}

impl SwingState {
    pub fn start_if_able(&mut self, hand: InteractionHand, animation: SwingAnimation, duration_ticks: i32) -> bool {
        if self.current_swing.is_some_and(|s| self.ticks <= s.duration_ticks / 2) && self.ticks > 0 {
            return false;
        }
        let old_swing = self.current_swing;
        let swing = SwingDescription { hand, animation, duration_ticks };
        self.current_swing = Some(swing);
        if old_swing != Some(swing) {
            self.old_animation = 0.0;
            self.animation = 0.0;
        }
        self.ticks = 0;
        true
    }

    pub fn tick(&mut self) {
        self.old_animation = self.animation;
        match self.current_swing {
            Some(swing) => {
                if swing.duration_ticks > 0 {
                    self.animation = (self.ticks as f32 / swing.duration_ticks as f32).min(1.0);
                }
                let ticks = self.ticks;
                self.ticks += 1;
                if ticks > swing.duration_ticks {
                    self.current_swing = None;
                    self.animation = 0.0;
                }
            }
            None => self.animation = 0.0,
        }
    }
}

pub fn get_comfortable_fall_distance(allowed_damage: f32) -> i32 {
    mth::floor_f(allowed_damage + 3.0)
}

pub fn entity_event_for_equipment_break(slot: EquipmentSlot) -> u8 {
    match slot {
        EquipmentSlot::Mainhand => entity_event::MAINHAND_BREAK,
        EquipmentSlot::Offhand => entity_event::OFFHAND_BREAK,
        EquipmentSlot::Head => entity_event::HEAD_BREAK,
        EquipmentSlot::Chest => entity_event::CHEST_BREAK,
        EquipmentSlot::Feet => entity_event::FEET_BREAK,
        EquipmentSlot::Legs => entity_event::LEGS_BREAK,
        EquipmentSlot::Body => entity_event::BODY_BREAK,
        EquipmentSlot::Saddle => entity_event::SADDLE_BREAK,
    }
}

fn for_each_modifier(stack: &ItemStack, slot: EquipmentSlot, mut f: impl FnMut(Attribute, AttributeModifier)) {
    if let Some(modifiers) = stack.get(dc::ATTRIBUTE_MODIFIERS) {
        for entry in modifiers.as_list() {
            let group = entry.get("slot").map_or(Ok(EquipmentSlotGroup::Any), enchantment::slot_group);
            if !group.is_ok_and(|group| group.test(slot)) {
                continue;
            }
            let attribute = entry.get("type").and_then(Nbt::as_str).and_then(Attribute::by_name);
            let id = entry.get("id").and_then(Nbt::as_str);
            let operation = entry.get("operation").and_then(Nbt::as_str).and_then(Operation::by_name);
            let amount = entry.get("amount").map(crate::world::item::component::codec::as_f64);
            if let (Some(attribute), Some(id), Some(operation), Some(amount)) = (attribute, id, operation, amount) {
                f(attribute, AttributeModifier { id: std::borrow::Cow::Owned(id.to_string()), amount, operation });
            }
        }
    }
    enchantment_helper::for_each_modifier(stack, slot, f);
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

pub fn update_mob_effect(entity_id: EntityId, effect: &MobEffectInstance, blend: bool) -> Vec<u8> {
    encode(clientbound::UPDATE_MOB_EFFECT, |p| {
        p.varint(entity_id);
        p.varint(effect.effect.protocol_id());
        p.varint(effect.amplifier);
        p.varint(effect.duration);
        p.u8(effect.ambient as u8 | (effect.visible as u8) << 1 | (effect.show_icon as u8) << 2 | (blend as u8) << 3);
    })
}

pub fn remove_mob_effect(entity_id: EntityId, effect: MobEffect) -> Vec<u8> {
    encode(clientbound::REMOVE_MOB_EFFECT, |p| {
        p.varint(entity_id);
        p.varint(effect.protocol_id());
    })
}

fn are_all_effects_ambient(effects: &[MobEffectInstance]) -> bool {
    effects.iter().all(|e| !e.visible || e.ambient)
}

impl ServerLevel {
    pub fn living(&self, id: EntityId) -> &LivingEntity {
        match self.players.get(&id) {
            Some(p) => &p.living,
            None => &self.mob(id).expect("a living entity").living,
        }
    }

    pub fn living_mut(&mut self, id: EntityId) -> &mut LivingEntity {
        if self.players.contains_key(&id) {
            return &mut self.player_mut(id).living;
        }
        &mut self.mob_mut(id).expect("a living entity").living
    }

    pub fn entity_and_living(&self, id: EntityId) -> (&Entity, &LivingEntity) {
        match self.players.get(&id) {
            Some(p) => (&p.entity, &p.living),
            None => {
                let mob = self.mob(id).expect("a living entity");
                (&mob.entity, &mob.living)
            }
        }
    }

    pub fn entity_and_living_mut(&mut self, id: EntityId) -> (&mut Entity, &mut LivingEntity) {
        if self.players.contains_key(&id) {
            let p = self.player_mut(id);
            return (&mut p.entity, &mut p.living);
        }
        let mob = self.mob_mut(id).expect("a living entity");
        (&mut mob.entity, &mut mob.living)
    }

    pub fn health(&self, id: EntityId) -> f32 {
        match self.players.get(&id) {
            Some(p) => p.health,
            None => self.mob(id).expect("a living entity").health,
        }
    }

    pub fn item_by_slot(&self, id: EntityId, slot: EquipmentSlot) -> Option<ItemStack> {
        if let Some(mob) = self.mob(id) {
            return mob.item_by_slot(slot).cloned();
        }
        let index = self.equipment_inventory_slot(id, slot)?;
        self.player(id).inventory.slots[index].clone().filter(|s| !s.is_empty())
    }

    pub fn on_equip_item(&mut self, id: EntityId, slot: EquipmentSlot, old_stack: &ItemStack, stack: &ItemStack) {
        if self.is_spectator(id) || ItemStack::is_same_item_same_components(old_stack, stack) || self.entity(id).first_tick {
            return;
        }
        let Some(equippable) = stack.get(crate::world::item::component::EQUIPPABLE).map(crate::world::item::component::values::Equippable::from_nbt) else { return };
        if !self.entity(id).is_silent() && EquipmentSlot::by_name(&equippable.slot) == Some(slot) {
            let Some(sound) = equippable.equip_sound.as_str().and_then(|name| crisp_data::registry::SOUND_EVENT.iter().position(|&s| s == name)) else { return };
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let seed = e.random.next_long();
            self.play_seeded_sound_at(None, x, y, z, sound as i32, source, 1.0, 1.0, seed);
        }
    }

    pub fn living_set_item_slot(&mut self, id: EntityId, slot: EquipmentSlot, item_stack: ItemStack) {
        let old_stack = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
        self.set_item_slot(id, slot, item_stack.clone());
        self.on_equip_item(id, slot, &old_stack, &item_stack);
    }

    pub fn set_item_slot(&mut self, id: EntityId, slot: EquipmentSlot, item_stack: ItemStack) {
        if let Some(mob) = self.mob_mut(id) {
            mob.equipment[slot as usize] = Some(item_stack).filter(|s| !s.is_empty());
            return;
        }
        let Some(index) = self.equipment_inventory_slot(id, slot) else { return };
        let p = self.player_mut(id);
        p.inventory.slots[index] = Some(item_stack).filter(|s| !s.is_empty());
        p.dirty_slots.push(index);
    }

    pub fn set_last_hurt_mob(&mut self, id: EntityId, target: EntityId) {
        let target = self.is_living(target).then_some(target);
        let tick_count = self.entity(id).tick_count;
        let living = self.living_mut(id);
        living.last_hurt_mob = target;
        living.last_hurt_mob_timestamp = tick_count;
    }

    pub fn get_knockback(&mut self, id: EntityId, target: EntityId, damage_source: &DamageSource) -> f32 {
        let knockback = self.attribute_value(id, Attribute::AttackKnockback) as f32;
        let weapon = self.get_weapon_item(id);
        self.modify_knockback(&weapon, target, damage_source, knockback) / 2.0
    }

    pub fn post_piercing_attack(&mut self, id: EntityId) {
        self.do_post_piercing_attack_effects(id);
    }

    pub fn get_attack_range_with(&self, id: EntityId, weapon_item: &ItemStack) -> AttackRange {
        match weapon_item.get(dc::ATTACK_RANGE) {
            Some(attack_range) => AttackRange::from_nbt(attack_range),
            None => AttackRange::default_for(self.attribute_value(id, Attribute::EntityInteractionRange) as f32),
        }
    }

    pub fn get_weapon_item(&self, id: EntityId) -> ItemStack {
        self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty)
    }

    pub fn hurt_and_break(&mut self, owner: EntityId, slot: EquipmentSlot, amount: i32) {
        let Some(mut item_stack) = self.item_by_slot(owner, slot) else { return };
        let new_amount = if !item_stack.is_damageable_item() || self.players.get(&owner).is_some_and(|p| p.abilities.instabuild) {
            0
        } else if amount > 0 {
            self.process_durability_change(&item_stack, amount)
        } else {
            amount
        };
        if new_amount == 0 {
            return;
        }
        item_stack.set_damage_value(item_stack.damage_value() + new_amount);
        if item_stack.is_broken() {
            let broken = item_stack.copy();
            item_stack.shrink(1);
            self.set_item_slot(owner, slot, item_stack);
            self.on_equipped_item_broken(owner, &broken, slot);
        } else {
            self.set_item_slot(owner, slot, item_stack);
        }
    }

    pub fn on_equipped_item_broken(&mut self, id: EntityId, broken_item: &ItemStack, in_slot: EquipmentSlot) {
        self.broadcast_entity_event(id, entity_event_for_equipment_break(in_slot));
        self.stop_location_based_effects(id, broken_item, in_slot);
    }

    pub fn max_health(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MaxHealth) as f32
    }

    pub fn max_absorption(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MaxAbsorption) as f32
    }

    pub fn absorption_amount(&self, id: EntityId) -> f32 {
        if let Some(mob) = self.mob(id) {
            return mob.absorption;
        }
        match self.player(id).entity.data.get(ids::PLAYER_ABSORPTION) {
            DataValue::Float(v) => *v,
            _ => 0.0,
        }
    }

    pub fn set_absorption_amount(&mut self, id: EntityId, amount: f32) {
        let amount = mth::clamp_f(amount, 0.0, self.max_absorption(id));
        if let Some(mob) = self.mob_mut(id) {
            mob.absorption = amount;
            return;
        }
        self.player_mut(id).entity.data.set(ids::PLAYER_ABSORPTION, DataValue::Float(amount));
    }

    pub fn set_health(&mut self, id: EntityId, health: f32) {
        let health = mth::clamp_f(health, 0.0, self.max_health(id));
        if let Some(mob) = self.mob_mut(id) {
            mob.health = health;
            mob.entity.data.set(ids::HEALTH, DataValue::Float(health));
            return;
        }
        let p = self.player_mut(id);
        p.health = health;
        p.entity.data.set(ids::HEALTH, DataValue::Float(health));
    }

    pub fn heal(&mut self, id: EntityId, amount: f32) {
        let health = self.health(id);
        if health > 0.0 {
            self.set_health(id, health + amount);
        }
    }

    pub fn is_dead_or_dying(&self, id: EntityId) -> bool {
        self.health(id) <= 0.0 || self.living(id).dead
    }

    pub fn is_baby(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.is_baby())
    }

    pub fn armor_value(&self, id: EntityId) -> i32 {
        mth::floor(self.attribute_value(id, Attribute::Armor))
    }

    pub fn has_line_of_sight(&mut self, id: EntityId, target: EntityId) -> bool {
        let e = self.entity(id);
        let from = Vec3::new(e.x(), e.eye_y(), e.z());
        let t = self.entity(target);
        let to = Vec3::new(t.x(), t.eye_y(), t.z());
        if to.distance_to(from) > 128.0 {
            return false;
        }
        let ctx = ClipContext { from, to, block: ClipBlock::Collider, fluid: ClipFluid::None, is_player: self.is_player(id) };
        self.clip(&ctx).miss
    }

    pub fn get_visibility_percent(&self, id: EntityId, targeting_entity: Option<EntityId>) -> f64 {
        let mut visibility_percent = 1.0;
        let e = self.entity(id);
        if e.is_discrete() {
            visibility_percent *= 0.8;
        }
        if e.shared_flag(flags::INVISIBLE) {
            let mut cover_percentage = self.get_armor_cover_percentage(id);
            if cover_percentage < 0.1 {
                cover_percentage = 0.1;
            }
            visibility_percent *= 0.7 * cover_percentage as f64;
        }
        if let Some(targeting_entity) = targeting_entity {
            let targeting_type = self.entity(targeting_entity).entity_type;
            for slot in EquipmentSlot::VALUES {
                let Some(item) = self.item_by_slot_ref(id, slot) else { continue };
                let Some(mob_visibility) = item.get(dc::MOB_VISIBILITY) else { continue };
                let equippable_slot = item.get(dc::EQUIPPABLE).and_then(|equippable| equippable.get("slot")).and_then(Nbt::as_str).and_then(EquipmentSlot::by_name);
                let targets = mob_visibility
                    .get("targeting_entity_types")
                    .and_then(|types| HolderSet::decode("minecraft:entity_type", types).ok())
                    .is_some_and(|types| types.contains(targeting_type));
                if equippable_slot == Some(slot) && targets {
                    visibility_percent *= mob_visibility.get("visibility").map_or(0.0, component_codec::as_f32) as f64;
                }
            }
        }
        visibility_percent.clamp(0.0, 10.0)
    }

    pub fn get_armor_cover_percentage(&self, id: EntityId) -> f32 {
        let mut total = 0;
        let mut count = 0;
        for slot in [EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head] {
            if self.item_by_slot_ref(id, slot).is_some() {
                count += 1;
            }
            total += 1;
        }
        if total > 0 { count as f32 / total as f32 } else { 0.0 }
    }

    pub fn can_attack(&self, _id: EntityId, target: EntityId) -> bool {
        if self.is_player(target) && self.difficulty == Difficulty::Peaceful { false } else { self.can_be_seen_as_enemy(target) }
    }

    pub fn can_be_seen_as_enemy(&self, id: EntityId) -> bool {
        if self.is_player(id) && self.player(id).abilities.invulnerable {
            return false;
        }
        !self.entity(id).invulnerable && self.can_be_seen_by_anyone(id)
    }

    pub fn can_be_seen_by_anyone(&self, id: EntityId) -> bool {
        !self.is_spectator(id) && self.is_alive(id)
    }

    pub fn get_last_damage_source(&self, id: EntityId) -> Option<&DamageSource> {
        let living = self.living(id);
        if self.game_time - living.last_damage_stamp > 40 { None } else { living.last_damage_source.as_ref() }
    }

    pub fn item_by_slot_ref(&self, id: EntityId, slot: EquipmentSlot) -> Option<&ItemStack> {
        if let Some(mob) = self.mob(id) {
            return mob.item_by_slot(slot);
        }
        let index = self.equipment_inventory_slot(id, slot)?;
        self.player(id).inventory.slots[index].as_ref().filter(|s| !s.is_empty())
    }

    pub fn swing(&mut self, id: EntityId, hand: InteractionHand, animation: SwingAnimation, send_to_swinging_entity: bool) -> bool {
        let duration = animation.duration;
        if !self.living_mut(id).swing_state.start_if_able(hand, animation, duration) {
            return false;
        }
        let packet = encode(clientbound::SWING_ANIMATION, |w| {
            w.varint(id);
            w.varint(hand as i32);
            animation.write(w);
        });
        if send_to_swinging_entity {
            self.send_to_tracking_players_and_self(id, &packet);
        } else {
            self.send_to_tracking_players(id, &packet);
        }
        true
    }

    pub fn set_sprinting(&mut self, id: EntityId, sprinting: bool) {
        let (e, living) = self.entity_and_living_mut(id);
        e.set_shared_flag(flags::SPRINTING, sprinting);
        let speed = living.attributes.instance(Attribute::MovementSpeed).expect("living entities have movement speed");
        speed.remove_modifier(&SPEED_MODIFIER_SPRINTING.id);
        if sprinting {
            speed.add_transient_modifier(SPEED_MODIFIER_SPRINTING);
        }
    }

    pub fn living_entity_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        self.is_invulnerable_to_base(id, source) || self.is_immune_to_damage(id, source)
    }

    pub fn living_entity_hurt_server(&mut self, id: EntityId, source: DamageSource, mut damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) || self.is_dead_or_dying(id) {
            return false;
        }
        if source.is(&damage_type_tags::IS_FIRE) && self.has_effect(id, mob_effects::FIRE_RESISTANCE) {
            return false;
        }
        if self.is_sleeping(id) {
            self.stop_sleeping(id);
        }
        self.living_mut(id).no_action_time = 0;
        if damage < 0.0 {
            damage = 0.0;
        }
        let blocked = false;
        if source.is(&damage_type_tags::IS_FREEZING) && entity_type_tags::FREEZE_HURTS_EXTRA_TYPES.contains(self.entity(id).entity_type as usize) {
            damage *= 5.0;
        }
        if source.is(&damage_type_tags::DAMAGES_HELMET) && self.item_by_slot(id, EquipmentSlot::Head).is_some() {
            self.hurt_helmet(id, &source, damage);
            damage *= 0.75;
        }
        if !damage.is_finite() {
            damage = f32::MAX;
        }
        let mut took_full_damage = true;
        let living = &self.living(id);
        if living.damage_cooldown_time as f32 > 10.0 && !source.is(&damage_type_tags::BYPASSES_COOLDOWN) {
            if damage <= living.last_hurt {
                return false;
            }
            let last_hurt = living.last_hurt;
            self.actually_hurt(id, &source, damage - last_hurt);
            self.living_mut(id).last_hurt = damage;
            took_full_damage = false;
        } else {
            let living = &mut self.living_mut(id);
            living.last_hurt = damage;
            living.damage_cooldown_time = 20;
            self.actually_hurt(id, &source, damage);
            let living = &mut self.living_mut(id);
            living.hurt_duration = 10;
            living.hurt_time = living.hurt_duration;
        }
        self.resolve_mob_responsible_for_damage(id, &source);
        self.resolve_player_responsible_for_damage(id, &source);
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
                    let sound = self.get_death_sound(id);
                    self.make_sound(id, sound);
                    self.play_secondary_hurt_sound(id, &source);
                }
                self.die(id, source);
            }
        } else if took_full_damage {
            self.play_hurt_sound(id, &source);
            self.play_secondary_hurt_sound(id, &source);
        }
        let success = !blocked || damage > 0.0;
        if success {
            let game_time = self.game_time;
            let living = &mut self.living_mut(id);
            living.last_damage_source = Some(source);
            living.last_damage_stamp = game_time;
        }
        success
    }

    pub fn actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_player(id) {
            self.player_actually_hurt(id, source, damage);
        } else if self.mob(id).is_some_and(|mob| mob.animal().is_some()) {
            self.animal_actually_hurt(id, source, damage);
        } else {
            self.living_entity_actually_hurt(id, source, damage);
        }
    }

    pub fn living_entity_actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
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
            self.record_damage(id, *source, damage);
            let health = self.health(id);
            self.set_health(id, health - damage);
            let absorption = self.absorption_amount(id);
            self.set_absorption_amount(id, absorption - damage);
        }
    }

    fn resolve_mob_responsible_for_damage(&mut self, id: EntityId, source: &DamageSource) {
        let Some(living_source) = source.causing_entity.filter(|&e| self.is_living(e)) else { return };
        if source.is(&damage_type_tags::NO_ANGER) || source.is_type(damage_types::WIND_CHARGE) && entity_type_tags::NO_ANGER_FROM_WIND_CHARGE.contains(self.entity(id).entity_type as usize) {
            return;
        }
        let uuid = self.entity(living_source).uuid;
        let tick_count = self.entity(id).tick_count;
        let living = self.living_mut(id);
        living.last_hurt_by_mob = Some(uuid);
        living.last_hurt_by_mob_timestamp = tick_count;
    }

    fn resolve_player_responsible_for_damage(&mut self, id: EntityId, source: &DamageSource) {
        if let Some(player) = source.causing_entity.filter(|&e| self.is_player(e)) {
            let uuid = self.entity(player).uuid;
            let living = self.living_mut(id);
            living.last_hurt_by_player = Some(uuid);
            living.last_hurt_by_player_memory_time = 100;
        }
    }

    fn get_death_sound(&self, id: EntityId) -> i32 {
        match self.mob(id) {
            Some(mob) => sound_event(&mob.sound_set().death_sound),
            None => sound_event("minecraft:entity.player.death"),
        }
    }

    fn play_hurt_sound(&mut self, id: EntityId, source: &DamageSource) {
        if let Some(mob) = self.mob_mut(id) {
            mob.ambient_sound_time = -mob.ambient_sound_interval();
        }
        let sound = self.get_hurt_sound(id, source);
        self.make_sound(id, sound);
    }

    fn get_hurt_sound(&self, id: EntityId, source: &DamageSource) -> i32 {
        match self.mob(id) {
            Some(mob) => sound_event(&mob.sound_set().hurt_sound),
            None => sound_event(source.ty.info().effects.sound()),
        }
    }

    fn hurt_armor(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_player(id) {
            self.do_hurt_equipment(id, source, damage, &[EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head]);
        }
    }

    fn hurt_helmet(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_player(id) {
            self.do_hurt_equipment(id, source, damage, &[EquipmentSlot::Head]);
        }
    }

    pub fn damage_after_armor_absorb(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> f32 {
        if source.is(&damage_type_tags::BYPASSES_ARMOR) {
            return damage;
        }
        self.hurt_armor(id, source, damage);
        let armor = self.armor_value(id) as f32;
        let toughness = self.attribute_value(id, Attribute::ArmorToughness) as f32;
        combat_rules::get_damage_after_absorb(self, id, damage, source, armor, toughness)
    }

    pub fn damage_after_magic_absorb(&mut self, id: EntityId, source: &DamageSource, mut damage: f32) -> f32 {
        if source.is(&damage_type_tags::BYPASSES_EFFECTS) {
            return damage;
        }
        if let Some(resistance) = self.living(id).effect(mob_effects::RESISTANCE)
            && !source.is(&damage_type_tags::BYPASSES_RESISTANCE)
        {
            let absorb = 25 - (resistance.amplifier + 1) * 5;
            damage = (damage * absorb as f32 / 25.0).max(0.0);
        }
        if damage <= 0.0 {
            return 0.0;
        }
        if source.is(&damage_type_tags::BYPASSES_ENCHANTMENTS) {
            return damage;
        }
        let enchantment_armor = self.get_damage_protection(id, source);
        if enchantment_armor > 0.0 {
            damage = combat_rules::get_damage_after_magic_absorb(damage, enchantment_armor);
        }
        damage
    }

    fn do_hurt_equipment(&mut self, id: EntityId, source: &DamageSource, damage: f32, slots: &[EquipmentSlot]) {
        if damage <= 0.0 {
            return;
        }
        let durability_damage = (damage / 4.0).max(1.0) as i32;
        for &slot in slots {
            let Some(stack) = self.item_by_slot(id, slot) else { continue };
            let info = stack.item.info();
            let can_be_hurt_by = info.damage_resistant.is_none_or(|tag| !source.is(tag));
            if info.equipment_slot.is_some() && info.damage_on_hurt && stack.is_damageable_item() && can_be_hurt_by {
                self.hurt_and_break(id, slot, durability_damage);
            }
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
        if self.is_player(id) {
            self.server_player_indicate_damage(id, xd, zd);
        }
    }

    pub fn make_sound(&mut self, id: EntityId, sound: i32) {
        let pitch = self.voice_pitch(id);
        self.entity_play_sound(id, sound, 1.0, pitch);
    }

    pub fn voice_pitch(&mut self, id: EntityId) -> f32 {
        let baby = self.is_baby(id);
        let random = &mut self.entity_mut(id).random;
        let spread = (random.next_float() - random.next_float()) * 0.2;
        if baby { spread + 1.5 } else { spread + 1.0 }
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
            let Some(mut stack) = self.item_by_slot(id, slot) else { continue };
            let Some(death_effects) = stack.item.info().death_protection else { continue };
            stack.shrink(1);
            self.set_item_slot(id, slot, stack);
            self.set_health(id, 1.0);
            for effect in death_effects {
                self.apply_consume_effect(id, effect);
            }
            self.broadcast_entity_event(id, entity_event::PROTECTED_FROM_DEATH);
            return true;
        }
        false
    }

    pub fn die(&mut self, id: EntityId, source: DamageSource) {
        if self.is_player(id) {
            self.server_player_die(id, source);
        } else {
            self.living_entity_die(id, source);
        }
    }

    pub fn living_entity_die(&mut self, id: EntityId, source: DamageSource) {
        if self.entity(id).is_removed() || self.living(id).dead {
            return;
        }
        if let Some(killer) = self.kill_credit(id) {
            self.award_kill_score(killer, id);
        }
        if self.is_sleeping(id) {
            self.stop_sleeping(id);
        }
        if self.entity(id).custom_name().is_some() {
            let message = self.death_message(id);
            info!("Named entity {} died: {}", self.entity_to_string(id), message.get_string());
        }
        self.living_mut(id).dead = true;
        self.drop_all_death_loot(id, &source);
        self.broadcast_entity_event(id, entity_event::DEATH);
        self.entity_mut(id).set_pose(Pose::Dying);
    }

    pub fn award_kill_score(&mut self, killer: EntityId, victim: EntityId) {
        if self.is_player(killer) {
            self.server_player_award_kill_score(killer, victim);
        }
    }

    pub fn drop_all_death_loot(&mut self, id: EntityId, source: &DamageSource) {
        let player_killed = self.living(id).last_hurt_by_player_memory_time > 0;
        if self.should_drop_loot(id) {
            self.drop_from_loot_table(id, source, player_killed);
        }
        self.drop_equipment(id);
        self.drop_experience(id, source.causing_entity);
    }

    fn should_drop_loot(&self, id: EntityId) -> bool {
        !self.is_baby(id) && self.game_rules.get_bool(game_rule::MOB_DROPS)
    }

    fn should_drop_experience(&self, id: EntityId) -> bool {
        !self.is_baby(id)
    }

    fn is_always_experience_dropper(&self, id: EntityId) -> bool {
        self.is_player(id)
    }

    fn drop_equipment(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.player_drop_equipment(id);
        }
    }

    fn drop_experience(&mut self, id: EntityId, killer: Option<EntityId>) {
        let living = self.living(id);
        if self.is_always_experience_dropper(id) || living.last_hurt_by_player_memory_time > 0 && self.should_drop_experience(id) && self.game_rules.get_bool(game_rule::MOB_DROPS) {
            let reward = self.get_experience_reward(id, killer);
            let pos = self.entity(id).position();
            self.award_experience(pos, reward);
        }
    }

    fn get_experience_reward(&mut self, id: EntityId, killer: Option<EntityId>) -> i32 {
        let base = self.get_base_experience_reward(id);
        self.process_mob_experience(killer, base)
    }

    fn get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        if self.is_player(id) {
            return self.player_get_base_experience_reward(id);
        }
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Chicken(_)) => self.chicken_get_base_experience_reward(id),
            None => 0,
        }
    }

    fn drop_from_loot_table(&mut self, id: EntityId, source: &DamageSource, player_killed: bool) {
        let Some(loot_table) = self.mob(id).and_then(|mob| mob.loot_table_id()) else { return };
        self.drop_from_loot_table_into(id, source, player_killed, &loot_table, &mut |level, item_stack| level.spawn_at_location(id, item_stack, 0.0));
        if let Some(mob) = self.mob_mut(id) {
            mob.loot_table = None;
        }
    }

    pub fn get_last_hurt_by_player(&self, id: EntityId) -> Option<EntityId> {
        self.living(id).last_hurt_by_player.as_ref().and_then(|&uuid| self.entity_manager.visible_entity_storage.get(uuid)).filter(|&p| self.is_player(p))
    }

    pub fn get_loot_table_seed(&self, id: EntityId) -> i64 {
        self.mob(id).map_or(0, |mob| mob.loot_table_seed)
    }

    pub fn drop_from_loot_table_into(
        &mut self,
        id: EntityId,
        source: &DamageSource,
        player_killed: bool,
        loot_table: &str,
        item_stack_consumer: &mut dyn FnMut(&mut ServerLevel, ItemStack),
    ) {
        let table = LootTable::get(loot_table);
        let position = self.entity(id).position();
        let killer_player = self.get_last_hurt_by_player(id).filter(|_| player_killed);
        let luck = killer_player.map(|player| self.attribute_value(player, Attribute::Luck) as f32);
        let loot_table_seed = self.get_loot_table_seed(id);
        let mut params = LootParams::new(self);
        params.this_entity = Some(id);
        params.origin = Some(position);
        params.damage_source = Some(*source);
        params.attacking_entity = source.causing_entity;
        params.direct_attacking_entity = source.direct_entity;
        if let (Some(killer_player), Some(luck)) = (killer_player, luck) {
            params.last_damage_player = Some(killer_player);
            params.luck = luck;
        }
        table.get_random_items_with_seed(params, loot_table_seed, item_stack_consumer);
    }

    pub fn drop_from_gift_loot_table(&mut self, id: EntityId, key: &str, consumer: impl FnMut(&mut ServerLevel, ItemStack)) -> bool {
        let position = self.entity(id).position();
        self.drop_from_loot_table_with_params(
            key,
            |params| {
                params.origin = Some(position);
                params.this_entity = Some(id);
            },
            consumer,
        )
    }

    fn drop_from_loot_table_with_params(&mut self, key: &str, params_builder: impl FnOnce(&mut LootParams), mut consumer: impl FnMut(&mut ServerLevel, ItemStack)) -> bool {
        let loot_table = LootTable::get(key);
        let mut params = LootParams::new(self);
        params_builder(&mut params);
        let drops = loot_table.get_random_items(params);
        if drops.is_empty() {
            return false;
        }
        for stack in drops {
            consumer(self, stack);
        }
        true
    }

    pub fn living_entity_tick(&mut self, id: EntityId) {
        self.entity_tick(id);
        self.updating_using_item(id);
        self.detect_equipment_updates(id);
        if self.entity(id).tick_count % 20 == 0 {
            self.recheck_combat_status(id);
        }
        if self.is_sleeping(id) {
            let can_interact_with_level = self.is_alive(id) && !self.entity(id).is_removed() && !self.is_spectator(id);
            if !can_interact_with_level || !self.check_bed_exists(id) {
                self.stop_sleeping(id);
            }
        }
        if !self.entity(id).is_removed() {
            self.ai_step(id);
        }
        if !self.is_living(id) {
            return;
        }
        if self.is_mob(id) {
            self.body_rotation_control_tick(id);
        }
        if self.is_sleeping(id) {
            self.entity_mut(id).set_x_rot(0.0);
        }
        self.refresh_dirty_attributes(id);
        let living = self.living_mut(id);
        if living.current_impulse_context_reset_grace_time > 0 {
            living.current_impulse_context_reset_grace_time -= 1;
        }
    }

    pub fn living_entity_base_tick(&mut self, id: EntityId) {
        self.living_mut(id).swing_state.tick();
        self.entity_base_tick(id);
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
                let can_drown = !entity_type_tags::CAN_BREATHE_UNDER_WATER.contains(self.entity(id).entity_type as usize)
                    && !self.has_water_breathing(id)
                    && !self.players.get(&id).is_some_and(|p| p.abilities.invulnerable);
                if can_drown {
                    let supply = self.decrease_air_supply(id, self.entity(id).air_supply());
                    self.entity_mut(id).set_air_supply(supply);
                    if self.entity(id).air_supply() <= -20 {
                        self.entity_mut(id).set_air_supply(0);
                        self.broadcast_entity_event(id, entity_event::DROWN_PARTICLES);
                        self.hurt_server(id, DamageSource::new(damage_types::DROWN), 2.0);
                    }
                } else if self.entity(id).air_supply() < TOTAL_AIR_SUPPLY && self.should_effects_refill_air_supply(id) {
                    let supply = (self.entity(id).air_supply() + 4).min(TOTAL_AIR_SUPPLY);
                    self.entity_mut(id).set_air_supply(supply);
                }
            } else if self.entity(id).air_supply() < TOTAL_AIR_SUPPLY {
                let supply = (self.entity(id).air_supply() + 4).min(TOTAL_AIR_SUPPLY);
                self.entity_mut(id).set_air_supply(supply);
            }
        }
        let player = self.is_player(id);
        let living = &mut self.living_mut(id);
        if living.hurt_time > 0 {
            living.hurt_time -= 1;
        }
        if living.damage_cooldown_time > 0 && !player {
            living.damage_cooldown_time -= 1;
        }
        if self.is_dead_or_dying(id) {
            self.tick_death(id);
        }
        let living = &mut self.living_mut(id);
        if living.last_hurt_by_player_memory_time > 0 {
            living.last_hurt_by_player_memory_time -= 1;
        } else {
            living.last_hurt_by_player = None;
        }
        if let Some(hurt_by) = self.living(id).last_hurt_by_mob.and_then(|uuid| self.entity_manager.visible_entity_storage.get(uuid)).filter(|&e| self.is_living(e)) {
            let alive = self.is_alive(hurt_by);
            let tick_count = self.entity(id).tick_count;
            let living = self.living_mut(id);
            if !alive || tick_count - living.last_hurt_by_mob_timestamp > 100 {
                living.last_hurt_by_mob = None;
                living.last_hurt_by_mob_timestamp = tick_count;
            }
        }
        self.tick_effects(id);
    }

    fn decrease_air_supply(&mut self, id: EntityId, supply: i32) -> i32 {
        let oxygen_bonus = self.attribute_value(id, Attribute::OxygenBonus);
        if oxygen_bonus > 0.0 && self.entity_mut(id).random.next_double() >= 1.0 / (oxygen_bonus + 1.0) { supply } else { supply - 1 }
    }

    fn tick_death(&mut self, id: EntityId) {
        let living = self.living_mut(id);
        living.death_time += 1;
        if living.death_time >= 20 && !self.entity(id).is_removed() {
            self.broadcast_entity_event(id, entity_event::POOF);
            self.remove(id, RemovalReason::Killed);
        }
    }

    pub fn remove(&mut self, id: EntityId, reason: RemovalReason) {
        if self.is_player(id) {
            self.player_remove(id, reason);
        } else {
            self.living_entity_remove(id, reason);
        }
    }

    pub fn living_entity_remove(&mut self, id: EntityId, reason: RemovalReason) {
        if self.entity(id).is_removed() {
            return;
        }
        if matches!(reason, RemovalReason::Killed | RemovalReason::Discarded) {
            self.trigger_on_death_mob_effects(id, reason);
        }
        self.entity_mut(id).removal = Some(reason);
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

    pub fn detect_equipment_updates(&mut self, id: EntityId) {
        let mut changed: Vec<(EquipmentSlot, Option<ItemStack>)> = Vec::new();
        for slot in EquipmentSlot::VALUES {
            let previous = self.living(id).last_equipment_items[slot as usize].clone();
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
                    if let Some(instance) = self.living_mut(id).attributes.instance(attribute) {
                        instance.remove_modifier(&modifier.id);
                        instance.add_transient_modifier(modifier);
                    }
                });
            }
        }
        let last = self.living(id).last_equipment_items.clone();
        let main = changed.iter().find(|c| c.0 == EquipmentSlot::Mainhand).map(|c| c.1.clone());
        let off = changed.iter().find(|c| c.0 == EquipmentSlot::Offhand).map(|c| c.1.clone());
        if let (Some(main), Some(off)) = (main, off)
            && main == last[EquipmentSlot::Offhand as usize]
            && off == last[EquipmentSlot::Mainhand as usize]
        {
            let packet = encode(clientbound::ENTITY_EVENT, |w| {
                w.i32(id);
                w.u8(entity_event::SWAP_HANDS);
            });
            self.send_to_tracking_players(id, &packet);
            changed.retain(|c| c.0 != EquipmentSlot::Mainhand && c.0 != EquipmentSlot::Offhand);
            let living = &mut self.living_mut(id);
            living.last_equipment_items[EquipmentSlot::Mainhand as usize] = main;
            living.last_equipment_items[EquipmentSlot::Offhand as usize] = off;
        }
        if changed.is_empty() {
            return;
        }
        let living = &mut self.living_mut(id);
        for (slot, current) in &changed {
            living.last_equipment_items[*slot as usize] = current.clone();
        }
        let packet = set_equipment(id, &changed);
        self.send_to_tracking_players(id, &packet);
    }

    pub fn stop_location_based_effects(&mut self, id: EntityId, previous: &ItemStack, slot: EquipmentSlot) {
        for_each_modifier(previous, slot, |attribute, modifier| {
            if let Some(instance) = self.living_mut(id).attributes.get_mut(attribute) {
                instance.remove_modifier(&modifier.id);
            }
        });
    }

    pub fn refresh_dirty_attributes(&mut self, id: EntityId) {
        let updated = self.living_mut(id).attributes.take_to_update();
        if updated & 1 << Attribute::MaxHealth as u64 != 0 {
            let max_health = self.max_health(id);
            if self.health(id) > max_health {
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

    pub fn ai_step(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.player_ai_step(id);
            return;
        }
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Chicken(_)) => self.chicken_ai_step(id),
            None => self.living_entity_ai_step(id),
        }
    }

    fn is_effective_ai(&self, id: EntityId) -> bool {
        self.mob(id).is_none_or(|mob| !mob.is_no_ai())
    }

    fn server_ai_step(&mut self, id: EntityId) {
        if self.is_mob(id) {
            self.mob_server_ai_step(id);
        }
    }

    pub fn living_entity_ai_step(&mut self, id: EntityId) {
        let player = self.is_player(id);
        let dead = self.is_dead_or_dying(id);
        let (e, living) = self.entity_and_living_mut(id);
        if living.no_jump_delay > 0 {
            living.no_jump_delay -= 1;
        }
        let movement = e.delta_movement;
        let (mut dx, mut dy, mut dz) = (movement.x, movement.y, movement.z);
        if player {
            if movement.horizontal_distance_sqr() < 9.0e-6 {
                dx = 0.0;
                dz = 0.0;
            }
        } else {
            if movement.x.abs() < 0.003 {
                dx = 0.0;
            }
            if movement.z.abs() < 0.003 {
                dz = 0.0;
            }
        }
        if movement.y.abs() < 0.003 {
            dy = 0.0;
        }
        e.set_delta_movement(Vec3::new(dx, dy, dz));
        living.xxa *= 0.98;
        living.zza *= 0.98;
        if dead {
            living.jumping = false;
            living.xxa = 0.0;
            living.zza = 0.0;
        } else if self.is_effective_ai(id) {
            self.server_ai_step(id);
        }
        let affected_by_fluids = self.is_affected_by_fluids(id);
        let (e, living) = self.entity_and_living(id);
        if living.jumping && affected_by_fluids {
            let fluid_height = if e.is_in_lava() { e.fluid_height(&fluid_tags::LAVA) } else { e.fluid_height(&fluid_tags::WATER) };
            let in_water_and_has_fluid_height = e.is_in_water() && fluid_height > 0.0;
            let threshold = e.fluid_jump_threshold();
            if !in_water_and_has_fluid_height || e.on_ground && !(fluid_height > threshold) {
                if !e.is_in_lava() || e.on_ground && e.fluid_height(&fluid_tags::LAVA) <= threshold {
                    if (e.on_ground || in_water_and_has_fluid_height && fluid_height <= threshold) && living.no_jump_delay == 0 {
                        self.jump_from_ground(id);
                        self.living_mut(id).no_jump_delay = 10;
                    }
                } else {
                    self.jump_in_liquid(id);
                }
            } else {
                self.jump_in_liquid(id);
            }
        } else {
            self.living_mut(id).no_jump_delay = 0;
        }
        let l = &self.living(id);
        let input = Vec3::new(l.xxa as f64, l.yya as f64, l.zza as f64);
        if l.has_effect(mob_effects::SLOW_FALLING) || l.has_effect(mob_effects::LEVITATION) {
            self.reset_fall_distance(id);
        }
        if self.is_effective_ai(id) {
            self.travel(id, input);
        }
        self.apply_effects_from_blocks(id);
        let e = self.entity_mut(id);
        if !e.is_in_powder_snow || !self.can_freeze(id) {
            let e = self.entity_mut(id);
            let ticks = (e.ticks_frozen() - 2).max(0);
            e.set_ticks_frozen(ticks);
        }
        self.remove_frost(id);
        self.try_add_frost(id);
        self.tick_freezing_damage(id);
        self.push_entities(id);
    }

    fn is_affected_by_fluids(&self, id: EntityId) -> bool {
        self.players.get(&id).is_none_or(|p| !p.abilities_flying())
    }

    fn is_pushable(&mut self, id: EntityId) -> bool {
        self.is_living(id) && self.is_alive(id) && !self.is_spectator(id) && !self.on_climbable(id)
    }

    fn pushable_by(&mut self, pusher: EntityId, other: EntityId) -> bool {
        let own_team = self.team_of(pusher).map(|t| (t.name().to_string(), t.collision_rule()));
        let own_rule = own_team.as_ref().map_or(CollisionRule::Always, |t| t.1);
        if own_rule == CollisionRule::Never || !self.is_pushable(other) {
            return false;
        }
        let their_team = self.team_of(other).map(|t| (t.name().to_string(), t.collision_rule()));
        let their_rule = their_team.as_ref().map_or(CollisionRule::Always, |t| t.1);
        if their_rule == CollisionRule::Never {
            return false;
        }
        let same_team = own_team.is_some() && own_team.as_ref().map(|t| &t.0) == their_team.as_ref().map(|t| &t.0);
        if (own_rule == CollisionRule::PushOwnTeam || their_rule == CollisionRule::PushOwnTeam) && same_team {
            return false;
        }
        own_rule != CollisionRule::PushOtherTeams && their_rule != CollisionRule::PushOtherTeams || same_team
    }

    fn push_entities(&mut self, id: EntityId) {
        let area = self.entity(id).bounding_box();
        let candidates = self.get_entities(Some(id), &area);
        let pushable: Vec<EntityId> = candidates.into_iter().filter(|&other| self.pushable_by(id, other)).collect();
        if pushable.is_empty() {
            return;
        }
        let max_cramming = self.game_rules.get_int(game_rule::MAX_ENTITY_CRAMMING);
        if max_cramming > 0 && pushable.len() as i32 > max_cramming - 1 && self.entity_mut(id).random.next_int_bounded(4) == 0 && pushable.len() as i32 > max_cramming - 1 {
            self.hurt_server(id, DamageSource::new(damage_types::CRAMMING), 6.0);
        }
        for other in pushable {
            self.push_entity(other, id);
        }
    }

    fn push_entity(&mut self, id: EntityId, other: EntityId) {
        if self.is_sleeping(id) {
            return;
        }
        let (e, o) = (self.entity(id), self.entity(other));
        if o.no_physics || e.no_physics {
            return;
        }
        let mut xa = o.x() - e.x();
        let mut za = o.z() - e.z();
        let mut dd = mth::abs_max(xa, za);
        if dd < 0.01f32 as f64 {
            return;
        }
        dd = dd.sqrt();
        xa /= dd;
        za /= dd;
        let pow = (1.0 / dd).min(1.0);
        xa *= pow;
        za *= pow;
        xa *= 0.05f32 as f64;
        za *= 0.05f32 as f64;
        if self.is_pushable(id) {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(-xa, 0.0, -za));
            e.needs_sync = true;
        }
        if self.is_pushable(other) {
            let o = self.entity_mut(other);
            o.set_delta_movement(o.delta_movement.add(xa, 0.0, za));
            o.needs_sync = true;
        }
    }

    fn remove_frost(&mut self, id: EntityId) {
        if let Some(speed) = self.living_mut(id).attributes.instance(Attribute::MovementSpeed)
            && speed.has_modifier(SPEED_MODIFIER_POWDER_SNOW_ID)
        {
            speed.remove_modifier(SPEED_MODIFIER_POWDER_SNOW_ID);
        }
    }

    fn try_add_frost(&mut self, id: EntityId) {
        let on = self.on_pos_legacy(id);
        if self.block_state(on).is_air() {
            return;
        }
        let ticks_frozen = self.entity(id).ticks_frozen();
        if ticks_frozen > 0 {
            let Some(speed) = self.living_mut(id).attributes.instance(Attribute::MovementSpeed) else { return };
            let percent_frozen = ticks_frozen.min(TICKS_REQUIRED_TO_FREEZE) as f32 / TICKS_REQUIRED_TO_FREEZE as f32;
            let slow_amount = -0.05f32 * percent_frozen;
            speed.add_transient_modifier(AttributeModifier {
                id: std::borrow::Cow::Borrowed(SPEED_MODIFIER_POWDER_SNOW_ID),
                amount: slow_amount as f64,
                operation: Operation::AddValue,
            });
        }
    }

    fn jump_in_liquid(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.add(0.0, 0.04f32 as f64, 0.0));
    }

    pub fn jump_from_ground(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.server_player_jump_from_ground(id);
        } else {
            self.living_entity_jump_from_ground(id);
        }
    }

    fn jump_boost_power(&self, id: EntityId) -> f32 {
        match self.living(id).effect(mob_effects::JUMP_BOOST) {
            Some(jump_boost) => 0.1 * (jump_boost.amplifier as f32 + 1.0),
            None => 0.0,
        }
    }

    pub fn living_entity_jump_from_ground(&mut self, id: EntityId) {
        let jump_power = self.attribute_value(id, Attribute::JumpStrength) as f32 * 1.0 * self.block_jump_factor(id) + self.jump_boost_power(id);
        if jump_power <= 1.0e-5 {
            return;
        }
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        e.set_delta_movement(Vec3::new(m.x, mth::jmax(jump_power as f64, m.y), m.z));
        if e.is_sprinting() {
            let angle = e.y_rot * mth::DEG_TO_RAD;
            e.add_delta_movement(Vec3::new(-mth::sin(angle as f64) as f64 * 0.2, 0.0, mth::cos(angle as f64) as f64 * 0.2));
        }
        e.needs_sync = true;
    }

    pub fn travel(&mut self, id: EntityId, input: Vec3) {
        if self.is_player(id) {
            self.player_travel(id, input);
        } else {
            self.living_entity_travel(id, input);
        }
    }

    pub fn living_entity_travel(&mut self, id: EntityId, input: Vec3) {
        if self.entity(id).is_in_liquid() && self.is_affected_by_fluids(id) {
            self.travel_in_fluid(id, input);
        } else {
            self.travel_in_air(id, input);
        }
    }

    fn travel_in_air(&mut self, id: EntityId, input: Vec3) {
        let pos_below = self.block_pos_below_that_affects_my_movement(id);
        let block_friction = if self.entity(id).on_ground {
            let friction = self.block_state(pos_below).block().info().friction;
            compute_modified_friction(friction, self.attribute_value(id, Attribute::FrictionModifier) as f32)
        } else {
            1.0
        };
        let movement = self.handle_relative_friction_and_calculate_movement(id, input, block_friction);
        let movement_y = match self.living(id).effect(mob_effects::LEVITATION) {
            Some(levitation) => movement.y + (0.05 * (levitation.amplifier + 1) as f64 - movement.y) * 0.2,
            None => movement.y - self.effective_gravity(id),
        };
        let air_drag_modifier = self.attribute_value(id, Attribute::AirDragModifier) as f32;
        let air_drag = compute_modified_friction(0.91, air_drag_modifier);
        let friction = block_friction * air_drag;
        let vertical_friction = compute_modified_friction(0.98, air_drag_modifier);
        let e = self.entity_mut(id);
        e.set_delta_movement(Vec3::new(movement.x * friction as f64, movement_y * vertical_friction as f64, movement.z * friction as f64));
    }

    fn travel_in_fluid(&mut self, id: EntityId, input: Vec3) {
        let e = self.entity(id);
        let is_falling = e.delta_movement.y <= 0.0;
        let old_y = e.y();
        let base_gravity = self.effective_gravity(id);
        if self.entity(id).is_in_water() {
            self.travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else {
            self.travel_in_lava(id, input, base_gravity, is_falling, old_y);
        }
    }

    fn travel_in_water(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        let e = self.entity(id);
        let mut slow_down = if e.is_sprinting() { 0.9f32 } else { 0.8 };
        let mut speed = 0.02f32;
        let mut water_walker = self.attribute_value(id, Attribute::WaterMovementEfficiency) as f32;
        if !e.on_ground {
            water_walker *= 0.5;
        }
        if water_walker > 0.0 {
            slow_down += (0.54600006 - slow_down) * water_walker;
            speed += (self.get_speed(id) - speed) * water_walker;
        }
        if self.living(id).has_effect(mob_effects::DOLPHINS_GRACE) {
            slow_down = 0.96;
        }
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let mut movement = self.entity(id).delta_movement;
        if self.entity(id).horizontal_collision && self.on_climbable(id) {
            movement = Vec3::new(movement.x, 0.2, movement.z);
        }
        let movement = movement.multiply(slow_down as f64, 0.8f32 as f64, slow_down as f64);
        let adjusted = self.fluid_falling_adjusted_movement(id, base_gravity, is_falling, movement);
        self.entity_mut(id).set_delta_movement(adjusted);
        self.jump_out_of_fluid(id, old_y);
    }

    fn travel_in_lava(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        self.entity_mut(id).move_relative(0.02, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity(id);
        if e.fluid_height(&fluid_tags::LAVA) <= e.fluid_jump_threshold() {
            let m = e.delta_movement.multiply(0.5, 0.8f32 as f64, 0.5);
            self.entity_mut(id).set_delta_movement(m);
            let adjusted = self.fluid_falling_adjusted_movement(id, base_gravity, is_falling, m);
            self.entity_mut(id).set_delta_movement(adjusted);
        } else {
            let m = e.delta_movement.scale(0.5);
            self.entity_mut(id).set_delta_movement(m);
        }
        if base_gravity != 0.0 {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -base_gravity / 4.0, 0.0));
        }
        self.jump_out_of_fluid(id, old_y);
    }

    fn jump_out_of_fluid(&mut self, id: EntityId, old_y: f64) {
        let e = self.entity(id);
        let m = e.delta_movement;
        if e.horizontal_collision && self.is_free(id, m.x, m.y + 0.6f32 as f64 - self.entity(id).y() + old_y, m.z) {
            self.entity_mut(id).set_delta_movement(Vec3::new(m.x, 0.3f32 as f64, m.z));
        }
    }

    fn fluid_falling_adjusted_movement(&self, id: EntityId, base_gravity: f64, is_falling: bool, movement: Vec3) -> Vec3 {
        if base_gravity != 0.0 && !self.entity(id).is_sprinting() {
            let yd = if is_falling && (movement.y - 0.005).abs() >= 0.003 && (movement.y - base_gravity / 16.0).abs() < 0.003 {
                -0.003
            } else {
                movement.y - base_gravity / 16.0
            };
            Vec3::new(movement.x, yd, movement.z)
        } else {
            movement
        }
    }

    fn handle_relative_friction_and_calculate_movement(&mut self, id: EntityId, input: Vec3, friction: f32) -> Vec3 {
        let speed = self.friction_influenced_speed(id, friction);
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        let delta = self.handle_on_climbable(id, delta);
        self.entity_mut(id).set_delta_movement(delta);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity(id);
        let mut movement = e.delta_movement;
        let jumping = self.living(id).jumping;
        if (e.horizontal_collision || jumping) && (self.on_climbable(id) || self.entity(id).was_in_powder_snow && self.can_entity_walk_on_powder_snow(id)) {
            movement = Vec3::new(movement.x, 0.2, movement.z);
        }
        movement
    }

    pub fn can_entity_walk_on_powder_snow(&self, id: EntityId) -> bool {
        entity_type_tags::POWDER_SNOW_WALKABLE_MOBS.contains(self.entity(id).entity_type as usize)
            || self.is_living(id) && self.item_by_slot_ref(id, EquipmentSlot::Feet).is_some_and(|s| s.item == items::LEATHER_BOOTS)
    }

    fn friction_influenced_speed(&self, id: EntityId, block_friction: f32) -> f32 {
        if self.entity(id).on_ground {
            let speed = self.get_speed(id);
            if block_friction as f64 > 0.6 { speed * (0.21600002 / (block_friction * block_friction * block_friction)) } else { speed }
        } else {
            self.get_flying_speed(id)
        }
    }

    fn get_flying_speed(&self, id: EntityId) -> f32 {
        if self.is_player(id) { self.player_get_flying_speed(id) } else { 0.02 }
    }

    pub fn get_speed(&self, id: EntityId) -> f32 {
        if self.is_player(id) { self.player_get_speed(id) } else { self.living(id).speed }
    }

    fn handle_on_climbable(&mut self, id: EntityId, delta: Vec3) -> Vec3 {
        if !self.on_climbable(id) {
            return delta;
        }
        self.reset_fall_distance(id);
        let max = 0.15f32 as f64;
        let xd = mth::clamp(delta.x, -max, max);
        let zd = mth::clamp(delta.z, -max, max);
        let mut yd = delta.y.max(-max);
        if yd < 0.0 && !self.in_block_state(id).is_block(blocks::SCAFFOLDING) && self.entity(id).is_shift_key_down() {
            yd = 0.0;
        }
        Vec3::new(xd, yd, zd)
    }

    pub fn on_climbable(&mut self, id: EntityId) -> bool {
        if self.is_spectator(id) {
            return false;
        }
        let pos = self.entity(id).block_position();
        let state = self.in_block_state(id);
        let climbable = state.is(&block_tags::CLIMBABLE)
            || state.block().is_instance_of("TrapDoorBlock") && state.get(OPEN) && {
                let below = self.block_state(pos.below());
                below.is_block(blocks::LADDER) && below.get(HORIZONTAL_FACING) == state.get(HORIZONTAL_FACING)
            };
        if climbable {
            self.living_mut(id).last_climbable_pos = Some(pos);
        }
        climbable
    }

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

    fn on_effect_added(&mut self, id: EntityId, effect: MobEffect, source: Option<EntityId>) {
        if self.is_player(id) {
            self.server_player_on_effect_added(id, effect, source);
        } else {
            self.living_entity_on_effect_added(id, effect, source);
        }
    }

    pub fn living_entity_on_effect_added(&mut self, id: EntityId, effect: MobEffect, _source: Option<EntityId>) {
        let living = self.living_mut(id);
        living.effects_dirty = true;
        let amplifier = living.effect(effect).expect("added effect").amplifier;
        add_attribute_modifiers(effect, &mut living.attributes, amplifier);
    }

    fn on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, source: Option<EntityId>) {
        if self.is_player(id) {
            self.server_player_on_effect_updated(id, effect, refresh_attributes, source);
        } else {
            self.living_entity_on_effect_updated(id, effect, refresh_attributes, source);
        }
    }

    pub fn living_entity_on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, _source: Option<EntityId>) {
        let living = &mut self.living_mut(id);
        living.effects_dirty = true;
        if refresh_attributes {
            let amplifier = living.effect(effect).expect("updated effect").amplifier;
            remove_attribute_modifiers(effect, &mut living.attributes);
            add_attribute_modifiers(effect, &mut living.attributes, amplifier);
            self.refresh_dirty_attributes(id);
        }
    }

    fn on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        if self.is_player(id) {
            self.server_player_on_effects_removed(id, effects);
        } else {
            self.living_entity_on_effects_removed(id, effects);
        }
    }

    pub fn living_entity_on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        let living = &mut self.living_mut(id);
        living.effects_dirty = true;
        for effect in effects {
            remove_attribute_modifiers(effect.effect, &mut living.attributes);
        }
        self.refresh_dirty_attributes(id);
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

    pub fn get_sleeping_pos(&self, id: EntityId) -> Option<BlockPos> {
        match self.entity(id).data.get(ids::SLEEPING_POS) {
            DataValue::OptionalBlockPos(pos) => *pos,
            _ => None,
        }
    }

    pub fn set_sleeping_pos(&mut self, id: EntityId, bed_position: BlockPos) {
        self.entity_mut(id).data.set(ids::SLEEPING_POS, DataValue::OptionalBlockPos(Some(bed_position)));
    }

    pub fn clear_sleeping_pos(&mut self, id: EntityId) {
        self.entity_mut(id).data.set(ids::SLEEPING_POS, DataValue::OptionalBlockPos(None));
    }

    pub fn is_sleeping(&self, id: EntityId) -> bool {
        self.is_living(id) && self.get_sleeping_pos(id).is_some()
    }

    pub fn living_entity_start_sleeping(&mut self, id: EntityId, bed_position: BlockPos) -> bool {
        let block_state = self.block_state(bed_position);
        if abstract_bed_block::kind(block_state.block()).is_none() {
            return false;
        }
        let Some(sleep_height) = abstract_bed_block::get_sleep_height(block_state, self, bed_position) else { return false };
        self.set_pos_to_bed(id, sleep_height, bed_position);
        self.set_block(bed_position, block_state.set(OCCUPIED, true), UPDATE_ALL);
        self.set_entity_pose(id, Pose::Sleeping);
        self.set_sleeping_pos(id, bed_position);
        let e = self.entity_mut(id);
        e.set_delta_movement(Vec3::ZERO);
        e.needs_sync = true;
        true
    }

    fn set_pos_to_bed(&mut self, id: EntityId, sleep_height: f64, bed_position: BlockPos) {
        self.entity_mut(id).set_pos(Vec3::new(bed_position.x as f64 + 0.5, bed_position.y as f64 + sleep_height + 0.125, bed_position.z as f64 + 0.5));
    }

    pub fn check_bed_exists(&mut self, id: EntityId) -> bool {
        let Some(bed_position) = self.get_sleeping_pos(id) else { return false };
        let state = self.block_state(bed_position);
        abstract_bed_block::kind(state.block()).is_some() && abstract_bed_block::get_sleep_height(state, self, bed_position).is_some()
    }

    pub fn living_entity_stop_sleeping(&mut self, id: EntityId) {
        if let Some(bed_position) = self.get_sleeping_pos(id).filter(|&pos| self.has_chunk_at(pos)) {
            let state = self.block_state(bed_position);
            if let Some(kind) = abstract_bed_block::kind(state.block()) {
                let facing = state.get(HORIZONTAL_FACING);
                self.set_block(bed_position, state.set(OCCUPIED, false), UPDATE_ALL);
                abstract_bed_block::on_stop_sleeping(kind, self, bed_position);
                let y_rot = self.entity(id).y_rot;
                let entity_type = self.dismount_type(id);
                let stand_up = abstract_bed_block::find_stand_up_position(entity_type, self, bed_position, facing, y_rot).unwrap_or_else(|| {
                    let above = bed_position.above();
                    Vec3::new(above.x as f64 + 0.5, above.y as f64 + 0.1, above.z as f64 + 0.5)
                });
                let look_direction = Vec3::at_bottom_center_of(bed_position).subtract(stand_up).normalize();
                let yaw = mth::wrap_degrees_d(mth::atan2(look_direction.z, look_direction.x) * (180.0f32 as f64) / std::f32::consts::PI as f64 - 90.0) as f32;
                let e = self.entity_mut(id);
                e.set_pos(stand_up);
                e.set_y_rot(yaw);
                e.set_x_rot(0.0);
            }
        }
        let pos = self.entity(id).position();
        self.set_entity_pose(id, Pose::Standing);
        self.entity_mut(id).set_pos(pos);
        self.clear_sleeping_pos(id);
    }

    pub fn stop_sleeping(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.stop_sleep_in_bed(id, true, true);
        } else {
            self.living_entity_stop_sleeping(id);
        }
    }

    pub fn dismount_type(&self, id: EntityId) -> crate::world::entity::vehicle::dismount_helper::DismountType {
        crate::world::entity::vehicle::dismount_helper::DismountType {
            dimensions: if self.is_player(id) { super::player::player::get_default_dimensions(Pose::Standing) } else { self.entity(id).dimensions },
            fire_immune: self.fire_immune(id),
            is_player: self.is_player(id),
        }
    }

    pub fn set_entity_pose(&mut self, id: EntityId, pose: Pose) {
        if self.is_player(id) {
            self.set_player_pose(id, pose);
        } else {
            self.entity_mut(id).set_pose(pose);
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::Vec3;
    use crisp_data::damage_types;

    use crate::server::level::{ServerLevel, testing};
    use crate::world::damagesource::DamageSource;
    use crate::world::entity::animal::chicken::Chicken;
    use crate::world::entity::{EntityId, LevelEntity};

    fn chicken(level: &mut ServerLevel, age: i32) -> EntityId {
        let mut mob = Chicken::create();
        mob.set_age(age);
        let id = mob.entity.id;
        assert!(level.add_fresh_entity(LevelEntity::Mob(Box::new(mob))));
        id
    }

    fn experience_orbs(level: &ServerLevel) -> usize {
        level.entities.values().filter(|e| matches!(e, LevelEntity::ExperienceOrb(_))).count()
    }

    #[test]
    fn held_weapons_apply_their_and_their_enchantments_attribute_modifiers() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let mut sword = crate::world::item::ItemStack::new(crisp_data::items::DIAMOND_SWORD, 1);
        crate::world::item::enchantment::enchantment_helper::enchant(&mut sword, "minecraft:sweeping_edge", 3);
        level.set_item_slot(player, crisp_data::EquipmentSlot::Mainhand, sword);
        level.detect_equipment_updates(player);
        assert_eq!(level.attribute_value(player, crate::world::entity::attributes::Attribute::AttackDamage), 7.0);
        assert_eq!(level.attribute_value(player, crate::world::entity::attributes::Attribute::SweepingDamageRatio), 0.75);
    }

    #[test]
    fn protection_enchantments_reduce_damage() {
        let damage_taken = |protection: i32| {
            let mut level = testing::level();
            let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
            let mut boots = crate::world::item::ItemStack::new(crisp_data::items::LEATHER_BOOTS, 1);
            crate::world::item::enchantment::enchantment_helper::enchant(&mut boots, "minecraft:protection", protection);
            level.set_item_slot(player, crisp_data::EquipmentSlot::Feet, boots);
            level.detect_equipment_updates(player);
            level.player_mut(player).movement.client_loaded_timeout_timer = 0;
            level.hurt_server(player, DamageSource::new(damage_types::MAGIC), 10.0);
            20.0 - level.health(player)
        };
        assert_eq!(damage_taken(0), 10.0);
        assert_eq!(damage_taken(4), 10.0 * (1.0 - 4.0 / 25.0));
    }

    #[test]
    fn player_attacks_knock_chickens_back() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let id = chicken(&mut level, 0);
        let attack = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        assert!(level.hurt_server(id, attack, 1.0));
        assert_eq!(level.health(id), 3.0);
        assert!(level.entity(id).needs_sync);
    }

    #[test]
    fn only_adult_chickens_killed_by_players_drop_experience() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let attack = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        let baby = chicken(&mut level, -24000);
        level.hurt_server(baby, attack, 100.0);
        assert!(level.living(baby).dead);
        assert_eq!(experience_orbs(&level), 0);
        let adult = chicken(&mut level, 0);
        level.hurt_server(adult, attack, 100.0);
        assert!(level.living(adult).dead);
        assert!(experience_orbs(&level) > 0);
    }
}
