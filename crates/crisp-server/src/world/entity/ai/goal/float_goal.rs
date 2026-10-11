use crisp_core::random::RandomSource;
use crisp_data::{Tag, fluid_tags};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::navigation::path_navigation::PathNavigation;

pub struct FloatGoal {
    fluid: &'static Tag,
}

impl FloatGoal {
    pub fn new(navigation: &mut PathNavigation) -> FloatGoal {
        navigation.set_can_float(true);
        FloatGoal { fluid: &fluid_tags::ENTITY_FLOATABLE }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        e.fluid_height(self.fluid) > e.fluid_jump_threshold() || e.is_in_lava()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob_mut(mob).expect("a mob");
        if m.entity.random.next_float() < 0.8 {
            m.ai.jump_control.jump();
        }
    }
}
