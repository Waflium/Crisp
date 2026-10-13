use crisp_core::{AABB, Vec3};

use super::entity_attachments::EntityAttachments;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EntityDimensions {
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
    pub attachments: EntityAttachments,
}

impl EntityDimensions {
    pub const fn scalable(width: f32, height: f32) -> EntityDimensions {
        EntityDimensions { width, height, eye_height: height * 0.85, attachments: EntityAttachments::create_default(height) }
    }

    pub const fn with_eye_height(self, eye_height: f32) -> EntityDimensions {
        EntityDimensions { eye_height, ..self }
    }

    pub const fn with_passenger_attachments(self, points: &'static [Vec3]) -> EntityDimensions {
        EntityDimensions { attachments: self.attachments.with_passenger(points), ..self }
    }

    pub const fn with_vehicle_attachment(self, points: &'static [Vec3]) -> EntityDimensions {
        EntityDimensions { attachments: self.attachments.with_vehicle(points), ..self }
    }

    pub fn scale(self, scale_factor: f32) -> EntityDimensions {
        if scale_factor == 1.0 {
            return self;
        }
        EntityDimensions {
            width: self.width * scale_factor,
            height: self.height * scale_factor,
            eye_height: self.eye_height * scale_factor,
            attachments: self.attachments.scale(scale_factor, scale_factor, scale_factor),
        }
    }

    pub fn make_bounding_box(self, pos: Vec3) -> AABB {
        let w = (self.width / 2.0) as f64;
        let h = self.height as f64;
        AABB::new(pos.x - w, pos.y, pos.z - w, pos.x + w, pos.y + h, pos.z + w)
    }
}
