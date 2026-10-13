use crisp_data::{item_tags, items};

use crate::server::level::ServerLevel;
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
use crate::world::entity::{EntityDimensions, EntityId, sound_event};
use crate::world::item::{ItemStack, item_utils};
use crate::world::{InteractionHand, InteractionResult};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 1.4).with_eye_height(1.3).with_passenger_attachments(&[crisp_core::Vec3::new(0.0, 1.36875, 0.0)]);

#[derive(Default)]
pub struct AbstractCow {
    pub animal: Animal,
}

pub struct CowSoundVariant {
    pub ambient_sound: String,
    pub hurt_sound: String,
    pub death_sound: String,
    pub step_sound: String,
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::COW_FOOD)
}

impl AbstractCow {
    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::Panic(PanicGoal::new(2.0)));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.25, |i| i.get_item().is(&item_tags::COW_FOOD), false)));
        goal_selector.add_goal(4, Goal::FollowParent(FollowParentGoal::new(1.25)));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(7, Goal::RandomLookAround(RandomLookAroundGoal::default()));
    }
}

impl ServerLevel {
    pub fn abstract_cow_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::BUCKET) && !self.is_baby(id) {
            self.entity_play_sound(player, sound_event("minecraft:entity.cow.milk"), 1.0, 1.0);
            let bucket_or_milk_bucket = item_utils::create_filled_result(self, player, hand, item_stack, ItemStack::new(items::MILK_BUCKET, 1), true);
            self.player_mut(player).set_item_in_hand(hand, Some(bucket_or_milk_bucket));
            return InteractionResult::SUCCESS;
        }
        self.animal_mob_interact(id, player, hand)
    }
}
