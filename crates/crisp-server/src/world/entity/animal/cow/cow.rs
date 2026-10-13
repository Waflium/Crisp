use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{registry, registry_id};
use serde_json::Value;

use super::abstract_cow::{AbstractCow, CowSoundVariant};
use crate::server::level::ServerLevel;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::COW_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityDimensions, EntityId};

pub const DATA_VARIANT_ID: usize = 18;
pub const DATA_SOUND_VARIANT_ID: usize = 19;
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.45, 0.7).with_eye_height(0.69).with_passenger_attachments(&[crisp_core::Vec3::new(0.0, 0.75, 0.0)]);
pub const VARIANT_REGISTRY: &str = "minecraft:cow_variant";
pub const SOUND_VARIANT_REGISTRY: &str = "minecraft:cow_sound_variant";

#[derive(Default)]
pub struct Cow {
    pub abstract_cow: AbstractCow,
}

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, crisp_data::COW_VARIANTS))
}

pub fn sound_variant(id: i32) -> &'static CowSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<CowSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = crisp_data::COW_SOUND_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        crisp_data::SyncedRegistry::get(SOUND_VARIANT_REGISTRY).entries.iter().map(|name| classic_sounds_from(&json[name])).collect()
    });
    &all[id as usize]
}

pub fn classic_sounds_from(v: &Value) -> CowSoundVariant {
    CowSoundVariant {
        ambient_sound: v["ambient_sound"].as_str().unwrap_or_default().to_string(),
        hurt_sound: v["hurt_sound"].as_str().unwrap_or_default().to_string(),
        death_sound: v["death_sound"].as_str().unwrap_or_default().to_string(),
        step_sound: v["step_sound"].as_str().unwrap_or_default().to_string(),
    }
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::CowVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn get_sound_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_SOUND_VARIANT_ID) {
        DataValue::CowSoundVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::CowVariant(variant));
}

pub fn set_sound_variant(entity: &mut Entity, sound_variant: i32) {
    entity.data.set(DATA_SOUND_VARIANT_ID, DataValue::CowSoundVariant(sound_variant));
}

impl Cow {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        let temperate = variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:temperate").unwrap_or(0);
        let classic = variant_utils::read_variant(SOUND_VARIANT_REGISTRY, "minecraft:classic").unwrap_or(0);
        data.extend([DataValue::CowVariant(temperate), DataValue::CowSoundVariant(classic)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:cow"), super::abstract_cow::DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&COW_ATTRIBUTES), MobKind::Cow(Cow::default()));
        Animal::init(&mut mob);
        mob
    }
}

impl ServerLevel {
    pub fn cow_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Cow::create());
        if self.mob(partner).is_some_and(|mob| matches!(mob.kind, MobKind::Cow(_))) {
            let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
            set_variant(&mut baby.entity, variant);
        }
        Some(baby)
    }

    pub fn cow_finalize_spawn(&mut self, mob: &mut Mob, pos: BlockPos) {
        if let Some(variant) = self.select_variant_to_spawn(pos, spawn_selectors()) {
            set_variant(&mut mob.entity, variant);
        }
        let sound_variant = self.pick_random_sound_variant(SOUND_VARIANT_REGISTRY);
        set_sound_variant(&mut mob.entity, sound_variant);
    }
}
