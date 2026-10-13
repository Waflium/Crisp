use super::EntityId;
use crate::server::level::ServerLevel;

pub fn not_riding(level: &ServerLevel, entity: EntityId, input: EntityId) -> bool {
    let mut input = input;
    while let Some(vehicle) = level.vehicle(input) {
        input = vehicle;
        if input == entity {
            return false;
        }
    }
    true
}
