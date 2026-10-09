use crisp_core::{Vec3, mth};
use crisp_data::packets::play::clientbound;

use crate::commands::source::{Anchor, CommandSourceStack};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, Debug)]
pub enum LookAt {
    Entity { entity: EntityId, anchor: Anchor },
    Position(Vec3),
}

pub fn anchor_apply(level: &ServerLevel, anchor: Anchor, entity: EntityId) -> Vec3 {
    let e = level.entity(entity);
    match anchor {
        Anchor::Feet => e.position(),
        Anchor::Eyes => Vec3::new(e.x(), e.y() + e.dimensions.eye_height as f64, e.z()),
    }
}

fn anchor_id(anchor: Anchor) -> i32 {
    match anchor {
        Anchor::Feet => 0,
        Anchor::Eyes => 1,
    }
}

impl LookAt {
    pub fn perform(self, level: &mut ServerLevel, source: &CommandSourceStack, target: EntityId) {
        let from = source.anchor;
        let (pos, to) = match self {
            LookAt::Entity { entity, anchor } => (anchor_apply(level, anchor, entity), Some((entity, anchor))),
            LookAt::Position(pos) => (pos, None),
        };
        look_at(level, target, from, pos);
        if let Some(player) = level.players.get(&target) {
            player.connection.send(clientbound::PLAYER_LOOK_AT, |w| {
                w.varint(anchor_id(from));
                w.f64(pos.x);
                w.f64(pos.y);
                w.f64(pos.z);
                w.bool(to.is_some());
                if let Some((entity, anchor)) = to {
                    w.varint(entity);
                    w.varint(anchor_id(anchor));
                }
            });
        }
    }
}

fn look_at(level: &mut ServerLevel, id: EntityId, anchor: Anchor, pos: Vec3) {
    let from = anchor_apply(level, anchor, id);
    let (xd, yd, zd) = (pos.x - from.x, pos.y - from.y, pos.z - from.z);
    let sd = (xd * xd + zd * zd).sqrt();
    let pi = std::f32::consts::PI as f64;
    let living = level.is_player(id);
    let e = level.entity_mut(id);
    e.set_x_rot(mth::wrap_degrees((-(mth::atan2(yd, sd) * 180.0 / pi)) as f32));
    e.set_y_rot(mth::wrap_degrees((mth::atan2(zd, xd) * 180.0 / pi) as f32 - 90.0));
    if living {
        e.y_head_rot = e.y_rot;
    }
}
