use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{blocks, game_rule};

use super::ageable_mob::{AgeableMob, get_speed_up_seconds_when_feeding};
use super::mob::{Mob, MobKind};
use crate::world::level::pathfinder::path_type::PathType;
use super::{EntityId, LevelEntity, entity_event};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::item::ItemStack;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::{InteractionHand, InteractionResult};

pub mod chicken;

#[derive(Default)]
pub struct Animal {
    pub ageable: AgeableMob,
    pub in_love: i32,
    pub love_cause: Option<u128>,
}

impl Animal {
    pub fn init(mob: &mut Mob) {
        mob.set_pathfinding_malus(PathType::FireInNeighbor, 16.0);
        mob.set_pathfinding_malus(PathType::Fire, -1.0);
    }
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

    pub fn animal_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        if PathNavigationRegion::new(self).get_block_state(pos.below()).is_block(blocks::GRASS_BLOCK) { 10.0 } else { self.get_pathfinding_cost_from_light_levels(pos) }
    }

    pub fn is_in_love(&self, id: EntityId) -> bool {
        self.mob(id).and_then(|mob| mob.animal()).is_some_and(|animal| animal.in_love > 0)
    }

    pub fn reset_love(&mut self, id: EntityId) {
        self.mob_mut(id).expect("an animal").animal_mut().expect("an animal").in_love = 0;
    }

    pub fn can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        if partner == id {
            return false;
        }
        let same_class = matches!((&self.mob(id).expect("an animal").kind, self.mob(partner).map(|mob| &mob.kind)), (MobKind::Chicken(_), Some(MobKind::Chicken(_))));
        same_class && self.is_in_love(id) && self.is_in_love(partner)
    }

    pub fn spawn_child_from_breeding(&mut self, id: EntityId, partner: EntityId) {
        let Some(mut offspring) = self.get_breed_offspring(id, partner) else { return };
        offspring.set_baby(true);
        let e = self.entity(id);
        offspring.entity.snap_to(e.x(), e.y(), e.z(), 0.0, 0.0);
        let offspring_id = offspring.entity.id;
        self.entities.insert(offspring_id, LevelEntity::Mob(offspring));
        self.finalize_spawn_child_from_breeding(id, partner, Some(offspring_id));
        let offspring = self.entities.shift_remove(&offspring_id).expect("offspring");
        self.add_fresh_entity(offspring);
    }

    fn get_love_cause(&self, id: EntityId) -> Option<EntityId> {
        let love_cause = self.mob(id)?.animal()?.love_cause?;
        self.players.values().find(|p| p.profile.uuid == love_cause).map(|p| p.entity.id)
    }

    pub fn finalize_spawn_child_from_breeding(&mut self, id: EntityId, partner: EntityId, offspring: Option<EntityId>) {
        if let Some(cause) = self.get_love_cause(id).or_else(|| self.get_love_cause(partner)) {
            self.award_stat(cause, crate::stats::stats::ANIMALS_BRED, 1);
            crate::advancements::triggers::criteria_triggers::BRED_ANIMALS.trigger(self, cause, id, partner, offspring);
        }
        self.set_age(id, 6000);
        self.set_age(partner, 6000);
        self.reset_love(id);
        self.reset_love(partner);
        self.broadcast_entity_event(id, entity_event::IN_LOVE_HEARTS);
        if self.game_rules.get_bool(game_rule::MOB_DROPS) {
            let e = self.entity_mut(id);
            let value = e.random.next_int_bounded(7) + 1;
            let pos = e.position();
            let orb = self.new_experience_orb(pos, Vec3::ZERO, value);
            self.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
        }
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
    fn chickens_follow_players_holding_seeds() {
        let (mut level, player, chicken) = setup(0, ItemStack::new(items::WHEAT_SEEDS, 1));
        level.entity_mut(chicken).set_pos(Vec3::new(7.5, -60.0, 0.5));
        for _ in 0..100 {
            level.tick();
        }
        let distance = level.entity(chicken).distance_to_sqr(level.entity(player));
        assert!(distance < 2.5 * 2.5 + 1.0, "chicken stayed {distance} away");
    }

    #[test]
    fn tempted_chickens_walk_vanillas_path() {
        use crate::world::level::update_flags::UPDATE_CLIENTS;
        let expected: Vec<Vec3> = include_str!("tempt_oracle.txt")
            .lines()
            .map(|line| {
                let v: Vec<f64> = line.split(' ').map(|n| n.parse().unwrap()).collect();
                Vec3::new(v[0], v[1], v[2])
            })
            .collect();
        let mut level = crate::server::level::testing::level();
        let mut fill = |level: &mut ServerLevel, from: (i32, i32, i32), to: (i32, i32, i32), block: crisp_data::Block| {
            for x in from.0..=to.0 {
                for y in from.1..=to.1 {
                    for z in from.2..=to.2 {
                        level.set_block(crisp_core::BlockPos::new(x, y, z), block.default_state(), UPDATE_CLIENTS);
                    }
                }
            }
        };
        fill(&mut level, (-2, 149, -6), (16, 149, 6), crisp_data::blocks::STONE);
        fill(&mut level, (7, 150, -2), (7, 151, 2), crisp_data::blocks::STONE);
        fill(&mut level, (9, 150, -6), (10, 150, 6), crisp_data::blocks::STONE);
        fill(&mut level, (5, 150, -4), (5, 150, -1), crisp_data::blocks::OAK_FENCE);
        let player = testing::player(&mut level, Vec3::new(12.5, 150.0, 0.5), 0.0, 0.0);
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::WHEAT_SEEDS, 1)));
        let mut mob = Chicken::create();
        mob.entity.snap_to(3.5, 150.0, 0.5, 0.0, 0.0);
        mob.entity.y_head_rot = 0.0;
        mob.living.y_body_rot = 0.0;
        let chicken = mob.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
        let mut actual = vec![level.entity(chicken).position()];
        for _ in 0..expected.len() {
            level.tick();
            actual.push(level.entity(chicken).position());
        }
        let n = expected.len() - 4;
        let aligned = (0..4).any(|skip| actual[skip..skip + n] == expected[..n] || actual[..n] == expected[skip..skip + n]);
        assert!(aligned, "the chicken left vanilla's path: {:?}", &actual[..12]);
    }

    #[test]
    fn chickens_ignore_players_without_seeds() {
        let (mut level, _player, chicken) = setup(0, ItemStack::new(items::STICK, 1));
        level.entity_mut(chicken).set_pos(Vec3::new(7.5, -60.0, 0.5));
        level.tick();
        assert!(!level.mob(chicken).unwrap().ai.goal_selector.get_available_goals().iter().any(|g| g.is_running() && matches!(g.get_goal(), crate::world::entity::ai::goal::goal::Goal::Tempt(_))));
    }

    #[test]
    fn chickens_in_love_breed() {
        let (mut level, _player, chicken) = setup(0, ItemStack::new(items::STICK, 1));
        let mut other = Chicken::create();
        other.entity.set_pos(Vec3::new(2.5, -60.0, 2.0));
        let partner = other.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(Box::new(other)));
        level.set_in_love(chicken, None);
        level.set_in_love(partner, None);
        for _ in 0..200 {
            level.tick();
        }
        let babies = level.entities.values().filter(|e| matches!(e, LevelEntity::Mob(mob) if mob.is_baby())).count();
        assert_eq!(babies, 1);
        for parent in [chicken, partner] {
            let animal = level.mob(parent).unwrap().animal().unwrap();
            assert!(animal.ageable.age > 5800 && animal.ageable.age < 6000);
            assert_eq!(animal.in_love, 0);
        }
    }

    #[test]
    fn spawn_eggs_spawn_chickens_on_the_clicked_face() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let mut egg = ItemStack::new(items::CHICKEN_SPAWN_EGG, 2);
        egg.set(dc::CUSTOM_NAME, Some(Nbt::String("Eggbert".into())));
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(egg));
        let block_pos = crisp_core::BlockPos::new(3, -61, 3);
        let hit = crate::world::level::block::BlockHitResult { location: Vec3::new(3.5, -60.0, 3.5), direction: crisp_core::Direction::Up, block_pos, inside: false, world_border_hit: false };
        assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit), InteractionResult::SUCCESS);
        let chicken = level.entities.keys().copied().find(|&id| level.is_mob(id)).unwrap();
        assert_eq!(level.entity(chicken).position(), Vec3::new(3.5, -60.0, 3.5));
        assert!(matches!(level.entity(chicken).data.get(crate::world::entity::data::ids::CUSTOM_NAME), DataValue::OptionalComponent(Some(Nbt::String(name))) if name == "Eggbert"));
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
