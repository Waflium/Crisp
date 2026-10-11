pub mod control;
pub mod goal;
pub mod navigation;
pub mod sensing;
pub mod targeting;
pub mod util;

use control::body_rotation_control::BodyRotationControl;
use control::jump_control::JumpControl;
use control::look_control::LookControl;
use control::move_control::MoveControl;
use goal::goal_selector::GoalSelector;
use navigation::path_navigation::PathNavigation;
use sensing::sensing::Sensing;

#[derive(Default)]
pub struct MobAi {
    pub look_control: LookControl,
    pub move_control: MoveControl,
    pub jump_control: JumpControl,
    pub body_rotation_control: BodyRotationControl,
    pub navigation: PathNavigation,
    pub goal_selector: GoalSelector,
    pub target_selector: GoalSelector,
    pub sensing: Sensing,
}
