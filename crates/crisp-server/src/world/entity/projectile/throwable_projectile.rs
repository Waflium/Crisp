use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::phys::hit_result::Type;

impl ServerLevel {
    pub fn throwable_projectile_tick(&mut self, id: EntityId) {
        self.apply_gravity(id);
        self.throwable_projectile_apply_inertia(id);
        let result = self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity));
        let new_position = if result.get_type() != Type::Miss {
            result.get_location()
        } else {
            let e = self.entity(id);
            e.position().add_vec(e.delta_movement)
        };
        self.entity_mut(id).set_pos(new_position);
        self.entity_moved(id);
        self.update_rotation(id);
        self.apply_effects_from_blocks(id);
        self.projectile_base_tick(id);
        if result.get_type() != Type::Miss && self.is_alive(id) {
            self.hit_target_or_deflect_self(id, result);
        }
    }

    fn throwable_projectile_apply_inertia(&mut self, id: EntityId) {
        let inertia = if self.entity(id).is_in_water() { 0.8f32 } else { self.air_drag(id) };
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(inertia as f64));
    }
}
