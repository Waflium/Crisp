use super::breed_goal::BreedGoal;
use super::float_goal::FloatGoal;
use super::follow_parent_goal::FollowParentGoal;
use super::look_at_player_goal::LookAtPlayerGoal;
use super::panic_goal::PanicGoal;
use super::random_look_around_goal::RandomLookAroundGoal;
use super::tempt_goal::TemptGoal;
use super::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flag {
    Move,
    Look,
    Jump,
    Target,
}

impl Flag {
    pub const VALUES: [Flag; 4] = [Flag::Move, Flag::Look, Flag::Jump, Flag::Target];

    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
}

pub enum Goal {
    Float(FloatGoal),
    Panic(PanicGoal),
    Breed(BreedGoal),
    Tempt(TemptGoal),
    FollowParent(FollowParentGoal),
    WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal),
    LookAtPlayer(LookAtPlayerGoal),
    RandomLookAround(RandomLookAroundGoal),
}

impl Goal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match self {
            Goal::Float(goal) => goal.can_use(level, mob),
            Goal::Panic(goal) => goal.can_use(level, mob),
            Goal::Breed(goal) => goal.can_use(level, mob),
            Goal::Tempt(goal) => goal.can_use(level, mob),
            Goal::FollowParent(goal) => goal.can_use(level, mob),
            Goal::WaterAvoidingRandomStroll(goal) => goal.can_use(level, mob),
            Goal::LookAtPlayer(goal) => goal.can_use(level, mob),
            Goal::RandomLookAround(goal) => goal.can_use(level, mob),
        }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match self {
            Goal::Float(goal) => goal.can_use(level, mob),
            Goal::Panic(goal) => goal.can_continue_to_use(level, mob),
            Goal::Breed(goal) => goal.can_continue_to_use(level, mob),
            Goal::Tempt(goal) => goal.can_continue_to_use(level, mob),
            Goal::FollowParent(goal) => goal.can_continue_to_use(level, mob),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.can_continue_to_use(level, mob),
            Goal::LookAtPlayer(goal) => goal.can_continue_to_use(level, mob),
            Goal::RandomLookAround(goal) => goal.can_continue_to_use(),
        }
    }

    pub fn is_interruptable(&self) -> bool {
        true
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(_) | Goal::Breed(_) => {}
            Goal::Panic(goal) => goal.start(level, mob),
            Goal::Tempt(goal) => goal.start(level),
            Goal::FollowParent(goal) => goal.start(),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.start(level, mob),
            Goal::LookAtPlayer(goal) => goal.start(level, mob),
            Goal::RandomLookAround(goal) => goal.start(level, mob),
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(_) | Goal::RandomLookAround(_) => {}
            Goal::Panic(goal) => goal.stop(),
            Goal::Breed(goal) => goal.stop(),
            Goal::Tempt(goal) => goal.stop(level, mob),
            Goal::FollowParent(goal) => goal.stop(),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.stop(level, mob),
            Goal::LookAtPlayer(goal) => goal.stop(),
        }
    }

    pub fn requires_update_every_tick(&self) -> bool {
        matches!(self, Goal::Float(_) | Goal::RandomLookAround(_))
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(goal) => goal.tick(level, mob),
            Goal::Panic(_) | Goal::WaterAvoidingRandomStroll(_) => {}
            Goal::Breed(goal) => goal.tick(level, mob),
            Goal::Tempt(goal) => goal.tick(level, mob),
            Goal::FollowParent(goal) => goal.tick(level, mob),
            Goal::LookAtPlayer(goal) => goal.tick(level, mob),
            Goal::RandomLookAround(goal) => goal.tick(level, mob),
        }
    }

    pub fn get_flags(&self) -> u8 {
        match self {
            Goal::Float(_) => Flag::Jump.bit(),
            Goal::Panic(_) | Goal::WaterAvoidingRandomStroll(_) => Flag::Move.bit(),
            Goal::Breed(_) | Goal::Tempt(_) | Goal::RandomLookAround(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::FollowParent(_) => 0,
            Goal::LookAtPlayer(_) => Flag::Look.bit(),
        }
    }

    pub fn adjusted_tick_delay(&self, ticks: i32) -> i32 {
        if self.requires_update_every_tick() { ticks } else { reduced_tick_delay(ticks) }
    }
}

pub fn reduced_tick_delay(ticks: i32) -> i32 {
    (ticks + 1) / 2
}
