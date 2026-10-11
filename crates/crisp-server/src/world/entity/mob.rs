use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::EquipmentSlot;
use crisp_protocol::nbt::Nbt;

use super::animal::chicken::{self, Chicken};
use super::ageable_mob::AgeableMob;
use super::ai::MobAi;
use super::animal::Animal;
use super::attributes::Attribute;
use super::data::{DataValue, ids};
use super::entity_save::uuid_from_nbt;
use super::living_entity::LivingEntity;
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::int_value;
use crate::world::level::storage::uuid_ints;

pub const DEFAULT_EQUIPMENT_DROP_CHANCE: f32 = 0.085;
const MOB_FLAG_NO_AI: i8 = 1;
const MOB_FLAG_LEFTHANDED: i8 = 2;

pub enum MobKind {
    Chicken(Chicken),
}

pub struct Mob {
    pub entity: Entity,
    pub living: LivingEntity,
    pub health: f32,
    pub absorption: f32,
    pub equipment: [Option<ItemStack>; 8],
    pub drop_chances: [f32; 8],
    pub ambient_sound_time: i32,
    pub xp_reward: i32,
    pub can_pick_up_loot: bool,
    pub persistence_required: bool,
    pub loot_table: Option<String>,
    pub loot_table_seed: i64,
    pub home_position: BlockPos,
    pub home_radius: i32,
    pub target: Option<EntityId>,
    pub ai: MobAi,
    pub kind: MobKind,
}

impl Mob {
    pub fn new(entity: Entity, living: LivingEntity, kind: MobKind) -> Mob {
        let mut mob = Mob {
            entity,
            living,
            health: 1.0,
            absorption: 0.0,
            equipment: Default::default(),
            drop_chances: [DEFAULT_EQUIPMENT_DROP_CHANCE; 8],
            ambient_sound_time: 0,
            xp_reward: 0,
            can_pick_up_loot: false,
            persistence_required: false,
            loot_table: None,
            loot_table_seed: 0,
            home_position: BlockPos::ZERO,
            home_radius: -1,
            target: None,
            ai: MobAi::default(),
            kind,
        };
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        mob.set_health(max_health);
        let e = &mut mob.entity;
        e.reapply_position();
        let y_rot = e.random.next_float() * std::f32::consts::TAU;
        e.set_y_rot(y_rot);
        e.y_head_rot = e.y_rot;
        mob
    }

    pub fn set_health(&mut self, health: f32) {
        self.health = crisp_core::mth::clamp_f(health, 0.0, self.living.attributes.value(Attribute::MaxHealth) as f32);
        self.entity.data.set(ids::HEALTH, DataValue::Float(self.health));
    }

    pub fn item_by_slot(&self, slot: EquipmentSlot) -> Option<&ItemStack> {
        self.equipment[slot as usize].as_ref().filter(|s| !s.is_empty())
    }

    fn mob_flags(&self) -> i8 {
        match self.entity.data.get(ids::MOB_FLAGS) {
            DataValue::Byte(flags) => *flags,
            _ => 0,
        }
    }

    fn set_mob_flag(&mut self, flag: i8, value: bool) {
        let flags = self.mob_flags();
        self.entity.data.set(ids::MOB_FLAGS, DataValue::Byte(if value { flags | flag } else { flags & !flag }));
    }

    pub fn is_no_ai(&self) -> bool {
        self.mob_flags() & MOB_FLAG_NO_AI != 0
    }

    pub fn set_no_ai(&mut self, no_ai: bool) {
        self.set_mob_flag(MOB_FLAG_NO_AI, no_ai);
    }

    pub fn is_left_handed(&self) -> bool {
        self.mob_flags() & MOB_FLAG_LEFTHANDED != 0
    }

    pub fn set_left_handed(&mut self, left_handed: bool) {
        self.set_mob_flag(MOB_FLAG_LEFTHANDED, left_handed);
    }

    pub fn is_baby(&self) -> bool {
        self.ageable().is_some_and(|a| a.age < 0)
    }

    pub fn cube_mob_size(&self) -> Option<i32> {
        match &self.kind {
            MobKind::Chicken(_) => None,
        }
    }

    pub fn raider(&self) -> Option<(bool, bool)> {
        match &self.kind {
            MobKind::Chicken(_) => None,
        }
    }

    pub fn sheep_sheared(&self) -> Option<bool> {
        match &self.kind {
            MobKind::Chicken(_) => None,
        }
    }

    pub fn ageable(&self) -> Option<&AgeableMob> {
        match &self.kind {
            MobKind::Chicken(chicken) => Some(&chicken.animal.ageable),
        }
    }

    pub fn ageable_mut(&mut self) -> Option<&mut AgeableMob> {
        match &mut self.kind {
            MobKind::Chicken(chicken) => Some(&mut chicken.animal.ageable),
        }
    }

    pub fn animal(&self) -> Option<&Animal> {
        match &self.kind {
            MobKind::Chicken(chicken) => Some(&chicken.animal),
        }
    }

    pub fn animal_mut(&mut self) -> Option<&mut Animal> {
        match &mut self.kind {
            MobKind::Chicken(chicken) => Some(&mut chicken.animal),
        }
    }

    pub fn is_age_locked(&self) -> bool {
        matches!(self.entity.data.get(ids::AGE_LOCKED), DataValue::Boolean(true))
    }

    pub fn set_age(&mut self, new_age: i32) -> bool {
        let Some(ageable) = self.ageable_mut() else { return false };
        let old_age = ageable.age;
        ageable.age = new_age;
        if old_age < 0 && new_age >= 0 || old_age >= 0 && new_age < 0 {
            self.entity.data.set(ids::AGEABLE_BABY, DataValue::Boolean(new_age < 0));
            return true;
        }
        false
    }

    pub fn default_dimensions(&self) -> EntityDimensions {
        match &self.kind {
            MobKind::Chicken(_) if self.is_baby() => chicken::BABY_DIMENSIONS,
            MobKind::Chicken(_) => chicken::DIMENSIONS,
        }
    }

    pub fn dimensions(&self) -> EntityDimensions {
        let scale = self.living.attributes.value(Attribute::Scale) as f32;
        let d = self.default_dimensions();
        EntityDimensions { width: d.width * scale, height: d.height * scale, eye_height: d.eye_height * scale }
    }

    pub fn loot_table_id(&self) -> Option<String> {
        if self.loot_table.is_some() {
            return self.loot_table.clone();
        }
        let id = crisp_data::registry::ENTITY_TYPE[self.entity.entity_type as usize];
        Some(format!("minecraft:entities/{}", id.trim_start_matches("minecraft:")))
    }

    pub fn save(&self, out: &mut Vec<(String, Nbt)>) {
        let living = &self.living;
        out.push(("Health".into(), Nbt::Float(self.health)));
        out.push(("HurtTime".into(), Nbt::Short(living.hurt_time as i16)));
        out.push(("DeathTime".into(), Nbt::Short(living.death_time as i16)));
        out.push(("AbsorptionAmount".into(), Nbt::Float(self.absorption)));
        out.push(("current_impulse_context_reset_grace_time".into(), Nbt::Int(living.current_impulse_context_reset_grace_time)));
        if let Some(pos) = living.current_impulse_impact_pos {
            out.push(("current_explosion_impact_pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        }
        out.push(("attributes".into(), living.attributes.to_nbt()));
        if !living.active_effects.is_empty() {
            out.push(("active_effects".into(), Nbt::List(living.active_effects.iter().map(|e| e.to_nbt()).collect())));
        }
        out.push(("FallFlying".into(), Nbt::Byte(self.entity.is_fall_flying() as i8)));
        if let DataValue::OptionalBlockPos(Some(pos)) = self.entity.data.get(ids::SLEEPING_POS) {
            out.push(("sleeping_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
        out.push(("Brain".into(), Nbt::Compound(vec![("memories".into(), Nbt::Compound(Vec::new()))])));
        if let Some(player) = living.last_hurt_by_player {
            out.push(("last_hurt_by_player".into(), uuid_ints(player)));
            out.push(("last_hurt_by_player_memory_time".into(), Nbt::Int(living.last_hurt_by_player_memory_time)));
        }
        if let Some(equipment) = self.save_equipment() {
            out.push(("equipment".into(), equipment));
        }
        out.push(("CanPickUpLoot".into(), Nbt::Byte(self.can_pick_up_loot as i8)));
        out.push(("PersistenceRequired".into(), Nbt::Byte(self.persistence_required as i8)));
        let drop_chances: Vec<(String, Nbt)> = EquipmentSlot::VALUES
            .into_iter()
            .filter(|&slot| self.drop_chances[slot as usize] != DEFAULT_EQUIPMENT_DROP_CHANCE)
            .map(|slot| (slot_name(slot).to_string(), Nbt::Float(self.drop_chances[slot as usize])))
            .collect();
        if !drop_chances.is_empty() {
            out.push(("drop_chances".into(), Nbt::Compound(drop_chances)));
        }
        if self.home_radius > -1 {
            out.push(("home_radius".into(), Nbt::Int(self.home_radius)));
            out.push(("home_pos".into(), Nbt::IntArray(vec![self.home_position.x, self.home_position.y, self.home_position.z])));
        }
        out.push(("LeftHanded".into(), Nbt::Byte(self.is_left_handed() as i8)));
        if let Some(table) = &self.loot_table {
            out.push(("DeathLootTable".into(), Nbt::String(table.clone())));
        }
        if self.loot_table_seed != 0 {
            out.push(("DeathLootTableSeed".into(), Nbt::Long(self.loot_table_seed)));
        }
        if self.is_no_ai() {
            out.push(("NoAI".into(), Nbt::Byte(1)));
        }
        if let Some(ageable) = self.ageable() {
            out.push(("Age".into(), Nbt::Int(ageable.age)));
            out.push(("ForcedAge".into(), Nbt::Int(ageable.forced_age)));
            out.push(("AgeLocked".into(), Nbt::Byte(self.is_age_locked() as i8)));
        }
        if let Some(animal) = self.animal() {
            out.push(("InLove".into(), Nbt::Int(animal.in_love)));
            if let Some(cause) = animal.love_cause {
                out.push(("LoveCause".into(), uuid_ints(cause)));
            }
        }
        match &self.kind {
            MobKind::Chicken(chicken) => {
                out.push(("IsChickenJockey".into(), Nbt::Byte(chicken.is_chicken_jockey as i8)));
                out.push(("EggLayTime".into(), Nbt::Int(chicken.egg_time)));
                if let DataValue::ChickenVariant(variant) = self.entity.data.get(chicken::DATA_VARIANT) {
                    out.push(("variant".into(), Nbt::String(chicken::variant_name(*variant).into())));
                }
                if let DataValue::ChickenSoundVariant(variant) = self.entity.data.get(chicken::DATA_SOUND_VARIANT) {
                    out.push(("sound_variant".into(), Nbt::String(chicken::sound_variant_name(*variant).into())));
                }
            }
        }
    }

    pub fn load(&mut self, nbt: &Nbt) {
        let float = |key: &str| nbt.get(key).and_then(Nbt::as_f64).map(|v| v as f32);
        let int = |key: &str| nbt.get(key).and_then(int_value);
        let boolean = |key: &str| nbt.get(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        let short = |key: &str| nbt.get(key).and_then(Nbt::as_i64).map_or(0, |v| v as i16 as i32);
        self.absorption = float("AbsorptionAmount").unwrap_or(0.0);
        if let Some(attributes) = nbt.get("attributes") {
            self.living.attributes.load(attributes);
        }
        self.living.active_effects.clear();
        for effect in nbt.get("active_effects").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(MobEffectInstance::from_nbt) {
            match self.living.active_effects.binary_search_by_key(&effect.effect, |e| e.effect) {
                Ok(index) => self.living.active_effects[index] = effect,
                Err(index) => self.living.active_effects.insert(index, effect),
            }
            self.living.effects_dirty = true;
        }
        let max_health = self.living.attributes.value(Attribute::MaxHealth) as f32;
        self.set_health(float("Health").unwrap_or(max_health));
        self.living.hurt_time = short("HurtTime");
        self.living.death_time = short("DeathTime");
        self.entity.set_shared_flag(super::flags::FALL_FLYING, boolean("FallFlying"));
        self.living.last_hurt_by_player = nbt.get("last_hurt_by_player").and_then(uuid_from_nbt);
        self.living.last_hurt_by_player_memory_time = int("last_hurt_by_player_memory_time").unwrap_or(0);
        self.load_equipment(nbt.get("equipment"));
        self.living.current_impulse_context_reset_grace_time = int("current_impulse_context_reset_grace_time").unwrap_or(0);
        self.living.current_impulse_impact_pos = match nbt.get("current_explosion_impact_pos") {
            Some(Nbt::List(v)) if v.len() == 3 => Some(crisp_core::Vec3::new(v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0))),
            _ => None,
        };
        self.can_pick_up_loot = boolean("CanPickUpLoot");
        self.persistence_required = boolean("PersistenceRequired");
        self.drop_chances = [DEFAULT_EQUIPMENT_DROP_CHANCE; 8];
        if let Some(Nbt::Compound(chances)) = nbt.get("drop_chances") {
            for (name, chance) in chances {
                if let (Some(slot), Some(chance)) = (EquipmentSlot::by_name(name), chance.as_f64()) {
                    self.drop_chances[slot as usize] = chance as f32;
                }
            }
        }
        self.home_radius = int("home_radius").unwrap_or(-1);
        if self.home_radius >= 0 {
            self.home_position = match nbt.get("home_pos") {
                Some(Nbt::IntArray(v)) if v.len() == 3 => BlockPos::new(v[0], v[1], v[2]),
                _ => BlockPos::ZERO,
            };
        }
        self.set_left_handed(boolean("LeftHanded"));
        self.loot_table = nbt.get("DeathLootTable").and_then(Nbt::as_str).map(str::to_string);
        self.loot_table_seed = nbt.get("DeathLootTableSeed").and_then(Nbt::as_i64).unwrap_or(0);
        self.set_no_ai(boolean("NoAI"));
        if self.ageable().is_some() {
            if self.set_age(int("Age").unwrap_or(0)) {
                self.refresh_dimensions();
            }
            let forced_age = int("ForcedAge").unwrap_or(0);
            self.ageable_mut().expect("ageable").forced_age = forced_age;
            self.entity.data.set(ids::AGE_LOCKED, DataValue::Boolean(boolean("AgeLocked")));
        }
        if let Some(animal) = self.animal_mut() {
            animal.in_love = int("InLove").unwrap_or(0);
            animal.love_cause = nbt.get("LoveCause").and_then(uuid_from_nbt);
        }
        let variant = nbt.get("variant").and_then(Nbt::as_str).and_then(chicken::variant_id);
        let sound_variant = nbt.get("sound_variant").and_then(Nbt::as_str).and_then(chicken::sound_variant_id);
        let egg_time = int("EggLayTime");
        let jockey = boolean("IsChickenJockey");
        match &mut self.kind {
            MobKind::Chicken(chicken) => {
                chicken.is_chicken_jockey = jockey;
                if let Some(egg_time) = egg_time {
                    chicken.egg_time = egg_time;
                }
                if let Some(variant) = variant {
                    self.entity.data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(variant));
                }
                if let Some(sound_variant) = sound_variant {
                    self.entity.data.set(chicken::DATA_SOUND_VARIANT, DataValue::ChickenSoundVariant(sound_variant));
                }
            }
        }
    }

    pub fn refresh_dimensions(&mut self) -> EntityDimensions {
        let old = self.entity.dimensions;
        self.entity.dimensions = self.dimensions();
        self.entity.reapply_position();
        old
    }

    pub fn sound_set(&self) -> &'static chicken::SoundSet {
        match &self.kind {
            MobKind::Chicken(_) => {
                let id = match self.entity.data.get(chicken::DATA_SOUND_VARIANT) {
                    DataValue::ChickenSoundVariant(id) => *id,
                    _ => 0,
                };
                let variant = chicken::sound_variant(id);
                if self.is_baby() { &variant.baby_sounds } else { &variant.adult_sounds }
            }
        }
    }

    pub fn max_head_y_rot(&self) -> i32 {
        75
    }

    pub fn max_head_x_rot(&self) -> i32 {
        40
    }

    pub fn head_rot_speed(&self) -> i32 {
        10
    }

    pub fn ambient_sound_interval(&self) -> i32 {
        match &self.kind {
            MobKind::Chicken(_) => 120,
        }
    }

    pub fn save_equipment(&self) -> Option<Nbt> {
        let slots: Vec<(String, Nbt)> = EquipmentSlot::VALUES
            .into_iter()
            .filter_map(|slot| self.item_by_slot(slot).map(|stack| (slot_name(slot).to_string(), stack.to_nbt())))
            .collect();
        (!slots.is_empty()).then_some(Nbt::Compound(slots))
    }

    pub fn load_equipment(&mut self, nbt: Option<&Nbt>) {
        self.equipment = Default::default();
        let Some(Nbt::Compound(slots)) = nbt else { return };
        for (name, stack) in slots {
            if let Some(slot) = EquipmentSlot::by_name(name)
                && let Ok(stack) = ItemStack::from_nbt(stack)
            {
                self.equipment[slot as usize] = Some(stack);
            }
        }
    }
}

pub fn slot_name(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Mainhand => "mainhand",
        EquipmentSlot::Offhand => "offhand",
        EquipmentSlot::Feet => "feet",
        EquipmentSlot::Legs => "legs",
        EquipmentSlot::Chest => "chest",
        EquipmentSlot::Head => "head",
        EquipmentSlot::Body => "body",
        EquipmentSlot::Saddle => "saddle",
    }
}

impl ServerLevel {
    pub fn mob(&self, id: EntityId) -> Option<&Mob> {
        match self.entities.get(&id) {
            Some(LevelEntity::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    pub fn mob_mut(&mut self, id: EntityId) -> Option<&mut Mob> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    pub fn is_mob(&self, id: EntityId) -> bool {
        self.mob(id).is_some()
    }

    pub fn mob_tick(&mut self, id: EntityId) {
        self.living_entity_tick(id);
    }

    pub fn mob_base_tick(&mut self, id: EntityId) {
        self.living_entity_base_tick(id);
        if self.mob(id).is_none() || !self.is_alive(id) {
            return;
        }
        let mob = self.mob_mut(id).expect("a mob");
        let ambient_sound_time = mob.ambient_sound_time;
        let roll = mob.entity.random.next_int_bounded(1000);
        mob.ambient_sound_time += 1;
        if roll < ambient_sound_time {
            mob.ambient_sound_time = -mob.ambient_sound_interval();
            let sound = super::sound_event(&mob.sound_set().ambient_sound);
            self.make_sound(id, sound);
        }
    }

    pub fn mob_ai_step(&mut self, id: EntityId) {
        self.living_entity_ai_step(id);
    }

    pub fn mob_server_ai_step(&mut self, id: EntityId) {
        self.mob_mut(id).expect("a mob").living.no_action_time += 1;
        self.custom_server_ai_step(id);
        self.move_control_tick(id);
        self.look_control_tick(id);
        self.jump_control_tick(id);
    }

    fn custom_server_ai_step(&mut self, id: EntityId) {
        if self.mob(id).is_some_and(|mob| mob.animal().is_some()) {
            self.animal_custom_server_ai_step(id);
        }
    }

    pub fn check_mob_despawn(&mut self, id: EntityId) {
        let mob = self.mob(id).expect("a mob");
        let persistent = mob.persistence_required;
        let removes_when_far_away = match &mob.kind {
            MobKind::Chicken(chicken) => chicken.is_chicken_jockey,
        };
        let position = mob.entity.position();
        let Some(player) = self.nearest_player_to_entity(position) else { return };
        let dist_sqr = self.entity(player).position().distance_to_sqr(position);
        let (despawn, no_despawn) = (128.0 * 128.0, 32.0 * 32.0);
        if !persistent && dist_sqr > despawn && removes_when_far_away {
            self.remove(id, super::RemovalReason::Discarded);
        }
        let mob = self.mob_mut(id).expect("a mob");
        if !persistent && mob.living.no_action_time > 600 && mob.entity.random.next_int_bounded(800) == 0 && dist_sqr > no_despawn && removes_when_far_away {
            self.remove(id, super::RemovalReason::Discarded);
        } else if dist_sqr < no_despawn {
            mob.living.no_action_time = 0;
        }
    }

    fn nearest_player_to_entity(&self, position: crisp_core::Vec3) -> Option<EntityId> {
        let mut best: Option<(EntityId, f64)> = None;
        for p in self.players.values().filter(|p| !self.is_spectator(p.entity.id)) {
            let distance = p.entity.position().distance_to_sqr(position);
            if best.is_none_or(|(_, b)| distance < b) {
                best = Some((p.entity.id, distance));
            }
        }
        best.map(|(id, _)| id)
    }

    pub fn finalize_mob_spawn(&mut self, mob: &mut Mob) {
        let pos = mob.entity.block_position();
        match mob.kind {
            MobKind::Chicken(_) => {
                if let Some(variant) = self.select_chicken_variant_to_spawn(pos) {
                    mob.entity.data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(variant));
                }
                let sound_variant = self.pick_random_chicken_sound_variant();
                mob.entity.data.set(chicken::DATA_SOUND_VARIANT, DataValue::ChickenSoundVariant(sound_variant));
            }
        }
        if let Some(follow_range) = mob.living.attributes.instance(Attribute::FollowRange)
            && !follow_range.has_modifier("minecraft:random_spawn_bonus")
        {
            follow_range.add_permanent_modifier(super::attributes::AttributeModifier {
                id: std::borrow::Cow::Borrowed("minecraft:random_spawn_bonus"),
                amount: self.random.triangle(0.0, 0.11485000000000001),
                operation: super::attributes::Operation::AddMultipliedBase,
            });
        }
        mob.set_left_handed(self.random.next_float() < 0.05);
    }
}
