use crisp_data::{Item, game_rule};

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, entity_defaults, ids};
use crate::world::entity::{Entity, EntityId, LevelEntity, RemovalReason};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub const DATA_ID_HURT: usize = 8;
pub const DATA_ID_HURTDIR: usize = 9;
pub const DATA_ID_DAMAGE: usize = 10;

pub fn vehicle_entity_defaults() -> Vec<DataValue> {
    let mut values = entity_defaults();
    values.extend([DataValue::Int(0), DataValue::Int(1), DataValue::Float(0.0)]);
    values
}

pub fn set_hurt_time(entity: &mut Entity, hurt_time: i32) {
    entity.data.set(DATA_ID_HURT, DataValue::Int(hurt_time));
}

pub fn set_hurt_dir(entity: &mut Entity, hurt_dir: i32) {
    entity.data.set(DATA_ID_HURTDIR, DataValue::Int(hurt_dir));
}

pub fn set_damage(entity: &mut Entity, damage: f32) {
    entity.data.set(DATA_ID_DAMAGE, DataValue::Float(damage));
}

pub fn get_damage(entity: &Entity) -> f32 {
    match entity.data.get(DATA_ID_DAMAGE) {
        DataValue::Float(v) => *v,
        _ => 0.0,
    }
}

pub fn get_hurt_time(entity: &Entity) -> i32 {
    match entity.data.get(DATA_ID_HURT) {
        DataValue::Int(v) => *v,
        _ => 0,
    }
}

pub fn get_hurt_dir(entity: &Entity) -> i32 {
    match entity.data.get(DATA_ID_HURTDIR) {
        DataValue::Int(v) => *v,
        _ => 0,
    }
}

impl ServerLevel {
    pub fn is_vehicle_entity(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Minecart(_) | LevelEntity::Boat(_)))
    }

    pub fn vehicle_entity_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.entity(id).is_removed() {
            return true;
        }
        if self.is_invulnerable_to_base(id, &source) {
            return false;
        }
        let e = self.entity_mut(id);
        let hurt_dir = get_hurt_dir(e);
        set_hurt_dir(e, -hurt_dir);
        set_hurt_time(e, 10);
        e.sync_velocity = true;
        let damage = get_damage(e) + damage * 10.0;
        set_damage(e, damage);
        let creative_player = source.is_creative_player(self);
        if (creative_player || !(get_damage(self.entity(id)) > 40.0)) && !self.should_source_destroy(id, &source) {
            if creative_player {
                self.remove(id, RemovalReason::Discarded);
            }
        } else {
            self.destroy_by_source(id, &source);
        }
        true
    }

    fn should_source_destroy(&self, _id: EntityId, _source: &DamageSource) -> bool {
        false
    }

    pub fn destroy(&mut self, id: EntityId, drop_item: Item) {
        self.remove(id, RemovalReason::Killed);
        if self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
            let mut item_stack = ItemStack::new(drop_item, 1);
            let custom_name = match self.entity(id).data.get(ids::CUSTOM_NAME) {
                DataValue::OptionalComponent(name) => name.clone(),
                _ => None,
            };
            item_stack.set(dc::CUSTOM_NAME, custom_name);
            self.spawn_at_location(id, item_stack, 0.0);
        }
    }

    fn destroy_by_source(&mut self, id: EntityId, source: &DamageSource) {
        if self.is_container_entity(id) {
            self.abstract_chest_boat_destroy(id, source);
            return;
        }
        let drop_item = self.vehicle_drop_item(id);
        self.destroy(id, drop_item);
    }

    pub fn vehicle_drop_item(&self, id: EntityId) -> Item {
        match self.entities.get(&id) {
            Some(LevelEntity::Minecart(minecart)) => minecart.get_drop_item(),
            Some(LevelEntity::Boat(boat)) => boat.get_drop_item(),
            _ => unreachable!("not a vehicle"),
        }
    }
}
