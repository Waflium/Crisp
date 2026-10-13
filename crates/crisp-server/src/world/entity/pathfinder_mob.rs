use crisp_core::BlockPos;

use super::EntityId;
use super::ai::goal::goal::Goal;
use super::mob::MobKind;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn get_walk_target_value(&self, id: EntityId, pos: BlockPos) -> f32 {
        match self.mob(id).expect("a pathfinder mob").kind {
            MobKind::MushroomCow(_) => self.mushroom_cow_get_walk_target_value(pos),
            _ => self.animal_get_walk_target_value(pos),
        }
    }

    pub fn is_path_finding(&self, id: EntityId) -> bool {
        !self.mob(id).expect("a pathfinder mob").ai.navigation.is_done()
    }

    pub fn is_panicking(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a pathfinder mob");
        mob.ai.goal_selector.get_available_goals().iter().any(|wrapped_goal| wrapped_goal.is_running() && matches!(wrapped_goal.get_goal(), Goal::Panic(_)))
    }
}
