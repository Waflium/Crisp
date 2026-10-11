use crisp_core::BlockPos;
use crisp_data::{Tag, damage_type_tags, fluid_tags};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::level::path_navigation_region::PathNavigationRegion;

pub struct PanicGoal {
    speed_modifier: f64,
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
    is_running: bool,
    panic_causing_damage_types: &'static Tag,
}

impl PanicGoal {
    pub fn new(speed_modifier: f64) -> PanicGoal {
        PanicGoal { speed_modifier, pos_x: 0.0, pos_y: 0.0, pos_z: 0.0, is_running: false, panic_causing_damage_types: &damage_type_tags::PANIC_CAUSES }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.should_panic(level, mob) {
            return false;
        }
        if level.is_on_fire(mob)
            && let Some(block_pos) = self.look_for_water(level, mob, 5)
        {
            self.pos_x = block_pos.x as f64;
            self.pos_y = block_pos.y as f64;
            self.pos_z = block_pos.z as f64;
            return true;
        }
        self.find_random_position(level, mob)
    }

    fn should_panic(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_last_damage_source(mob).is_some_and(|source| source.is(self.panic_causing_damage_types))
    }

    fn find_random_position(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(pos) = default_random_pos::get_pos(level, mob, 5, 4) else { return false };
        self.pos_x = pos.x;
        self.pos_y = pos.y;
        self.pos_z = pos.z;
        true
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (x, y, z, speed_modifier) = (self.pos_x, self.pos_y, self.pos_z, self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
        self.is_running = true;
    }

    pub fn stop(&mut self) {
        self.is_running = false;
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a mob").ai.navigation.is_done()
    }

    fn look_for_water(&self, level: &ServerLevel, mob: EntityId, xz_dist: i32) -> Option<BlockPos> {
        let mob_position = level.entity(mob).block_position();
        let region = PathNavigationRegion::new(level);
        if !region.get_block_state(mob_position).collision_voxel_shape().is_empty() {
            return None;
        }
        BlockPos::within_box_by_manhattan_distance(mob_position, xz_dist, 1, xz_dist).find(|&pos| region.get_block_state(pos).fluid_state().is(&fluid_tags::WATER))
    }
}
