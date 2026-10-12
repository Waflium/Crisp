use crisp_data::registry;

use super::map_decoration_types::TYPES;
use super::map_index::MapIndex;
use super::map_item_saved_data::MapItemSavedData;

#[test]
fn decoration_types_follow_the_registry() {
    assert_eq!(registry::MAP_DECORATION_TYPE.len(), TYPES.len());
    for (key, ty) in registry::MAP_DECORATION_TYPE.iter().zip(TYPES.iter()) {
        assert_eq!(*key, format!("minecraft:{}", ty.name));
    }
}

#[test]
fn fresh_maps_are_centered_like_vanilla() {
    let data = MapItemSavedData::create_fresh(0.0, 0.0, 0, true, false, "minecraft:overworld");
    assert_eq!((data.center_x, data.center_z), (0, 0));
    let data = MapItemSavedData::create_fresh(100.0, -100.0, 0, true, false, "minecraft:overworld");
    assert_eq!((data.center_x, data.center_z), (128, -128));
    let data = MapItemSavedData::create_fresh(100.0, -100.0, 2, true, false, "minecraft:overworld");
    assert_eq!((data.center_x, data.center_z), (192, -320));
}

#[test]
fn saved_data_round_trips() {
    let mut data = MapItemSavedData::create_fresh(5.0, 7.0, 1, false, true, "minecraft:the_nether");
    data.colors[129] = 42;
    let encoded = data.encode();
    assert_eq!(encoded.get("scale"), Some(&crisp_protocol::nbt::Nbt::Byte(1)));
    assert!(encoded.get("locked").is_none());
    let decoded = MapItemSavedData::decode(&encoded).unwrap();
    assert_eq!(decoded.encode(), encoded);
    let mut index = MapIndex::default();
    assert_eq!(index.encode(), crisp_protocol::nbt::Nbt::Compound(Vec::new()));
    assert_eq!(index.get_next_map_id().id(), 0);
    assert_eq!(MapIndex::decode(&index.encode()).unwrap().get_next_map_id().id(), 1);
}
