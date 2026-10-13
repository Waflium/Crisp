use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{EquipmentSlot, item_tags, items, registry, registry_id};
use serde_json::Value;

use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
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
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::PIG_ATTRIBUTES;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityDimensions, EntityId};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_VARIANT_ID: usize = 19;
pub const DATA_SOUND_VARIANT_ID: usize = 20;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 0.9).with_passenger_attachments(&[crisp_core::Vec3::new(0.0, 0.86875, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.45, 0.45).with_eye_height(0.40625).with_passenger_attachments(&[crisp_core::Vec3::new(0.0, 0.5, 0.0)]);
pub const VARIANT_REGISTRY: &str = "minecraft:pig_variant";
pub const SOUND_VARIANT_REGISTRY: &str = "minecraft:pig_sound_variant";

#[derive(Default)]
pub struct Pig {
    pub animal: Animal,
}

pub struct PigSoundSet {
    pub ambient_sound: String,
    pub hurt_sound: String,
    pub death_sound: String,
    pub step_sound: String,
    pub eat_sound: String,
}

pub struct PigSoundVariant {
    pub adult_sounds: PigSoundSet,
    pub baby_sounds: PigSoundSet,
}

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, crisp_data::PIG_VARIANTS))
}

pub fn sound_variant(id: i32) -> &'static PigSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<PigSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = crisp_data::PIG_SOUND_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let set = |v: &Value| PigSoundSet {
            ambient_sound: v["ambient_sound"].as_str().unwrap_or_default().to_string(),
            hurt_sound: v["hurt_sound"].as_str().unwrap_or_default().to_string(),
            death_sound: v["death_sound"].as_str().unwrap_or_default().to_string(),
            step_sound: v["step_sound"].as_str().unwrap_or_default().to_string(),
            eat_sound: v["eat_sound"].as_str().unwrap_or_default().to_string(),
        };
        crisp_data::SyncedRegistry::get(SOUND_VARIANT_REGISTRY)
            .entries
            .iter()
            .map(|name| PigSoundVariant { adult_sounds: set(&json[name]["adult_sounds"]), baby_sounds: set(&json[name]["baby_sounds"]) })
            .collect()
    });
    &all[id as usize]
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::PIG_FOOD)
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::PigVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn get_sound_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_SOUND_VARIANT_ID) {
        DataValue::PigSoundVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::PigVariant(variant));
}

pub fn set_sound_variant(entity: &mut Entity, sound_variant: i32) {
    entity.data.set(DATA_SOUND_VARIANT_ID, DataValue::PigSoundVariant(sound_variant));
}

impl Pig {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        let temperate = variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:temperate").unwrap_or(0);
        let classic = variant_utils::read_variant(SOUND_VARIANT_REGISTRY, "minecraft:classic").unwrap_or(0);
        data.extend([DataValue::Int(0), DataValue::PigVariant(temperate), DataValue::PigSoundVariant(classic)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:pig"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&PIG_ATTRIBUTES), MobKind::Pig(Pig::default()));
        Animal::init(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(1.25)));
        goal_selector.add_goal(3, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(4, Goal::Tempt(TemptGoal::new(1.2, |i| i.is(items::CARROT_ON_A_STICK), false)));
        goal_selector.add_goal(4, Goal::Tempt(TemptGoal::new(1.2, |i| i.get_item().is(&item_tags::PIG_FOOD), false)));
        goal_selector.add_goal(5, Goal::FollowParent(FollowParentGoal::new(1.1)));
        goal_selector.add_goal(6, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
    }

    pub fn get_sound_set(mob: &Mob) -> &'static PigSoundSet {
        let sound_variant = sound_variant(get_sound_variant(&mob.entity));
        if mob.is_baby() { &sound_variant.baby_sounds } else { &sound_variant.adult_sounds }
    }
}

impl ServerLevel {
    pub fn pig_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let has_food = is_food(&self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty));
        if !has_food && self.is_saddled(id) && !self.is_vehicle(id) && !self.player(player).is_secondary_use_active() {
            return InteractionResult::SUCCESS;
        }
        let interaction_result = self.animal_mob_interact(id, player, hand);
        if !interaction_result.consumes_action() {
            let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if !self.is_equippable_in_slot(id, &item_stack, EquipmentSlot::Saddle) {
                return InteractionResult::Pass;
            }
            let mut item_stack = item_stack;
            let result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            return result;
        }
        interaction_result
    }

    pub fn pig_can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        if slot != EquipmentSlot::Saddle { true } else { self.is_alive(id) && !self.is_baby(id) }
    }

    pub fn pig_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        if self.difficulty != Difficulty::Peaceful {
            let params = ConversionParams::single(self, id, false, true);
            let zombified_piglin = self.convert_to(id, "minecraft:zombified_piglin", params, |level, zombified_piglin| {
                level.mob_mut(zombified_piglin).expect("a zombified piglin").persistence_required = true;
            });
            if zombified_piglin.is_none() {
                self.entity_thunder_hit(id, lightning_bolt);
            }
        } else {
            self.entity_thunder_hit(id, lightning_bolt);
        }
    }

    pub fn pig_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Pig::create());
        if self.mob(partner).is_some_and(|mob| matches!(mob.kind, MobKind::Pig(_))) {
            let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
            set_variant(&mut baby.entity, variant);
        }
        Some(baby)
    }

    pub fn pig_finalize_spawn(&mut self, mob: &mut Mob, pos: BlockPos) {
        if let Some(variant) = self.select_variant_to_spawn(pos, spawn_selectors()) {
            set_variant(&mut mob.entity, variant);
        }
        let sound_variant = self.pick_random_sound_variant(SOUND_VARIANT_REGISTRY);
        set_sound_variant(&mut mob.entity, sound_variant);
    }
}
