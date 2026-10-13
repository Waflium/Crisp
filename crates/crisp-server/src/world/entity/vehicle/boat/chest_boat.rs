use crisp_data::Item;

use super::abstract_boat::{AbstractBoat, BoatKind};

pub const TYPES: [&str; 10] = [
    "minecraft:acacia_chest_boat",
    "minecraft:birch_chest_boat",
    "minecraft:cherry_chest_boat",
    "minecraft:dark_oak_chest_boat",
    "minecraft:jungle_chest_boat",
    "minecraft:mangrove_chest_boat",
    "minecraft:oak_chest_boat",
    "minecraft:pale_oak_chest_boat",
    "minecraft:poplar_chest_boat",
    "minecraft:spruce_chest_boat",
];

pub fn create(entity_type: &str) -> Option<AbstractBoat> {
    TYPES.contains(&entity_type).then(|| AbstractBoat::new(entity_type, BoatKind::ChestBoat, Item::by_name(entity_type).expect("chest boat item")))
}
