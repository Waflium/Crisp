use crisp_data::Item;

use super::abstract_boat::{AbstractBoat, BoatKind};

pub const TYPES: [&str; 10] = [
    "minecraft:acacia_boat",
    "minecraft:birch_boat",
    "minecraft:cherry_boat",
    "minecraft:dark_oak_boat",
    "minecraft:jungle_boat",
    "minecraft:mangrove_boat",
    "minecraft:oak_boat",
    "minecraft:pale_oak_boat",
    "minecraft:poplar_boat",
    "minecraft:spruce_boat",
];

pub fn create(entity_type: &str) -> Option<AbstractBoat> {
    TYPES.contains(&entity_type).then(|| AbstractBoat::new(entity_type, BoatKind::Boat, Item::by_name(entity_type).expect("boat item")))
}
