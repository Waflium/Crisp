use crisp_core::random::RandomSource;

use super::ageable_mob::{AgeableMob, get_speed_up_seconds_when_feeding};
use super::mob::MobKind;
use super::{EntityId, entity_event};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

pub mod chicken;

#[derive(Default)]
pub struct Animal {
    pub ageable: AgeableMob,
    pub in_love: i32,
    pub love_cause: Option<u128>,
}

impl ServerLevel {
    pub fn animal_custom_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("an animal");
        let age = mob.ageable().expect("an animal").age;
        if age != 0 {
            mob.animal_mut().expect("an animal").in_love = 0;
        }
    }

    pub fn animal_actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        self.mob_mut(id).expect("an animal").animal_mut().expect("an animal").in_love = 0;
        self.living_entity_actually_hurt(id, source, damage);
    }

    pub fn is_food(&self, id: EntityId, item_stack: &ItemStack) -> bool {
        match self.mob(id).expect("an animal").kind {
            MobKind::Chicken(_) => chicken::is_food(item_stack),
        }
    }

    pub fn animal_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.is_food(id, &item_stack) {
            let age = self.mob(id).expect("an animal").ageable().expect("an animal").age;
            if age == 0 && self.can_fall_in_love(id) {
                self.use_player_item(player, hand, &mut item_stack);
                self.set_in_love(id, Some(player));
                return InteractionResult::SUCCESS_SERVER;
            }
            if self.can_age_up(id) {
                self.use_player_item(player, hand, &mut item_stack);
                self.age_up(id, get_speed_up_seconds_when_feeding(-age), true);
                return InteractionResult::SUCCESS;
            }
        }
        self.ageable_mob_mob_interact(id, player, hand)
    }

    pub fn can_fall_in_love(&self, id: EntityId) -> bool {
        self.mob(id).expect("an animal").animal().expect("an animal").in_love <= 0
    }

    pub fn set_in_love(&mut self, id: EntityId, player: Option<EntityId>) {
        let love_cause = player.filter(|&player| self.is_player(player)).map(|player| self.entity(player).uuid);
        let animal = self.mob_mut(id).expect("an animal").animal_mut().expect("an animal");
        animal.in_love = 600;
        if love_cause.is_some() {
            animal.love_cause = love_cause;
        }
        self.broadcast_entity_event(id, entity_event::IN_LOVE_HEARTS);
    }

    pub fn animal_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        1 + self.entity_mut(id).random.next_int_bounded(3)
    }

    pub fn animal_ai_step(&mut self, id: EntityId) {
        self.ageable_mob_ai_step(id);
        let mob = self.mob_mut(id).expect("an animal");
        let age = mob.ageable().expect("an animal").age;
        let animal = mob.animal_mut().expect("an animal");
        if age != 0 {
            animal.in_love = 0;
        }
        if animal.in_love > 0 {
            animal.in_love -= 1;
            if animal.in_love % 10 == 0 {
                let e = &mut mob.entity;
                e.random.next_gaussian();
                e.random.next_gaussian();
                e.random.next_gaussian();
                e.random_x(1.0);
                e.random_y();
                e.random_z(1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::Vec3;
    use crisp_data::items;
    use crisp_protocol::nbt::Nbt;

    use super::chicken::{self, Chicken};
    use crate::server::level::{ServerLevel, testing};
    use crate::world::entity::data::DataValue;
    use crate::world::entity::{EntityId, LevelEntity};
    use crate::world::item::ItemStack;
    use crate::world::item::component as dc;
    use crate::world::{InteractionHand, InteractionResult};

    fn setup(age: i32, held: ItemStack) -> (ServerLevel, EntityId, EntityId) {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(held));
        let mut mob = Chicken::create();
        mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
        mob.set_age(age);
        mob.refresh_dimensions();
        let chicken = mob.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
        (level, player, chicken)
    }

    fn interact(level: &mut ServerLevel, player: EntityId, chicken: EntityId) -> InteractionResult {
        let location = level.entity(chicken).position();
        level.interact_on(player, chicken, InteractionHand::MainHand, location)
    }

    fn held_count(level: &ServerLevel, player: EntityId) -> i32 {
        level.player(player).item_in_hand(InteractionHand::MainHand).map_or(0, |s| s.count)
    }

    #[test]
    fn feeding_an_adult_puts_it_in_love() {
        let (mut level, player, chicken) = setup(0, ItemStack::new(items::WHEAT_SEEDS, 4));
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS_SERVER);
        let animal = level.mob(chicken).unwrap().animal().unwrap();
        assert_eq!(animal.in_love, 600);
        assert_eq!(animal.love_cause, Some(level.entity(player).uuid));
        assert_eq!(held_count(&level, player), 3);
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::Pass);
        assert_eq!(held_count(&level, player), 3);
    }

    #[test]
    fn feeding_a_baby_speeds_up_growth() {
        let (mut level, player, chicken) = setup(-24000, ItemStack::new(items::WHEAT_SEEDS, 1));
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS);
        let ageable = level.mob(chicken).unwrap().ageable().unwrap();
        assert_eq!(ageable.age, -21600);
        assert_eq!(ageable.forced_age, 2400);
        assert_eq!(ageable.forced_age_timer, 40);
        assert_eq!(held_count(&level, player), 0);
    }

    #[test]
    fn creative_players_keep_their_food() {
        let (mut level, player, chicken) = setup(0, ItemStack::new(items::WHEAT_SEEDS, 4));
        level.player_mut(player).abilities.instabuild = true;
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS_SERVER);
        assert_eq!(held_count(&level, player), 4);
    }

    #[test]
    fn other_items_do_nothing() {
        let (mut level, player, chicken) = setup(0, ItemStack::new(items::STICK, 1));
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::Pass);
        assert_eq!(level.mob(chicken).unwrap().animal().unwrap().in_love, 0);
    }

    #[test]
    fn name_tags_name_the_chicken() {
        let mut tag = ItemStack::new(items::NAME_TAG, 1);
        tag.set(dc::CUSTOM_NAME, Some(Nbt::String("Clucky".into())));
        let (mut level, player, chicken) = setup(0, tag);
        level.player_mut(player).abilities.instabuild = true;
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS);
        assert!(matches!(level.entity(chicken).data.get(crate::world::entity::data::ids::CUSTOM_NAME), DataValue::OptionalComponent(Some(Nbt::String(name))) if name == "Clucky"));
        assert!(level.mob(chicken).unwrap().persistence_required);
        assert_eq!(held_count(&level, player), 1);
        level.player_mut(player).abilities.instabuild = false;
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS);
        assert_eq!(held_count(&level, player), 0);
    }

    #[test]
    fn spawn_eggs_spawn_a_baby_of_the_same_variant() {
        let (mut level, player, chicken) = setup(0, ItemStack::new(items::CHICKEN_SPAWN_EGG, 2));
        let cold = chicken::variant_id("minecraft:cold").unwrap();
        level.entity_mut(chicken).data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(cold));
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS_SERVER);
        let baby = level.entities.keys().copied().find(|&id| id != chicken && level.is_mob(id)).unwrap();
        let mob = level.mob(baby).unwrap();
        assert_eq!(mob.ageable().unwrap().age, -24000);
        assert_eq!(mob.entity.position(), level.entity(chicken).position());
        assert!(matches!(mob.entity.data.get(chicken::DATA_VARIANT), DataValue::ChickenVariant(v) if *v == cold));
        assert_eq!(held_count(&level, player), 1);
    }

    #[test]
    fn golden_dandelions_lock_a_babys_age() {
        let (mut level, player, chicken) = setup(-100, ItemStack::new(items::GOLDEN_DANDELION, 1));
        assert_eq!(interact(&mut level, player, chicken), InteractionResult::SUCCESS);
        let mob = level.mob(chicken).unwrap();
        assert!(mob.is_age_locked());
        assert!(mob.persistence_required);
        assert_eq!(mob.ageable().unwrap().age, -24000);
        assert_eq!(mob.ageable().unwrap().age_lock_particle_timer, 40);
        assert_eq!(held_count(&level, player), 0);
    }
}
