use crisp_core::{AABB, Vec3};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EntityDimensions {
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
}

impl EntityDimensions {
    pub fn make_bounding_box(self, pos: Vec3) -> AABB {
        let w = (self.width / 2.0) as f64;
        let h = self.height as f64;
        AABB::new(pos.x - w, pos.y, pos.z - w, pos.x + w, pos.y + h, pos.z + w)
    }
}
