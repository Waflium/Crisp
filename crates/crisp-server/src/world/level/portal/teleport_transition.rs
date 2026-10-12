use crisp_core::{BlockPos, Vec3};
use crisp_data::packets::play::clientbound;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::relative::PositionMoveRotation;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PostTeleportTransition {
    play_portal_sound: bool,
    place_portal_ticket: bool,
}

impl PostTeleportTransition {
    pub const DO_NOTHING: PostTeleportTransition = PostTeleportTransition { play_portal_sound: false, place_portal_ticket: false };
    pub const PLAY_PORTAL_SOUND: PostTeleportTransition = PostTeleportTransition { play_portal_sound: true, place_portal_ticket: false };
    pub const PLACE_PORTAL_TICKET: PostTeleportTransition = PostTeleportTransition { play_portal_sound: false, place_portal_ticket: true };

    pub fn then(self, other: PostTeleportTransition) -> PostTeleportTransition {
        PostTeleportTransition {
            play_portal_sound: self.play_portal_sound || other.play_portal_sound,
            place_portal_ticket: self.place_portal_ticket || other.place_portal_ticket,
        }
    }

    pub fn on_transition(self, level: &mut ServerLevel, entity: EntityId) {
        if self.play_portal_sound {
            play_portal_sound(level, entity);
        }
        if self.place_portal_ticket {
            place_portal_ticket(level, entity);
        }
    }
}

fn play_portal_sound(level: &ServerLevel, entity: EntityId) {
    if let Some(player) = level.players.get(&entity) {
        player.connection.send(clientbound::LEVEL_EVENT, |w| {
            w.i32(1032);
            w.position(0, 0, 0);
            w.i32(0);
            w.bool(false);
        });
    }
}

fn place_portal_ticket(level: &mut ServerLevel, entity: EntityId) {
    let position = level.entity(entity).position();
    level.place_portal_ticket(BlockPos::containing(position.x, position.y, position.z));
}

#[derive(Clone, Copy, Debug)]
pub struct TeleportTransition {
    pub new_level: usize,
    pub position: Vec3,
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub missing_respawn_block: bool,
    pub as_passenger: bool,
    pub relatives: u16,
    pub post_teleport_transition: PostTeleportTransition,
}

impl TeleportTransition {
    pub fn new(new_level: usize, position: Vec3, delta_movement: Vec3, y_rot: f32, x_rot: f32, post_teleport_transition: PostTeleportTransition) -> TeleportTransition {
        TeleportTransition::with_relatives(new_level, position, delta_movement, y_rot, x_rot, 0, post_teleport_transition)
    }

    pub fn with_relatives(
        new_level: usize,
        position: Vec3,
        delta_movement: Vec3,
        y_rot: f32,
        x_rot: f32,
        relatives: u16,
        post_teleport_transition: PostTeleportTransition,
    ) -> TeleportTransition {
        TeleportTransition { new_level, position, delta_movement, y_rot, x_rot, missing_respawn_block: false, as_passenger: false, relatives, post_teleport_transition }
    }

    pub fn position_move_rotation(&self) -> PositionMoveRotation {
        PositionMoveRotation { position: self.position, delta_movement: self.delta_movement, y_rot: self.y_rot, x_rot: self.x_rot }
    }

    pub fn with_rotation(self, y_rot: f32, x_rot: f32) -> TeleportTransition {
        TeleportTransition { y_rot, x_rot, ..self }
    }

    pub fn with_position(self, position: Vec3) -> TeleportTransition {
        TeleportTransition { position, ..self }
    }

    pub fn transition_as_passenger(self) -> TeleportTransition {
        TeleportTransition { as_passenger: true, ..self }
    }
}
