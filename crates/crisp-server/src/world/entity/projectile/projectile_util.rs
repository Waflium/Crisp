use crisp_core::{AABB, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::{HitResult, Type};

pub fn compute_margin(tick_count: i32) -> f32 {
    0.0f32.max(0.3f32.min((tick_count - 2) as f32 / 20.0))
}

impl ServerLevel {
    pub fn get_hit_result_on_move_vector(&mut self, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool) -> HitResult {
        self.get_hit_result_on_move_vector_with_clip(source, matching, ClipBlock::Collider)
    }

    pub fn get_hit_result_on_move_vector_with_clip(&mut self, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool, clip_type: ClipBlock) -> HitResult {
        let e = self.entity(source);
        let (movement, from, margin) = (e.delta_movement, e.position(), compute_margin(e.tick_count));
        self.get_hit_result(from, source, matching, movement, margin, clip_type)
    }

    fn get_hit_result(&mut self, from: Vec3, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool, delta: Vec3, entity_margin: f32, clip_type: ClipBlock) -> HitResult {
        let mut to = from.add_vec(delta);
        let ctx = ClipContext { from, to, block: clip_type, fluid: ClipFluid::None, is_player: self.is_player(source) };
        let clip = self.clip_including_border(&ctx);
        let mut hit_result = HitResult::Block { hit: clip.hit, miss: clip.miss };
        if hit_result.get_type() != Type::Miss {
            to = hit_result.get_location();
        }
        let area = self.entity(source).bounding_box().expand_towards_vec(delta).inflate_all(1.0);
        if let Some((entity, location)) = self.get_entity_hit_result(source, from, to, &area, matching, entity_margin) {
            hit_result = HitResult::Entity(EntityHitResult::new(entity, location));
        }
        hit_result
    }

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

    #[allow(clippy::too_many_arguments)]
    pub fn get_many_entity_hit_result(
        &mut self,
        source: EntityId,
        from: Vec3,
        to: Vec3,
        target_search_area: &AABB,
        matching: impl Fn(&ServerLevel, EntityId) -> bool,
        entity_margin: f32,
        clip_type: ClipBlock,
        include_from_entity: bool,
        project_surface_hit_location: bool,
    ) -> Vec<EntityHitResult> {
        let mut collector = Vec::new();
        for entity in self.get_entities(Some(source), target_search_area) {
            if !matching(self, entity) {
                continue;
            }
            let entity_bb = self.entity(entity).bounding_box();
            if include_from_entity && entity_bb.contains_vec(from) {
                collector.push(EntityHitResult::new(entity, from));
                continue;
            }
            if let Some(exact_hit) = entity_bb.clip(from, to) {
                collector.push(EntityHitResult::new(entity, exact_hit));
            } else if !(entity_margin <= 0.0)
                && let Some(outside_hit_position) = entity_bb.inflate_all(entity_margin as f64).clip(from, to)
            {
                let mut towards_target = entity_bb.center();
                let ctx = ClipContext { from: outside_hit_position, to: towards_target, block: clip_type, fluid: ClipFluid::None, is_player: self.is_player(source) };
                let clip = self.clip_including_border(&ctx);
                if !clip.miss {
                    towards_target = clip.hit.location;
                }
                if let Some(surface_hit) = self.entity(entity).bounding_box().clip(outside_hit_position, towards_target) {
                    collector.push(EntityHitResult::new(entity, if project_surface_hit_location { surface_hit } else { outside_hit_position }));
                }
            }
        }
        collector
    }
}
