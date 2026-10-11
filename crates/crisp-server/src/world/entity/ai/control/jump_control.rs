use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct JumpControl {
    pub jump: bool,
}

impl JumpControl {
    pub fn jump(&mut self) {
        self.jump = true;
    }
}

impl ServerLevel {
    pub fn jump_control_tick(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a mob");
        mob.living.jumping = mob.ai.jump_control.jump;
        mob.ai.jump_control.jump = false;
    }
}
