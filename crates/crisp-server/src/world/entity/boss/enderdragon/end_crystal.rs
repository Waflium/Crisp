use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};
use crisp_data::{damage_type_tags, damage_types, registry, registry_id};

use crate::server::THE_END;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, RemovalReason};
use crate::world::level::BlockGetter;
use crate::world::level::block::base_fire_block;
use crate::world::level::server_explosion::ExplosionInteraction;

const DATA_BEAM_TARGET: usize = 8;
const DATA_SHOW_BOTTOM: usize = 9;
const DEFAULT_SHOW_BOTTOM: bool = true;

pub struct EndCrystal {
    pub entity: Entity,
    pub time: i32,
}

impl EndCrystal {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 2.0, height: 2.0, eye_height: 2.0 * 0.85 };

    pub fn of_type() -> EndCrystal {
        let mut data = entity_defaults();
        data.push(DataValue::OptionalBlockPos(None));
        data.push(DataValue::Boolean(DEFAULT_SHOW_BOTTOM));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:end_crystal"), Self::DIMENSIONS, data);
        let time = entity.random.next_int_bounded(100000);
        EndCrystal { entity, time }
    }

    pub fn set_beam_target(&mut self, target: Option<BlockPos>) {
        self.entity.data.set(DATA_BEAM_TARGET, DataValue::OptionalBlockPos(target));
    }

    pub fn get_beam_target(&self) -> Option<BlockPos> {
        match self.entity.data.get(DATA_BEAM_TARGET) {
            DataValue::OptionalBlockPos(target) => *target,
            _ => None,
        }
    }

    pub fn set_show_bottom(&mut self, show_bottom: bool) {
        self.entity.data.set(DATA_SHOW_BOTTOM, DataValue::Boolean(show_bottom));
    }

    pub fn shows_bottom(&self) -> bool {
        matches!(self.entity.data.get(DATA_SHOW_BOTTOM), DataValue::Boolean(true))
    }
}

impl ServerLevel {
    fn end_crystal(&mut self, id: EntityId) -> &mut EndCrystal {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::EndCrystal(crystal)) => crystal,
            _ => unreachable!("not an end crystal"),
        }
    }

    pub fn tick_end_crystal(&mut self, id: EntityId) {
        self.end_crystal(id).time += 1;
        self.apply_effects_from_blocks(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        self.handle_portal(id);
        let pos = self.entity(id).block_position();
        if self.dimension == THE_END && self.block_state(pos).is_air() {
            let fire = base_fire_block::get_state(self, pos);
            self.set_block(pos, fire, crate::world::level::update_flags::UPDATE_ALL);
        }
    }

    pub fn end_crystal_hurt_server(&mut self, id: EntityId, source: DamageSource) -> bool {
        if self.is_invulnerable_to_base(id, &source) {
            return false;
        }
        if source.causing_entity.is_some_and(|e| self.entity_ref(e).is_some_and(|e| e.entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:ender_dragon"))) {
            return false;
        }
        if !self.entity(id).is_removed() {
            self.entity_mut(id).removal = Some(RemovalReason::Killed);
            if !source.is(&damage_type_tags::IS_EXPLOSION) {
                let damage_source = source.causing_entity.map(|causing| DamageSource {
                    ty: damage_types::PLAYER_EXPLOSION,
                    causing_entity: Some(causing),
                    direct_entity: Some(id),
                    damage_source_position: None,
                });
                let e = self.entity(id);
                let center = Vec3::new(e.x(), e.y(), e.z());
                self.explode(Some(id), None, damage_source, center, 6.0, false, ExplosionInteraction::Block);
            }
        }
        true
    }
}
