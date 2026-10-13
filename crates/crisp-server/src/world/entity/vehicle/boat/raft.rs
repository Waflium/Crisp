use crisp_data::Item;

use super::abstract_boat::{AbstractBoat, BoatKind};

pub const TYPES: [&str; 1] = ["minecraft:bamboo_raft"];

pub fn create(entity_type: &str) -> Option<AbstractBoat> {
    TYPES.contains(&entity_type).then(|| AbstractBoat::new(entity_type, BoatKind::Raft, Item::by_name(entity_type).expect("raft item")))
}
