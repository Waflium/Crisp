use crisp_data::{Block, BlockState};

use super::mirror::Mirror;
use super::rotation::Rotation;

type Properties = &'static [(&'static str, &'static str)];

fn state(name: &str, properties: &[(&str, &str)]) -> BlockState {
    let block = Block::by_name(&format!("minecraft:{name}")).unwrap();
    properties.iter().fold(block.default_state(), |state, (property, value)| state.with_property_by_name(property, value).unwrap())
}

fn describe(state: BlockState) -> String {
    let properties = state.properties().map(|(name, value)| format!("{name}={value}")).collect::<Vec<_>>().join(",");
    format!("{}[{properties}]", state.block().name())
}

#[test]
fn rotates_like_vanilla() {
    let cases: &[(&str, Properties, Rotation, Properties)] = &[
        ("oak_stairs", &[("facing", "north"), ("shape", "outer_left")], Rotation::Clockwise90, &[("facing", "east"), ("shape", "outer_left")]),
        ("rail", &[("shape", "north_east")], Rotation::Clockwise90, &[("shape", "south_east")]),
        ("rail", &[("shape", "north_east")], Rotation::Counterclockwise90, &[("shape", "north_west")]),
        ("powered_rail", &[("shape", "ascending_east")], Rotation::Clockwise180, &[("shape", "ascending_west")]),
        ("red_bed", &[("facing", "east"), ("part", "head")], Rotation::Clockwise90, &[("facing", "south"), ("part", "head")]),
        ("oak_sign", &[("rotation", "3")], Rotation::Clockwise90, &[("rotation", "7")]),
        ("vine", &[("north", "true"), ("up", "true")], Rotation::Clockwise90, &[("east", "true"), ("up", "true")]),
        ("cobblestone_wall", &[("north", "tall"), ("east", "low")], Rotation::Counterclockwise90, &[("north", "low"), ("east", "none"), ("west", "tall")]),
        ("redstone_wire", &[("north", "side"), ("east", "up"), ("power", "5")], Rotation::Clockwise180, &[("south", "side"), ("west", "up"), ("power", "5")]),
        ("bone_block", &[("axis", "x")], Rotation::Clockwise90, &[("axis", "z")]),
        ("bone_block", &[("axis", "x")], Rotation::Clockwise180, &[("axis", "x")]),
        ("crafter", &[("orientation", "north_up")], Rotation::Clockwise90, &[("orientation", "east_up")]),
        ("crafter", &[("orientation", "down_east")], Rotation::Counterclockwise90, &[("orientation", "down_north")]),
        ("glow_lichen", &[("north", "true"), ("down", "true")], Rotation::Clockwise90, &[("east", "true"), ("down", "true")]),
        ("brown_mushroom_block", &[("north", "false"), ("up", "false")], Rotation::Clockwise90, &[("east", "false"), ("up", "false")]),
        ("nether_portal", &[("axis", "x")], Rotation::Clockwise90, &[("axis", "z")]),
        ("anvil", &[("facing", "north")], Rotation::Counterclockwise90, &[("facing", "west")]),
    ];
    for &(name, properties, rotation, expected) in cases {
        assert_eq!(describe(super::rotate(state(name, properties), rotation)), describe(state(name, expected)), "{name} {properties:?} {rotation:?}");
    }
}

#[test]
fn mirrors_like_vanilla() {
    let cases: &[(&str, Properties, Mirror, Properties)] = &[
        ("oak_stairs", &[("facing", "north"), ("shape", "outer_left")], Mirror::LeftRight, &[("facing", "south"), ("shape", "outer_right")]),
        ("oak_stairs", &[("facing", "east"), ("shape", "inner_left")], Mirror::FrontBack, &[("facing", "west"), ("shape", "inner_left")]),
        ("oak_stairs", &[("facing", "north"), ("shape", "inner_left")], Mirror::FrontBack, &[("facing", "north"), ("shape", "inner_left")]),
        ("rail", &[("shape", "north_east")], Mirror::LeftRight, &[("shape", "south_east")]),
        ("oak_door", &[("facing", "north"), ("hinge", "left")], Mirror::LeftRight, &[("facing", "south"), ("hinge", "right")]),
        ("oak_door", &[("facing", "north"), ("hinge", "left")], Mirror::FrontBack, &[("facing", "north"), ("hinge", "right")]),
        ("red_bed", &[("facing", "east"), ("part", "head")], Mirror::FrontBack, &[("facing", "west"), ("part", "head")]),
        ("chest", &[("facing", "north"), ("type", "left")], Mirror::LeftRight, &[("facing", "south"), ("type", "left")]),
        ("oak_sign", &[("rotation", "3")], Mirror::LeftRight, &[("rotation", "5")]),
        ("oak_sign", &[("rotation", "3")], Mirror::FrontBack, &[("rotation", "13")]),
        ("vine", &[("north", "true"), ("up", "true")], Mirror::LeftRight, &[("south", "true"), ("up", "true")]),
        ("cobblestone_wall", &[("north", "tall"), ("east", "low")], Mirror::FrontBack, &[("north", "tall"), ("east", "none"), ("west", "low")]),
        ("redstone_wire", &[("north", "side"), ("east", "up"), ("power", "5")], Mirror::FrontBack, &[("north", "side"), ("west", "up"), ("power", "5")]),
        ("crafter", &[("orientation", "north_up")], Mirror::FrontBack, &[("orientation", "north_up")]),
        ("glow_lichen", &[("north", "true"), ("down", "true")], Mirror::LeftRight, &[("south", "true"), ("down", "true")]),
        ("end_rod", &[("facing", "east")], Mirror::FrontBack, &[("facing", "west")]),
        ("player_head", &[("rotation", "5")], Mirror::LeftRight, &[("rotation", "3")]),
        ("anvil", &[("facing", "north")], Mirror::LeftRight, &[("facing", "north")]),
    ];
    for &(name, properties, mirror, expected) in cases {
        assert_eq!(describe(super::mirror(state(name, properties), mirror)), describe(state(name, expected)), "{name} {properties:?} {mirror:?}");
    }
}

#[test]
#[ignore]
fn rotate_and_mirror_match_vanilla_dump() {
    let path = std::env::var("CRISP_ROTATION_DUMP").expect("CRISP_ROTATION_DUMP must point at the RotationOracle output");
    let dump = std::fs::read(path).unwrap();
    assert_eq!(dump.len(), BlockState::count() * 14);
    let mut mismatches = 0;
    for id in 0..BlockState::count() {
        let state = BlockState(id as u16);
        let expected = |i: usize| BlockState(u16::from_le_bytes([dump[id * 14 + i * 2], dump[id * 14 + i * 2 + 1]]));
        let rotated = Rotation::VALUES.map(|rotation| super::rotate(state, rotation));
        let mirrored = [Mirror::None, Mirror::LeftRight, Mirror::FrontBack].map(|mirror| super::mirror(state, mirror));
        for (i, actual) in rotated.into_iter().chain(mirrored).enumerate() {
            if actual != expected(i) {
                if mismatches < 50 {
                    eprintln!("{} op {i}: crisp {} vanilla {}", describe(state), describe(actual), describe(expected(i)));
                }
                mismatches += 1;
            }
        }
    }
    eprintln!("{mismatches} mismatches over {} states", BlockState::count());
    assert_eq!(mismatches, 0);
}
