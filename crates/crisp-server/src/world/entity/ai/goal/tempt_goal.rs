use crisp_data::{EquipmentSlot, Tag};

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::Attribute;

pub const DEFAULT_STOP_DISTANCE: f64 = 2.5;

pub struct TemptGoal {
    speed_modifier: f64,
    px: f64,
    py: f64,
    pz: f64,
    p_rot_x: f64,
    p_rot_y: f64,
    player: Option<EntityId>,
    calm_down: i32,
    is_running: bool,
    items: &'static Tag,
    can_scare: bool,
    stop_distance: f64,
}

impl TemptGoal {
    pub fn new(speed_modifier: f64, items: &'static Tag, can_scare: bool) -> TemptGoal {
        TemptGoal {
            speed_modifier,
            px: 0.0,
            py: 0.0,
            pz: 0.0,
            p_rot_x: 0.0,
            p_rot_y: 0.0,
            player: None,
            calm_down: 0,
            is_running: false,
            items,
            can_scare,
            stop_distance: DEFAULT_STOP_DISTANCE,
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.calm_down > 0 {
            self.calm_down -= 1;
            return false;
        }
        let items = self.items;
        let should_follow = |target: EntityId, level: &ServerLevel| should_follow(level, items, target);
        let targeting_conditions = TargetingConditions::for_non_combat().ignore_line_of_sight().selector(&should_follow).range(level.attribute_value(mob, Attribute::TemptRange));
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.player = level.get_nearest_player(&targeting_conditions, Some(mob), x, y, z);
        self.player.is_some()
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if self.can_scare
            && let Some(player) = level.entity_ref(self.player.expect("a tempting player"))
        {
            if level.entity(mob).distance_to_sqr(player) < 36.0 {
                if player.distance_to_sqr_xyz(self.px, self.py, self.pz) > 0.010000000000000002 {
                    return false;
                }
                if (player.x_rot as f64 - self.p_rot_x).abs() > 5.0 || (player.y_rot as f64 - self.p_rot_y).abs() > 5.0 {
                    return false;
                }
            } else {
                self.px = player.x();
                self.py = player.y();
                self.pz = player.z();
            }
            self.p_rot_x = player.x_rot as f64;
            self.p_rot_y = player.y_rot as f64;
        }
        self.can_use(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel) {
        let player = level.entity(self.player.expect("a tempting player"));
        self.px = player.x();
        self.py = player.y();
        self.pz = player.z();
        self.is_running = true;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.player = None;
        self.stop_navigation(level, mob);
        self.calm_down = reduced_tick_delay(100);
        self.is_running = false;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let player = self.player.expect("a tempting player");
        let m = level.mob(mob).expect("a mob");
        let (y_max_rot_speed, x_max_rot_angle) = ((m.max_head_y_rot() + 20) as f32, m.get_max_head_x_rot() as f32);
        level.set_look_at_entity(mob, player, y_max_rot_speed, x_max_rot_angle);
        if level.entity(mob).distance_to_sqr(level.entity(player)) < self.stop_distance * self.stop_distance {
            self.stop_navigation(level, mob);
        } else {
            self.navigate_towards(level, mob, player);
        }
    }

    fn stop_navigation(&self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").ai.navigation.stop();
    }

    fn navigate_towards(&self, level: &mut ServerLevel, mob: EntityId, player: EntityId) {
        let speed_modifier = self.speed_modifier;
        level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, player, speed_modifier));
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

fn should_follow(level: &ServerLevel, items: &Tag, player: EntityId) -> bool {
    let test = |slot| level.item_by_slot_ref(player, slot).is_some_and(|item| item.get_item().is(items));
    test(EquipmentSlot::Mainhand) || test(EquipmentSlot::Offhand)
}
