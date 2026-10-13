use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{EquipmentSlot, items};
use crisp_protocol::nbt::Nbt;

use super::animal::chicken::{self, Chicken};
use super::animal::cow::abstract_cow::{self, AbstractCow};
use super::animal::cow::cow::{self, Cow};
use super::animal::cow::mushroom_cow::{self, MushroomCow};
use super::animal::pig::pig::{self, Pig};
use super::ageable_mob::{AgeableMob, BABY_START_AGE};
use super::conversion_params::ConversionParams;
use super::variant::variant_utils;
use super::ai::MobAi;
use super::ai::goal::goal::Flag;
use super::ai::navigation::path_navigation::PathNavigation;
use crate::world::level::pathfinder::path_type::PathType;
use super::animal::Animal;
use super::attributes::Attribute;
use super::data::{DataValue, ids};
use super::entity_save::uuid_from_nbt;
use super::ai::goal::goal_selector::GoalSelector;
use super::living_entity::{LivingEntity, get_comfortable_fall_distance};
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::item::{self, ItemStack, spawn_egg_item};
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::block::entity::int_value;
use crate::world::level::storage::uuid_ints;

pub const DEFAULT_EQUIPMENT_DROP_CHANCE: f32 = 0.085;
const MOB_FLAG_NO_AI: i8 = 1;
const MOB_FLAG_LEFTHANDED: i8 = 2;

pub enum MobKind {
    Chicken(Chicken),
    Cow(Cow),
    MushroomCow(MushroomCow),
    Pig(Pig),
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
    pathfinding_malus: [Option<f32>; PathType::COUNT],
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
            pathfinding_malus: [None; PathType::COUNT],
            kind,
        };
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        mob.set_health(max_health);
        let e = &mut mob.entity;
        e.reapply_position();
        let y_rot = e.random.next_float() * std::f32::consts::TAU;
        e.set_y_rot(y_rot);
        e.y_head_rot = e.y_rot;
        mob.ai.navigation = mob.create_navigation();
        mob.register_goals();
        mob
    }

    fn register_goals(&mut self) {
        match self.kind {
            MobKind::Chicken(_) => Chicken::register_goals(&mut self.ai),
            MobKind::Cow(_) | MobKind::MushroomCow(_) => AbstractCow::register_goals(&mut self.ai),
            MobKind::Pig(_) => Pig::register_goals(&mut self.ai),
        }
    }

    fn create_navigation(&self) -> PathNavigation {
        PathNavigation::new(self.living.attributes.base_value(Attribute::FollowRange))
    }

    pub fn get_pathfinding_malus(&self, path_type: PathType) -> f32 {
        self.pathfinding_malus[path_type as usize].unwrap_or_else(|| path_type.get_malus())
    }

    pub fn set_pathfinding_malus(&mut self, path_type: PathType, cost: f32) {
        self.pathfinding_malus[path_type as usize] = Some(cost);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.living.speed = speed;
        self.living.zza = speed;
    }

    pub fn get_max_head_x_rot(&self) -> i32 {
        40
    }

    fn update_control_flags(&mut self) {
        let goal_selector = &mut self.ai.goal_selector;
        goal_selector.set_control_flag(Flag::Move, true);
        goal_selector.set_control_flag(Flag::Jump, true);
        goal_selector.set_control_flag(Flag::Look, true);
    }

    pub fn get_head_rot_speed(&self) -> i32 {
        10
    }

    pub fn has_home(&self) -> bool {
        self.home_radius != -1
    }

    pub fn is_within_home(&self, pos: BlockPos) -> bool {
        self.home_radius == -1 || self.home_position.dist_sqr(pos) < (self.home_radius * self.home_radius) as f64
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
        None
    }

    pub fn raider(&self) -> Option<(bool, bool)> {
        None
    }

    pub fn sheep_sheared(&self) -> Option<bool> {
        None
    }

    pub fn ageable(&self) -> Option<&AgeableMob> {
        self.animal().map(|animal| &animal.ageable)
    }

    pub fn ageable_mut(&mut self) -> Option<&mut AgeableMob> {
        self.animal_mut().map(|animal| &mut animal.ageable)
    }

    pub fn animal(&self) -> Option<&Animal> {
        match &self.kind {
            MobKind::Chicken(chicken) => Some(&chicken.animal),
            MobKind::Cow(cow) => Some(&cow.abstract_cow.animal),
            MobKind::MushroomCow(mushroom_cow) => Some(&mushroom_cow.abstract_cow.animal),
            MobKind::Pig(pig) => Some(&pig.animal),
        }
    }

    pub fn animal_mut(&mut self) -> Option<&mut Animal> {
        match &mut self.kind {
            MobKind::Chicken(chicken) => Some(&mut chicken.animal),
            MobKind::Cow(cow) => Some(&mut cow.abstract_cow.animal),
            MobKind::MushroomCow(mushroom_cow) => Some(&mut mushroom_cow.abstract_cow.animal),
            MobKind::Pig(pig) => Some(&mut pig.animal),
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

    pub fn set_baby(&mut self, baby: bool) {
        if self.ageable().is_some() && self.set_age(if baby { BABY_START_AGE } else { 0 }) {
            self.refresh_dimensions();
        }
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        self.apply_implicit_components(stack);
    }

    fn apply_implicit_components(&mut self, components: &impl DataComponentGetter) {
        match self.kind {
            MobKind::Chicken(_) => {
                self.apply_implicit_component_if_present(components, dc::CHICKEN_VARIANT);
                self.apply_implicit_component_if_present(components, dc::CHICKEN_SOUND_VARIANT);
            }
            MobKind::Cow(_) => {
                self.apply_implicit_component_if_present(components, dc::COW_VARIANT);
                self.apply_implicit_component_if_present(components, dc::COW_SOUND_VARIANT);
            }
            MobKind::MushroomCow(_) => {
                self.apply_implicit_component_if_present(components, dc::MOOSHROOM_VARIANT);
            }
            MobKind::Pig(_) => {
                self.apply_implicit_component_if_present(components, dc::PIG_VARIANT);
                self.apply_implicit_component_if_present(components, dc::PIG_SOUND_VARIANT);
            }
        }
        self.apply_implicit_component_if_present(components, dc::CUSTOM_NAME);
        self.apply_implicit_component_if_present(components, dc::CUSTOM_DATA);
    }

    fn apply_implicit_component(&mut self, ty: DataComponentType, value: &Nbt) -> bool {
        match self.kind {
            MobKind::Chicken(_) if ty == dc::CHICKEN_VARIANT => {
                if let Some(variant) = value.as_str().and_then(chicken::variant_id) {
                    self.entity.data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(variant));
                }
                return true;
            }
            MobKind::Chicken(_) if ty == dc::CHICKEN_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(chicken::sound_variant_id) {
                    self.entity.data.set(chicken::DATA_SOUND_VARIANT, DataValue::ChickenSoundVariant(sound_variant));
                }
                return true;
            }
            MobKind::Cow(_) if ty == dc::COW_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(cow::VARIANT_REGISTRY, name)) {
                    cow::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Cow(_) if ty == dc::COW_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(|name| variant_utils::read_variant(cow::SOUND_VARIANT_REGISTRY, name)) {
                    cow::set_sound_variant(&mut self.entity, sound_variant);
                }
                return true;
            }
            MobKind::MushroomCow(_) if ty == dc::MOOSHROOM_VARIANT => {
                if let Some(variant) = value.as_str().and_then(mushroom_cow::Variant::by_name) {
                    mushroom_cow::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Pig(_) if ty == dc::PIG_VARIANT => {
                if let Some(variant) = value.as_str().and_then(|name| variant_utils::read_variant(pig::VARIANT_REGISTRY, name)) {
                    pig::set_variant(&mut self.entity, variant);
                }
                return true;
            }
            MobKind::Pig(_) if ty == dc::PIG_SOUND_VARIANT => {
                if let Some(sound_variant) = value.as_str().and_then(|name| variant_utils::read_variant(pig::SOUND_VARIANT_REGISTRY, name)) {
                    pig::set_sound_variant(&mut self.entity, sound_variant);
                }
                return true;
            }
            _ => {}
        }
        self.entity.apply_implicit_component(ty, value)
    }

    fn apply_implicit_component_if_present(&mut self, components: &impl DataComponentGetter, ty: DataComponentType) -> bool {
        match components.get(ty) {
            Some(value) => self.apply_implicit_component(ty, &value),
            None => false,
        }
    }

    pub fn default_dimensions(&self) -> EntityDimensions {
        match &self.kind {
            MobKind::Chicken(_) if self.is_baby() => chicken::BABY_DIMENSIONS,
            MobKind::Chicken(_) => chicken::DIMENSIONS,
            MobKind::Cow(_) if self.is_baby() => cow::BABY_DIMENSIONS,
            MobKind::MushroomCow(_) if self.is_baby() => mushroom_cow::BABY_DIMENSIONS,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => abstract_cow::DIMENSIONS,
            MobKind::Pig(_) if self.is_baby() => pig::BABY_DIMENSIONS,
            MobKind::Pig(_) => pig::DIMENSIONS,
        }
    }

    pub fn passenger_attachment(&self) -> crisp_core::Vec3 {
        let baby = self.is_baby();
        let (x, y, z) = match &self.kind {
            MobKind::Chicken(_) if baby => (0.0, 0.375f32 as f64, 0.0),
            MobKind::Chicken(_) => (0.0, 0.7, -0.1),
            MobKind::Cow(_) | MobKind::MushroomCow(_) if baby => (0.0, 0.75f32 as f64, 0.0),
            MobKind::Cow(_) | MobKind::MushroomCow(_) => (0.0, 1.36875f32 as f64, 0.0),
            MobKind::Pig(_) if baby => (0.0, 0.5f32 as f64, 0.0),
            MobKind::Pig(_) => (0.0, 0.86875f32 as f64, 0.0),
        };
        let scale = self.living.attributes.value(Attribute::Scale) as f32 as f64;
        crisp_core::Vec3::new(x * scale, y * scale, z * scale)
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
        if living.locator_bar_icon.has_data() {
            out.push(("locator_bar_icon".into(), living.locator_bar_icon.to_nbt()));
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
            MobKind::Cow(_) => {
                out.push(("variant".into(), Nbt::String(variant_utils::variant_name(cow::VARIANT_REGISTRY, cow::get_variant(&self.entity)).into())));
                out.push(("sound_variant".into(), Nbt::String(variant_utils::variant_name(cow::SOUND_VARIANT_REGISTRY, cow::get_sound_variant(&self.entity)).into())));
            }
            MobKind::MushroomCow(mushroom_cow) => {
                out.push(("Type".into(), Nbt::String(mushroom_cow::get_variant(&self.entity).get_serialized_name().into())));
                if let Some(stew_effects) = &mushroom_cow.stew_effects {
                    out.push(("stew_effects".into(), stew_effects.clone()));
                }
            }
            MobKind::Pig(_) => {
                out.push(("variant".into(), Nbt::String(variant_utils::variant_name(pig::VARIANT_REGISTRY, pig::get_variant(&self.entity)).into())));
                out.push(("sound_variant".into(), Nbt::String(variant_utils::variant_name(pig::SOUND_VARIANT_REGISTRY, pig::get_sound_variant(&self.entity)).into())));
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
        self.living.locator_bar_icon = nbt.get("locator_bar_icon").and_then(crate::world::waypoints::waypoint::Icon::from_nbt).unwrap_or_default();
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
        let read = |key: &str, registry: &str| nbt.get(key).and_then(Nbt::as_str).and_then(|name| variant_utils::read_variant(registry, name));
        match &mut self.kind {
            MobKind::Cow(_) => {
                if let Some(variant) = read("variant", cow::VARIANT_REGISTRY) {
                    cow::set_variant(&mut self.entity, variant);
                }
                if let Some(sound_variant) = read("sound_variant", cow::SOUND_VARIANT_REGISTRY) {
                    cow::set_sound_variant(&mut self.entity, sound_variant);
                }
            }
            MobKind::MushroomCow(mushroom_cow) => {
                let variant = nbt.get("Type").and_then(Nbt::as_str).and_then(mushroom_cow::Variant::by_name).unwrap_or(mushroom_cow::Variant::DEFAULT);
                mushroom_cow::set_variant(&mut self.entity, variant);
                mushroom_cow.stew_effects = nbt.get("stew_effects").filter(|effects| matches!(effects, Nbt::List(_))).cloned();
            }
            MobKind::Pig(_) => {
                if let Some(variant) = read("variant", pig::VARIANT_REGISTRY) {
                    pig::set_variant(&mut self.entity, variant);
                }
                if let Some(sound_variant) = read("sound_variant", pig::SOUND_VARIANT_REGISTRY) {
                    pig::set_sound_variant(&mut self.entity, sound_variant);
                }
            }
            MobKind::Chicken(chicken) => {
                let variant = nbt.get("variant").and_then(Nbt::as_str).and_then(chicken::variant_id);
                let sound_variant = nbt.get("sound_variant").and_then(Nbt::as_str).and_then(chicken::sound_variant_id);
                chicken.is_chicken_jockey = boolean("IsChickenJockey");
                if let Some(egg_time) = int("EggLayTime") {
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

    fn chicken_sound_set(&self) -> &'static chicken::SoundSet {
        let id = match self.entity.data.get(chicken::DATA_SOUND_VARIANT) {
            DataValue::ChickenSoundVariant(id) => *id,
            _ => 0,
        };
        let variant = chicken::sound_variant(id);
        if self.is_baby() { &variant.baby_sounds } else { &variant.adult_sounds }
    }

    fn cow_sound_set(&self) -> &'static abstract_cow::CowSoundVariant {
        match &self.kind {
            MobKind::Cow(_) => cow::sound_variant(cow::get_sound_variant(&self.entity)),
            _ => mushroom_cow::get_sound_set(),
        }
    }

    pub fn get_ambient_sound(&self) -> &'static str {
        match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().ambient_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().ambient_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).ambient_sound,
        }
    }

    pub fn get_hurt_sound(&self) -> &'static str {
        match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().hurt_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().hurt_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).hurt_sound,
        }
    }

    pub fn get_death_sound(&self) -> &'static str {
        match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().death_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().death_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).death_sound,
        }
    }

    pub fn get_step_sound(&self) -> &'static str {
        match &self.kind {
            MobKind::Chicken(_) => &self.chicken_sound_set().step_sound,
            MobKind::Cow(_) | MobKind::MushroomCow(_) => &self.cow_sound_set().step_sound,
            MobKind::Pig(_) => &Pig::get_sound_set(self).step_sound,
        }
    }

    pub fn get_sound_volume(&self) -> f32 {
        match &self.kind {
            MobKind::Cow(_) | MobKind::MushroomCow(_) => 0.4,
            _ => 1.0,
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
        if self.animal().is_some() { 120 } else { 80 }
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

    pub fn check_and_handle_important_interactions(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::NAME_TAG) {
            let name_tag_interaction_result = item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if name_tag_interaction_result.consumes_action() {
                return name_tag_interaction_result;
            }
        }
        if item_stack.get_item().is_instance_of("SpawnEggItem") {
            let entity = self.entity(id);
            let (entity_type, position) = (entity.entity_type, entity.position());
            let offspring = spawn_egg_item::spawn_offspring_from_spawn_egg(self, player, id, entity_type, position, &mut item_stack);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if offspring.is_none() {
                return InteractionResult::Pass;
            }
            InteractionResult::SUCCESS_SERVER
        } else {
            InteractionResult::Pass
        }
    }

    pub fn mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        match self.mob(id).expect("a mob").kind {
            MobKind::Chicken(_) => self.animal_mob_interact(id, player, hand),
            MobKind::Cow(_) => self.abstract_cow_mob_interact(id, player, hand),
            MobKind::MushroomCow(_) => self.mushroom_cow_mob_interact(id, player, hand),
            MobKind::Pig(_) => self.pig_mob_interact(id, player, hand),
        }
    }

    pub fn mob_mob_interact(&mut self, _id: EntityId, _player: EntityId, _hand: InteractionHand) -> InteractionResult {
        InteractionResult::Pass
    }

    pub fn is_saddled(&self, id: EntityId) -> bool {
        self.has_valid_equippable_item_for_slot(id, EquipmentSlot::Saddle)
    }

    fn has_valid_equippable_item_for_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        self.item_by_slot(id, slot).is_some_and(|item_stack| self.is_equippable_in_slot(id, &item_stack, slot))
    }

    pub fn set_guaranteed_drop(&mut self, id: EntityId, slot: EquipmentSlot) {
        self.mob_mut(id).expect("a mob").drop_chances[slot as usize] = 2.0;
    }

    pub fn can_shear_equipment(&self, id: EntityId) -> bool {
        !self.is_vehicle(id)
    }

    pub fn attempt_to_shear_equipment(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> bool {
        for slot in EquipmentSlot::VALUES {
            let Some(item_stack) = self.item_by_slot(id, slot) else { continue };
            let Some(equippable) = item_stack.get(dc::EQUIPPABLE).map(item::component::values::Equippable::from_nbt) else { continue };
            let creative = self.players.get(&player).is_some_and(|p| p.game_mode.game_type.is_creative());
            if equippable.can_be_sheared && (!crate::world::item::enchantment::enchantment_helper::has_prevent_armor_change(&item_stack) || creative) {
                self.shear_item(id, player, hand, slot, item_stack);
                let sound = equippable.shearing_sound.as_str().map_or(super::sound_event("minecraft:item.shears.snip"), super::sound_event);
                self.entity_play_sound(id, sound, 1.0, 1.0);
                return true;
            }
        }
        false
    }

    fn shear_item(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, slot: EquipmentSlot, item_stack_to_shear: ItemStack) {
        self.hurt_and_break(player, hand.as_equipment_slot(), 1);
        let equipment_spawn_offset = self.mob(id).expect("a mob").passenger_attachment();
        self.living_set_item_slot(id, slot, ItemStack::empty());
        self.spawn_at_location_with_offset(id, item_stack_to_shear.clone(), equipment_spawn_offset);
        crate::advancements::triggers::criteria_triggers::PLAYER_SHEARED_EQUIPMENT.trigger(self, player, &item_stack_to_shear, id);
    }

    pub fn convert_to(&mut self, id: EntityId, entity_type: &str, params: ConversionParams, after_conversion: impl FnOnce(&mut ServerLevel, EntityId)) -> Option<EntityId> {
        if self.entity(id).is_removed() {
            return None;
        }
        let LevelEntity::Mob(new_mob) = ServerLevel::create_entity(entity_type)? else { return None };
        let new_id = new_mob.entity.id;
        self.entities.insert(new_id, LevelEntity::Mob(new_mob));
        params.ty.convert(self, id, new_id, &params);
        after_conversion(self, new_id);
        let new_mob = self.entities.shift_remove(&new_id).expect("the converted mob");
        self.add_fresh_entity(new_mob);
        if params.ty.should_discard_after_conversion() {
            self.remove(id, super::RemovalReason::Discarded);
        }
        Some(new_id)
    }

    pub fn mob_drop_custom_death_loot(&mut self, id: EntityId, killed_by_player: bool) {
        for slot in EquipmentSlot::VALUES {
            let item_stack = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
            let drop_chance = self.mob(id).expect("a mob").drop_chances[slot as usize];
            if drop_chance != 0.0 {
                let preserve = drop_chance > 1.0;
                if !item_stack.is_empty()
                    && !crate::world::item::enchantment::enchantment_helper::has_prevent_equipment_drop(&item_stack)
                    && (killed_by_player || preserve)
                    && self.entity_mut(id).random.next_float() < drop_chance
                {
                    let mut item_stack = item_stack;
                    if !preserve && item_stack.is_damageable_item() {
                        let max_damage = item_stack.max_damage();
                        let random = &mut self.entity_mut(id).random;
                        let bound = random.next_int_bounded((max_damage - 3).max(1));
                        let damage = max_damage - random.next_int_bounded(1 + bound);
                        item_stack.set_damage_value(damage);
                    }
                    self.spawn_at_location(id, item_stack, 0.0);
                    self.set_item_slot(id, slot, ItemStack::empty());
                }
            }
        }
    }

    pub fn use_player_item(&mut self, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) {
        let before_use_count = item_stack.count;
        let use_remainder = item_stack.get(dc::USE_REMAINDER).map(ItemStackTemplate::from_nbt);
        item_stack.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, item_stack);
        if let Some(use_remainder) = use_remainder {
            let new_hand_stack = if self.has_infinite_materials(player) || item_stack.count >= before_use_count {
                item_stack.clone()
            } else {
                let remainder_stack = use_remainder.create().unwrap_or_else(ItemStack::empty);
                if item_stack.is_empty() {
                    remainder_stack
                } else {
                    self.handle_extra_items_created_on_use(player, remainder_stack);
                    item_stack.clone()
                }
            };
            self.player_mut(player).update_item_in_hand(hand, &new_hand_stack);
        }
    }

    pub fn mob_tick(&mut self, id: EntityId) {
        self.living_entity_tick(id);
        if let Some(mob) = self.mob_mut(id)
            && mob.entity.tick_count % 5 == 0
        {
            mob.update_control_flags();
        }
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
            self.play_ambient_sound(id);
        }
    }

    pub fn play_ambient_sound(&mut self, id: EntityId) {
        let sound = super::sound_event(self.mob(id).expect("a mob").get_ambient_sound());
        self.make_sound(id, sound);
    }

    pub fn mob_ai_step(&mut self, id: EntityId) {
        self.living_entity_ai_step(id);
    }

    pub fn mob_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a mob");
        mob.living.no_action_time += 1;
        mob.ai.sensing.tick();
        let id_based_tick_count = mob.entity.tick_count.wrapping_add(id);
        if id_based_tick_count % 2 != 0 && mob.entity.tick_count > 1 {
            self.with_target_selector(id, |selector, level| selector.tick_running_goals(level, id, false));
            self.with_goal_selector(id, |selector, level| selector.tick_running_goals(level, id, false));
        } else {
            self.with_target_selector(id, |selector, level| selector.tick(level, id));
            self.with_goal_selector(id, |selector, level| selector.tick(level, id));
        }
        if self.mob(id).is_none() {
            return;
        }
        self.get_navigation(id, |navigation, level| navigation.tick(level, id));
        self.custom_server_ai_step(id);
        self.move_control_tick(id);
        self.look_control_tick(id);
        self.jump_control_tick(id);
    }

    fn with_goal_selector(&mut self, id: EntityId, f: impl FnOnce(&mut GoalSelector, &mut ServerLevel)) {
        let Some(mob) = self.mob_mut(id) else { return };
        let mut selector = std::mem::take(&mut mob.ai.goal_selector);
        f(&mut selector, self);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.goal_selector = selector;
        }
    }

    fn with_target_selector(&mut self, id: EntityId, f: impl FnOnce(&mut GoalSelector, &mut ServerLevel)) {
        let Some(mob) = self.mob_mut(id) else { return };
        let mut selector = std::mem::take(&mut mob.ai.target_selector);
        f(&mut selector, self);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.target_selector = selector;
        }
    }

    pub fn get_navigation<R>(&mut self, id: EntityId, f: impl FnOnce(&mut PathNavigation, &mut ServerLevel) -> R) -> R {
        let mut navigation = std::mem::take(&mut self.mob_mut(id).expect("a mob").ai.navigation);
        let result = f(&mut navigation, self);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.navigation = navigation;
        }
        result
    }

    pub fn get_max_fall_distance(&self, id: EntityId) -> i32 {
        let mob = self.mob(id).expect("a mob");
        if mob.target.is_none() {
            return get_comfortable_fall_distance(0.0);
        }
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        let mut sacrifice = (mob.health - max_health * 0.33) as i32;
        sacrifice -= (3 - self.difficulty as i32) * 4;
        if sacrifice < 0 {
            sacrifice = 0;
        }
        get_comfortable_fall_distance(sacrifice as f32)
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
            _ => false,
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
            MobKind::Cow(_) => self.cow_finalize_spawn(mob, pos),
            MobKind::MushroomCow(_) => {}
            MobKind::Pig(_) => self.pig_finalize_spawn(mob, pos),
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
