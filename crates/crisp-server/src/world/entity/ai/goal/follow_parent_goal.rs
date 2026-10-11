use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct FollowParentGoal {
    parent: Option<EntityId>,
    speed_modifier: f64,
    time_to_recalc_path: i32,
}

impl FollowParentGoal {
    pub fn new(speed_modifier: f64) -> FollowParentGoal {
        FollowParentGoal { parent: None, speed_modifier, time_to_recalc_path: 0 }
    }

    fn age(level: &ServerLevel, animal: EntityId) -> i32 {
        level.mob(animal).and_then(|mob| mob.ageable()).expect("an animal").age
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if FollowParentGoal::age(level, mob) >= 0 {
            return false;
        }
        let e = level.entity(mob);
        let parents = level.get_entities_of_class_same_kind(mob, &e.bounding_box().inflate(8.0, 4.0, 8.0));
        let mut closest = None;
        let mut closest_dist_sqr = f64::MAX;
        for parent in parents {
            if FollowParentGoal::age(level, parent) >= 0 {
                let dist_sqr = level.entity(mob).distance_to_sqr(level.entity(parent));
                if !(dist_sqr > closest_dist_sqr) {
                    closest_dist_sqr = dist_sqr;
                    closest = Some(parent);
                }
            }
        }
        let Some(closest) = closest else { return false };
        if closest_dist_sqr < 9.0 {
            return false;
        }
        self.parent = Some(closest);
        true
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if FollowParentGoal::age(level, mob) >= 0 {
            return false;
        }
        let parent = self.parent.expect("a parent");
        if !level.is_alive(parent) {
            return false;
        }
        let dist_sqr = level.entity(mob).distance_to_sqr(level.entity(parent));
        !(dist_sqr < 9.0) && !(dist_sqr > 256.0)
    }

    pub fn start(&mut self) {
        self.time_to_recalc_path = 0;
    }

    pub fn stop(&mut self) {
        self.parent = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path <= 0 {
            self.time_to_recalc_path = reduced_tick_delay(10);
            let (parent, speed_modifier) = (self.parent.expect("a parent"), self.speed_modifier);
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, parent, speed_modifier));
        }
    }
}
