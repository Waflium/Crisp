use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{item_tags, items, registry, registry_id};

use super::sheep_color_spawn_rules;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::eat_block_goal::EatBlockGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::SHEEP_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::dye_color::DyeColor;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_WOOL_ID: usize = 18;
pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.9, height: 1.3, eye_height: 1.235 };
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.45, height: 0.65, eye_height: 0.65625 };
const SHEAR_SHEEP: &str = "minecraft:shearing/sheep";

#[derive(Default)]
pub struct Sheep {
    pub animal: Animal,
    pub eat_animation_tick: i32,
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::SHEEP_FOOD)
}

fn wool(entity: &Entity) -> i8 {
    match entity.data.get(DATA_WOOL_ID) {
        DataValue::Byte(wool) => *wool,
        _ => 0,
    }
}

pub fn get_color(entity: &Entity) -> DyeColor {
    DyeColor::by_id((wool(entity) & 15) as i32)
}

pub fn set_color(entity: &mut Entity, color: DyeColor) {
    let current = wool(entity);
    entity.data.set(DATA_WOOL_ID, DataValue::Byte((current as u8 & 240 | color.id as u8 & 15) as i8));
}

pub fn is_sheared(entity: &Entity) -> bool {
    wool(entity) & 16 != 0
}

pub fn set_sheared(entity: &mut Entity, value: bool) {
    let current = wool(entity);
    entity.data.set(DATA_WOOL_ID, DataValue::Byte(if value { current | 16 } else { current & -17 }));
}

impl Sheep {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Byte(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:sheep"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&SHEEP_ATTRIBUTES), MobKind::Sheep(Sheep::default()));
        Animal::init(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(1.25)));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.1, |i| i.get_item().is(&item_tags::SHEEP_FOOD), false)));
        goal_selector.add_goal(4, Goal::FollowParent(FollowParentGoal::new(1.1)));
        goal_selector.add_goal(5, Goal::EatBlock(EatBlockGoal::default()));
        goal_selector.add_goal(6, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
    }
}

impl ServerLevel {
    pub fn sheep_custom_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a sheep");
        let eat_animation_tick = mob
            .ai
            .goal_selector
            .get_available_goals()
            .iter()
            .find_map(|wrapped_goal| match wrapped_goal.get_goal() {
                Goal::EatBlock(eat_block_goal) => Some(eat_block_goal.get_eat_animation_tick()),
                _ => None,
            })
            .unwrap_or(0);
        if let MobKind::Sheep(sheep) = &mut mob.kind {
            sheep.eat_animation_tick = eat_animation_tick;
        }
        self.animal_custom_server_ai_step(id);
    }

    pub fn sheep_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::SHEARS) {
            if self.sheep_ready_for_shearing(id) {
                self.sheep_shear(id, SoundSource::Players, &item_stack);
                self.hurt_and_break(player, hand.as_equipment_slot(), 1);
                return InteractionResult::SUCCESS_SERVER;
            }
            return InteractionResult::CONSUME;
        }
        self.animal_mob_interact(id, player, hand)
    }

    pub fn sheep_shear(&mut self, id: EntityId, sound_source: SoundSource, tool: &ItemStack) {
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.sheep.shear"), sound_source, 1.0, 1.0);
        self.drop_from_shearing_loot_table(id, SHEAR_SHEEP, tool, |l, drop| {
            for _ in 0..drop.count {
                let Some(entity) = l.spawn_at_location(id, drop.copy_with_count(1), 1.0) else { continue };
                let random = &mut l.entity_mut(id).random;
                let delta = Vec3::new(
                    ((random.next_float() - random.next_float()) * 0.1) as f64,
                    (random.next_float() * 0.05) as f64,
                    ((random.next_float() - random.next_float()) * 0.1) as f64,
                );
                let e = l.entity_mut(entity);
                let movement = e.delta_movement.add_vec(delta);
                e.set_delta_movement(movement);
            }
        });
        set_sheared(self.entity_mut(id), true);
    }

    pub fn sheep_ready_for_shearing(&self, id: EntityId) -> bool {
        !is_sheared(self.entity(id)) && !self.is_baby(id)
    }

    pub fn sheep_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut sheep = Box::new(Sheep::create());
        let parent1_dye_color = get_color(self.entity(id));
        let parent2_dye_color = get_color(self.entity(partner));
        set_color(&mut sheep.entity, DyeColor::get_mixed_color(self, parent1_dye_color, parent2_dye_color));
        Some(sheep)
    }

    pub fn sheep_ate(&mut self, id: EntityId) {
        set_sheared(self.entity_mut(id), false);
        if self.can_age_up(id) {
            self.age_up(id, 60, false);
        }
    }

    pub fn get_random_sheep_color(&mut self, pos: BlockPos) -> DyeColor {
        let biome = self.get_biome(pos).0 as i32;
        sheep_color_spawn_rules::get_sheep_color(biome, &mut self.random)
    }

    pub fn sheep_finalize_spawn(&mut self, mob: &mut Mob, pos: BlockPos) {
        let color = self.get_random_sheep_color(pos);
        set_color(&mut mob.entity, color);
    }

    pub fn is_sheep(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Mob(mob)) if matches!(mob.kind, MobKind::Sheep(_)))
    }
}
