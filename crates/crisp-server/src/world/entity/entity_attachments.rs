use crisp_core::{Vec3, mth};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityAttachment {
    Passenger,
    Vehicle,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Points {
    Explicit(&'static [Vec3]),
    AtHeight(f32),
    AtFeet,
}

impl Points {
    fn len(self) -> usize {
        match self {
            Points::Explicit(points) => points.len(),
            Points::AtHeight(_) | Points::AtFeet => 1,
        }
    }

    fn get(self, index: usize) -> Vec3 {
        match self {
            Points::Explicit(points) => points[index],
            Points::AtHeight(height) => Vec3::new(0.0, height as f64, 0.0),
            Points::AtFeet => Vec3::ZERO,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EntityAttachments {
    passenger: Points,
    vehicle: Points,
    scale: Option<[f32; 3]>,
}

impl EntityAttachments {
    pub const fn create_default(height: f32) -> EntityAttachments {
        EntityAttachments { passenger: Points::AtHeight(height), vehicle: Points::AtFeet, scale: None }
    }

    pub const fn with_passenger(self, points: &'static [Vec3]) -> EntityAttachments {
        EntityAttachments { passenger: Points::Explicit(points), ..self }
    }

    pub const fn with_vehicle(self, points: &'static [Vec3]) -> EntityAttachments {
        EntityAttachments { vehicle: Points::Explicit(points), ..self }
    }

    pub fn scale(self, x: f32, y: f32, z: f32) -> EntityAttachments {
        let scale = match self.scale {
            Some([sx, sy, sz]) => [sx * x, sy * y, sz * z],
            None => [x, y, z],
        };
        EntityAttachments { scale: Some(scale), ..self }
    }

    fn points(&self, attachment: EntityAttachment) -> Points {
        match attachment {
            EntityAttachment::Passenger => self.passenger,
            EntityAttachment::Vehicle => self.vehicle,
        }
    }

    fn scaled(&self, point: Vec3) -> Vec3 {
        match self.scale {
            Some([x, y, z]) => point.multiply(x as f64, y as f64, z as f64),
            None => point,
        }
    }

    pub fn get_nullable(&self, attachment: EntityAttachment, index: i32, rot_y: f32) -> Option<Vec3> {
        let points = self.points(attachment);
        (index >= 0 && (index as usize) < points.len()).then(|| transform_point(self.scaled(points.get(index as usize)), rot_y))
    }

    pub fn get(&self, attachment: EntityAttachment, index: i32, rot_y: f32) -> Vec3 {
        self.get_nullable(attachment, index, rot_y).expect("attachment point")
    }

    pub fn get_clamped(&self, attachment: EntityAttachment, index: i32, rot_y: f32) -> Vec3 {
        let points = self.points(attachment);
        let point = points.get(mth::clamp_i(index, 0, points.len() as i32 - 1) as usize);
        transform_point(self.scaled(point), rot_y)
    }
}

fn transform_point(point: Vec3, rot_y: f32) -> Vec3 {
    point.y_rot(-rot_y * mth::DEG_TO_RAD)
}
