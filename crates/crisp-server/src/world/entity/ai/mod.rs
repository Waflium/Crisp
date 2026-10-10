pub mod control;

use control::{BodyRotationControl, JumpControl, LookControl, MoveControl};

#[derive(Default)]
pub struct MobAi {
    pub look_control: LookControl,
    pub move_control: MoveControl,
    pub jump_control: JumpControl,
    pub body_rotation_control: BodyRotationControl,
}
