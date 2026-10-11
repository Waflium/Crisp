use crisp_core::{AABB, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

impl ServerLevel {
    pub fn get_entity_hit_result(
        &self,
        source: EntityId,
        from: Vec3,
        to: Vec3,
        target_search_area: &AABB,
        matching: impl Fn(&ServerLevel, EntityId) -> bool,
        entity_margin: f32,
    ) -> Option<(EntityId, Vec3)> {
        let mut nearest = f64::MAX;
        let mut hit: Option<(EntityId, Vec3)> = None;
        for entity in self.get_entities(Some(source), target_search_area) {
            if !matching(self, entity) {
                continue;
            }
            let bb = self.entity(entity).bounding_box().inflate_all(entity_margin as f64);
            if let Some(location) = bb.clip(from, to) {
                let dd = from.distance_to_sqr(location);
                if dd < nearest {
                    hit = Some((entity, location));
                    nearest = dd;
                }
            }
        }
        hit
    }
}
