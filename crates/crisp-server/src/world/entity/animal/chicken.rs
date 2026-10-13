use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{SyncedRegistry, item_tags, registry, registry_id};
use serde_json::Value;

use super::Animal;
use crate::server::level::ServerLevel;
use crate::world::entity::attributes::CHICKEN_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::sound_event;
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::variant::variant_utils::SpawnPrioritySelectors;
use crate::world::item::ItemStack;
use crate::world::level::pathfinder::path_type::PathType;

pub const DATA_VARIANT: usize = 18;
const CHICKEN_LAY: &str = "minecraft:gameplay/chicken_lay";
pub const DATA_SOUND_VARIANT: usize = 19;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.4, 0.7).with_eye_height(0.644).with_passenger_attachments(&[Vec3::new(0.0, 0.7, -0.1)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.3, 0.4).with_eye_height(0.28125).with_passenger_attachments(&[Vec3::new(0.0, 0.375, 0.0)]);

#[derive(Default)]
pub struct Chicken {
    pub animal: Animal,
    pub egg_time: i32,
    pub is_chicken_jockey: bool,
}

pub struct SoundSet {
    pub ambient_sound: String,
    pub death_sound: String,
    pub hurt_sound: String,
    pub step_sound: String,
}

pub struct ChickenSoundVariant {
    pub adult_sounds: SoundSet,
    pub baby_sounds: SoundSet,
}

fn variants() -> &'static SyncedRegistry {
    SyncedRegistry::get("minecraft:chicken_variant")
}

fn sound_variants() -> &'static SyncedRegistry {
    SyncedRegistry::get("minecraft:chicken_sound_variant")
}

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load("minecraft:chicken_variant", crisp_data::CHICKEN_VARIANTS))
}

pub fn sound_variant(id: i32) -> &'static ChickenSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<ChickenSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = crisp_data::CHICKEN_SOUND_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let set = |v: &Value| SoundSet {
            ambient_sound: v["ambient_sound"].as_str().unwrap_or_default().to_string(),
            death_sound: v["death_sound"].as_str().unwrap_or_default().to_string(),
            hurt_sound: v["hurt_sound"].as_str().unwrap_or_default().to_string(),
            step_sound: v["step_sound"].as_str().unwrap_or_default().to_string(),
        };
        sound_variants().entries.iter().map(|name| ChickenSoundVariant { adult_sounds: set(&json[name]["adult_sounds"]), baby_sounds: set(&json[name]["baby_sounds"]) }).collect()
    });
    &all[id as usize]
}

fn chicken_mut(mob: &mut Mob) -> &mut Chicken {
    match &mut mob.kind {
        MobKind::Chicken(chicken) => chicken,
        _ => unreachable!("not a chicken"),
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::CHICKEN_FOOD)
}

fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT) {
        DataValue::ChickenVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn variant_id(name: &str) -> Option<i32> {
    variants().id_of(name)
}

pub fn variant_name(id: i32) -> &'static str {
    variants().entries[id as usize]
}

pub fn sound_variant_id(name: &str) -> Option<i32> {
    sound_variants().id_of(name)
}

pub fn sound_variant_name(id: i32) -> &'static str {
    sound_variants().entries[id as usize]
}

impl Chicken {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        let temperate = variant_id("minecraft:temperate").unwrap_or(0);
        let classic = sound_variant_id("minecraft:classic").unwrap_or(0);
        data.extend([DataValue::ChickenVariant(temperate), DataValue::ChickenSoundVariant(classic)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:chicken"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&CHICKEN_ATTRIBUTES), MobKind::Chicken(Chicken::default()));
        Animal::init(&mut mob);
        let egg_time = mob.entity.random.next_int_bounded(6000) + 6000;
        chicken_mut(&mut mob).egg_time = egg_time;
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(1.4)));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.0, |i| i.get_item().is(&item_tags::CHICKEN_FOOD), false)));
        goal_selector.add_goal(4, Goal::FollowParent(FollowParentGoal::new(1.1)));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(7, Goal::RandomLookAround(RandomLookAroundGoal::default()));
    }
}

impl ServerLevel {
    pub fn chicken_ai_step(&mut self, id: EntityId) {
        self.animal_ai_step(id);
        if self.mob(id).is_none() {
            return;
        }
        let entity = self.entity_mut(id);
        let movement = entity.delta_movement;
        if !entity.on_ground && movement.y < 0.0 {
            entity.delta_movement = Vec3::new(movement.x, movement.y * 0.6, movement.z);
        }
        let alive = self.is_alive(id);
        let mob = self.mob_mut(id).expect("a mob");
        let baby = mob.is_baby();
        let chicken = chicken_mut(mob);
        if alive && !baby && !chicken.is_chicken_jockey && {
            chicken.egg_time -= 1;
            chicken.egg_time <= 0
        } {
            if self.drop_from_gift_loot_table(id, CHICKEN_LAY, |level, item_stack| {
                level.spawn_at_location(id, item_stack, 0.0);
            }) {
                let random = &mut self.entity_mut(id).random;
                let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
                self.entity_play_sound(id, sound_event("minecraft:entity.chicken.egg"), 1.0, pitch);
            }
            let mob = self.mob_mut(id).expect("a mob");
            let egg_time = mob.entity.random.next_int_bounded(6000) + 6000;
            chicken_mut(mob).egg_time = egg_time;
        }
    }

    pub fn chicken_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Chicken::create());
        if self.mob(partner).is_some_and(|mob| matches!(mob.kind, MobKind::Chicken(_))) {
            let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
            baby.entity.data.set(DATA_VARIANT, DataValue::ChickenVariant(variant));
        }
        Some(baby)
    }

    pub fn chicken_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        let is_chicken_jockey = matches!(&self.mob(id).expect("a chicken").kind, MobKind::Chicken(chicken) if chicken.is_chicken_jockey);
        if is_chicken_jockey { 10 } else { self.animal_get_base_experience_reward(id) }
    }

    pub fn select_chicken_variant_to_spawn(&mut self, pos: BlockPos) -> Option<i32> {
        self.select_variant_to_spawn(pos, spawn_selectors())
    }

    pub fn pick_random_chicken_sound_variant(&mut self) -> i32 {
        self.pick_random_sound_variant("minecraft:chicken_sound_variant")
    }
}
