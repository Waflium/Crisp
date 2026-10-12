use crisp_core::random::RandomSource;
use crisp_core::Vec3;
use crisp_data::{BlockState, blocks, game_rule, registry, registry_id};

use super::data::{DataValue, entity_defaults};
use super::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType};
use crate::server::level::ServerLevel;
use crate::world::level::server_explosion::{ExplosionDamageCalculator, ExplosionInteraction};

pub const DEFAULT_FUSE_TIME: i32 = 80;
const DATA_FUSE: usize = 8;
const DATA_BLOCK_STATE: usize = 9;

pub struct PrimedTnt {
    pub entity: Entity,
    pub owner: Option<u128>,
    pub used_portal: bool,
    pub explosion_power: f32,
}

impl PrimedTnt {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.98, height: 0.98, eye_height: 0.15 };

    pub fn of_type() -> PrimedTnt {
        let mut data = entity_defaults();
        data.push(DataValue::Int(DEFAULT_FUSE_TIME));
        data.push(DataValue::BlockState(blocks::TNT.default_state()));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:tnt"), Self::DIMENSIONS, data);
        PrimedTnt { entity, owner: None, used_portal: false, explosion_power: 4.0 }
    }

    pub fn new(level: &mut ServerLevel, pos: Vec3, owner: Option<u128>) -> PrimedTnt {
        let mut tnt = PrimedTnt::of_type();
        let entity = &mut tnt.entity;
        entity.set_pos(pos);
        let rot = level.random.next_double() * (std::f32::consts::PI * 2.0) as f64;
        entity.delta_movement = Vec3::new(-rot.sin() * 0.02, 0.2f32 as f64, -rot.cos() * 0.02);
        entity.set_old_pos();
        tnt.owner = owner;
        tnt
    }

    pub fn fuse(&self) -> i32 {
        match self.entity.data.get(DATA_FUSE) {
            DataValue::Int(fuse) => *fuse,
            _ => unreachable!("the fuse is an int"),
        }
    }

    pub fn set_fuse(&mut self, fuse: i32) {
        self.entity.data.set(DATA_FUSE, DataValue::Int(fuse));
    }

    pub fn block_state(&self) -> BlockState {
        match self.entity.data.get(DATA_BLOCK_STATE) {
            DataValue::BlockState(state) => *state,
            _ => unreachable!("the block state is a block state"),
        }
    }

    pub fn set_block_state(&mut self, state: BlockState) {
        self.entity.data.set(DATA_BLOCK_STATE, DataValue::BlockState(state));
    }
}

pub fn random_short_fuse(fuse: i32, random: &mut impl RandomSource) -> i32 {
    random.next_int_bounded((fuse / 4).max(1)) + fuse / 8
}

impl ServerLevel {
    fn primed_tnt(&mut self, id: EntityId) -> &mut PrimedTnt {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::PrimedTnt(tnt)) => tnt,
            _ => unreachable!("not primed TNT"),
        }
    }

    pub fn tick_primed_tnt(&mut self, id: EntityId) {
        self.handle_portal(id);
        self.apply_gravity(id);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        self.apply_effects_from_blocks(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        let drag = self.air_drag(id) as f64;
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(drag));
        if e.on_ground {
            e.set_delta_movement(e.delta_movement.multiply(0.7, -0.5, 0.7));
        }
        let tnt = self.primed_tnt(id);
        let fuse = tnt.fuse() - 1;
        tnt.set_fuse(fuse);
        if fuse <= 0 {
            tnt.entity.discard();
            self.explode_primed_tnt(id);
        } else {
            self.update_fluid_interaction(id);
        }
        let e = self.entity_mut(id);
        e.requires_precise_position = e.horizontal_collision || e.vertical_collision;
    }

    fn explode_primed_tnt(&mut self, id: EntityId) {
        if !self.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            return;
        }
        let tnt = self.primed_tnt(id);
        let (owner, power, used_portal) = (tnt.owner, tnt.explosion_power, tnt.used_portal);
        let e = &tnt.entity;
        let center = Vec3::new(e.x(), e.y() + PrimedTnt::DIMENSIONS.height as f64 * 0.0625, e.z());
        let owner = owner.and_then(|uuid| self.players.values().find(|p| p.profile.uuid == uuid && !p.entity.is_removed()).map(|p| p.entity.id));
        let damage_calculator = if used_portal { ExplosionDamageCalculator::UsedPortal } else { ExplosionDamageCalculator::Default };
        self.explode_with_calculator(Some(id), owner, None, damage_calculator, center, power, false, ExplosionInteraction::Tnt);
    }
}
