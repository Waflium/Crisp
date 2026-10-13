use crisp_core::Vec3;
use crisp_core::random::RandomSource;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::default_random_pos;

pub const DEFAULT_INTERVAL: i32 = 120;

pub struct RandomStrollGoal {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
    interval: i32,
    force_trigger: bool,
    check_no_action_time: bool,
}

impl RandomStrollGoal {
    pub fn new(speed_modifier: f64) -> RandomStrollGoal {
        RandomStrollGoal { wanted_x: 0.0, wanted_y: 0.0, wanted_z: 0.0, speed_modifier, interval: DEFAULT_INTERVAL, force_trigger: false, check_no_action_time: true }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId, get_position: impl FnOnce(&mut ServerLevel, EntityId) -> Option<Vec3>) -> bool {
        if level.has_controlling_passenger(mob) {
            return false;
        }
        if !self.force_trigger {
            if self.check_no_action_time && level.living(mob).no_action_time >= 100 {
                return false;
            }
            if level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(self.interval)) != 0 {
                return false;
            }
        }
        let Some(pos) = get_position(level, mob) else { return false };
        self.wanted_x = pos.x;
        self.wanted_y = pos.y;
        self.wanted_z = pos.z;
        self.force_trigger = false;
        true
    }

    pub fn get_position(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
        default_random_pos::get_pos(level, mob, 10, 7)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a mob").ai.navigation.is_done() && !level.has_controlling_passenger(mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.wanted_x, self.wanted_y, self.wanted_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").ai.navigation.stop();
    }

    pub fn trigger(&mut self) {
        self.force_trigger = true;
    }

    pub fn set_interval(&mut self, interval: i32) {
        self.interval = interval;
    }
}
