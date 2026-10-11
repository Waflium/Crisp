use super::goal::Goal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct WrappedGoal {
    goal: Goal,
    priority: i32,
    is_running: bool,
}

impl WrappedGoal {
    pub fn new(priority: i32, goal: Goal) -> WrappedGoal {
        WrappedGoal { goal, priority, is_running: false }
    }

    pub fn can_be_replaced_by(&self, goal: &WrappedGoal) -> bool {
        self.is_interruptable() && goal.get_priority() < self.get_priority()
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.goal.can_continue_to_use(level, mob)
    }

    pub fn is_interruptable(&self) -> bool {
        self.goal.is_interruptable()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if !self.is_running {
            self.is_running = true;
            self.goal.start(level, mob);
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.is_running {
            self.is_running = false;
            self.goal.stop(level, mob);
        }
    }

    pub fn requires_update_every_tick(&self) -> bool {
        self.goal.requires_update_every_tick()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.goal.tick(level, mob);
    }

    pub fn get_flags(&self) -> u8 {
        self.goal.get_flags()
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn get_priority(&self) -> i32 {
        self.priority
    }

    pub fn get_goal(&self) -> &Goal {
        &self.goal
    }
}
