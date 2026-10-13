use crisp_core::Vec3;
use crisp_core::random::RandomSource;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectileDeflection {
    None,
    Reverse,
    AimDeflect,
    MomentumDeflect,
}

impl ProjectileDeflection {
    pub fn deflect(self, level: &mut ServerLevel, projectile: EntityId, entity: Option<EntityId>, power: Vec3) {
        match self {
            ProjectileDeflection::None => {}
            ProjectileDeflection::Reverse => {
                let p = level.entity_mut(projectile);
                let rotation = 170.0 + p.random.next_float() * 20.0;
                p.set_delta_movement(p.delta_movement.multiply(-power.x * 0.5, -power.y * 0.5, -power.z * 0.5));
                p.set_y_rot(p.y_rot + rotation);
                p.y_rot_o += rotation;
                p.needs_sync = true;
            }
            ProjectileDeflection::AimDeflect => {
                if let Some(entity) = entity {
                    let look_angle = level.entity(entity).look_angle();
                    let p = level.entity_mut(projectile);
                    p.set_delta_movement(look_angle.multiply(power.x, power.y, power.z));
                    p.needs_sync = true;
                }
            }
            ProjectileDeflection::MomentumDeflect => {
                if let Some(entity) = entity {
                    let movement = level.entity(entity).last_known_speed.normalize();
                    let p = level.entity_mut(projectile);
                    p.set_delta_movement(movement.multiply(power.x, power.y, power.z));
                    p.needs_sync = true;
                }
            }
        }
    }
}
