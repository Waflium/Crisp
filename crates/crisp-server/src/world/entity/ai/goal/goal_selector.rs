use super::goal::{Flag, Goal};
use super::wrapped_goal::WrappedGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct GoalSelector {
    locked_flags: [Option<usize>; 4],
    available_goals: Vec<WrappedGoal>,
    disabled_flags: u8,
}

impl GoalSelector {
    pub fn add_goal(&mut self, prio: i32, goal: Goal) {
        self.available_goals.push(WrappedGoal::new(prio, goal));
    }

    fn goal_contains_any_flags(goal: &WrappedGoal, disabled_flags: u8) -> bool {
        goal.get_flags() & disabled_flags != 0
    }

    fn goal_can_be_replaced_for_all_flags(&self, goal: usize) -> bool {
        let wrapped = &self.available_goals[goal];
        Flag::VALUES.into_iter().filter(|flag| wrapped.get_flags() & flag.bit() != 0).all(|flag| match self.locked_flags[flag as usize] {
            Some(locked) => self.available_goals[locked].can_be_replaced_by(wrapped),
            None => true,
        })
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        for goal in &mut self.available_goals {
            if goal.is_running() && (GoalSelector::goal_contains_any_flags(goal, self.disabled_flags) || !goal.can_continue_to_use(level, mob)) {
                goal.stop(level, mob);
            }
        }
        for locked in &mut self.locked_flags {
            if locked.is_some_and(|goal| !self.available_goals[goal].is_running()) {
                *locked = None;
            }
        }
        for i in 0..self.available_goals.len() {
            let goal = &self.available_goals[i];
            if !goal.is_running()
                && !GoalSelector::goal_contains_any_flags(goal, self.disabled_flags)
                && self.goal_can_be_replaced_for_all_flags(i)
                && self.available_goals[i].can_use(level, mob)
            {
                let flags = self.available_goals[i].get_flags();
                for flag in Flag::VALUES.into_iter().filter(|flag| flags & flag.bit() != 0) {
                    if let Some(current_goal) = self.locked_flags[flag as usize] {
                        self.available_goals[current_goal].stop(level, mob);
                    }
                    self.locked_flags[flag as usize] = Some(i);
                }
                self.available_goals[i].start(level, mob);
            }
        }
        self.tick_running_goals(level, mob, true);
    }

    pub fn tick_running_goals(&mut self, level: &mut ServerLevel, mob: EntityId, force_tick_all_running_goals: bool) {
        for goal in &mut self.available_goals {
            if goal.is_running() && (force_tick_all_running_goals || goal.requires_update_every_tick()) {
                goal.tick(level, mob);
            }
        }
    }

    pub fn get_available_goals(&self) -> &[WrappedGoal] {
        &self.available_goals
    }

    pub fn disable_control_flag(&mut self, flag: Flag) {
        self.disabled_flags |= flag.bit();
    }

    pub fn enable_control_flag(&mut self, flag: Flag) {
        self.disabled_flags &= !flag.bit();
    }

    pub fn set_control_flag(&mut self, flag: Flag, enabled: bool) {
        if enabled {
            self.enable_control_flag(flag);
        } else {
            self.disable_control_flag(flag);
        }
    }
}
