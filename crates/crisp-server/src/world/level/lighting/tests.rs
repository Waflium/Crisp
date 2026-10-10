use std::path::PathBuf;

use crisp_core::{BlockPos, ChunkPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, SLAB_TYPE, SlabType};
use crisp_data::{BlockState, DimensionType, blocks};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;
use sha2::{Digest, Sha256};

use super::*;
use crate::server::level::chunk_cache::ChunkCache;
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::chunk::{FlatLevelSource, serialization};

type Chunks = &'static [((i32, i32), &'static str)];
type Probes = &'static [((i32, i32, i32), i32, i32)];

const FLAT: &str = "216a129a13398d6e504027f459359d9b7b71fac54c263b05376ef06cbb258172";

fn overworld() -> &'static DimensionType {
    &crisp_data::DIMENSION_TYPES[0]
}

fn light_hash(cache: &ChunkCache, x: i32, z: i32) -> String {
    let mut w = Writer::new();
    write_light_data(&mut w, ChunkPos::new(x, z), &cache.light_engine.engine, None, None);
    Sha256::digest(w.as_slice()).iter().map(|b| format!("{b:02x}")).collect()
}

fn check(cache: &ChunkCache, name: &str, chunks: Chunks, probes: Probes) {
    for &((x, z), hash) in chunks {
        assert_eq!(light_hash(cache, x, z), hash, "{name}: light of chunk {x}, {z}");
    }
    let engine = &cache.light_engine.engine;
    for &((x, y, z), sky, block) in probes {
        let pos = BlockPos::new(x, y, z);
        let light = (engine.light_value(LightLayer::Sky, pos), engine.light_value(LightLayer::Block, pos));
        assert_eq!(light, (sky, block), "{name}: sky and block light at {x}, {y}, {z}");
    }
}

#[test]
fn flat_world_sky_light() {
    let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), None);
    cache.prepare_ticking_chunk(ChunkPos::new(0, 0));
    let chunks: Vec<_> = (-1..=1).flat_map(|z| (-1..=1).map(move |x| ((x, z), FLAT))).collect();
    let chunks: Chunks = chunks.leak();
    check(&cache, "flat", chunks, &[((8, -60, 8), 15, 0), ((8, -61, 8), 0, 0), ((8, 300, 8), 15, 0), ((-8, -60, 20), 15, 0)]);
}

#[test]
fn sky_light_through_leaves_water_and_glass() {
    let layers = [
        (blocks::BEDROCK, 1),
        (blocks::DIRT, 2),
        (blocks::WATER, 4),
        (blocks::OAK_LEAVES, 3),
        (blocks::GLASS, 1),
    ];
    let generator = FlatLevelSource { layers: layers.iter().map(|&(b, n)| (b.default_state(), n)).collect(), ..FlatLevelSource::default() };
    let mut cache = ChunkCache::new(overworld(), generator, None);
    cache.prepare_ticking_chunk(ChunkPos::new(0, 0));
    const LAYERS: &str = "c00d50e8cbb50a34302d3ed2ff18eca56cce578306becf47e45eecdaf5924b6d";
    let probes: Probes = &[
        ((8, -52, 8), 15, 0),
        ((8, -53, 8), 15, 0),
        ((8, -54, 8), 15, 0),
        ((8, -55, 8), 14, 0),
        ((8, -56, 8), 13, 0),
        ((8, -57, 8), 12, 0),
        ((8, -58, 8), 11, 0),
        ((8, -59, 8), 10, 0),
        ((8, -60, 8), 9, 0),
        ((8, -61, 8), 8, 0),
        ((8, -62, 8), 0, 0),
    ];
    check(&cache, "layers", &[((0, 0), LAYERS), ((1, 0), LAYERS), ((0, 1), LAYERS), ((1, 1), LAYERS)], probes);
}

fn slab(slab_type: SlabType) -> BlockState {
    blocks::SMOOTH_STONE_SLAB.default_state().set(SLAB_TYPE, slab_type)
}

fn steps() -> Vec<Vec<(BlockPos, BlockState)>> {
    let at = |x, y, z, state| (BlockPos::new(x, y, z), state);
    let torch = vec![at(8, -60, 8, blocks::TORCH.default_state())];
    let mut overhang = Vec::new();
    for x in 13..=17 {
        for z in 6..=10 {
            overhang.push(at(x, -57, z, blocks::STONE.default_state()));
        }
    }
    let mut blockers = vec![
        at(3, -60, 3, blocks::GLASS.default_state()),
        at(3, -59, 3, blocks::GLASS.default_state()),
        at(5, -60, 3, blocks::OAK_LEAVES.default_state()),
        at(5, -59, 3, blocks::OAK_LEAVES.default_state()),
        at(5, -58, 3, blocks::OAK_LEAVES.default_state()),
        at(7, -60, 3, blocks::WATER.default_state()),
        at(8, -59, 8, slab(SlabType::Bottom)),
        at(12, -60, 2, blocks::TORCH.default_state()),
        at(12, -59, 2, slab(SlabType::Top)),
        at(13, -60, 2, blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::East)),
        at(11, -60, 2, blocks::OAK_STAIRS.default_state()),
    ];
    for x in 4..=6 {
        for z in 12..=14 {
            blockers.push(at(x, -58, z, slab(SlabType::Bottom)));
        }
    }
    blockers.push(at(5, -60, 13, blocks::GLOWSTONE.default_state()));
    let air = blocks::AIR.default_state();
    let mut removals = vec![at(8, -60, 8, air)];
    for x in 13..=15 {
        for z in 6..=10 {
            removals.push(at(x, -57, z, air));
        }
    }
    removals.push(at(12, -59, 2, air));
    removals.push(at(5, -58, 13, air));
    vec![torch, overhang, blockers, removals]
}

fn area(hashes: [&'static str; 12]) -> Vec<((i32, i32), &'static str)> {
    (-1..=1).flat_map(|z| (-1..=2).map(move |x| (x, z))).zip(hashes).collect()
}

const STEP_1: [&str; 12] = [
    FLAT,
    "8709d1135a533f85bbfb901f4a4369065e67aa036d3564d64fc1c52b8dd66ea3",
    FLAT,
    FLAT,
    "bd62e4a73ef39cd77bcc6ed3fe1e42f338d1f47b89aeeffa875e258a1b98a871",
    "0ce0d1193a315047a535fdb23c03e97c1b143674f11f32eee92c28f610c10dba",
    "cb6535133233906ce91a2b55eb2acb5bcb5301eabf5a34314c9d98c727dfdf62",
    FLAT,
    FLAT,
    "e5475ddb1abbd82005afc1795f1791f543cbdef9c8818e0548839f9c9a0595cb",
    FLAT,
    FLAT,
];

const STEP_2: [&str; 12] = [
    FLAT,
    "8709d1135a533f85bbfb901f4a4369065e67aa036d3564d64fc1c52b8dd66ea3",
    FLAT,
    FLAT,
    "bd62e4a73ef39cd77bcc6ed3fe1e42f338d1f47b89aeeffa875e258a1b98a871",
    "3fac937388ab9e18e61f0b415c905ad9f10526d8b82d46a2d8acc0d52ed06646",
    "ce44aca97c23ee165a1d62cd94043b610fd5297bfebe5d6f3927e1e2e032eaac",
    FLAT,
    FLAT,
    "e5475ddb1abbd82005afc1795f1791f543cbdef9c8818e0548839f9c9a0595cb",
    FLAT,
    FLAT,
];

const STEP_3: [&str; 12] = [
    FLAT,
    "a5c806b62b71666f006afc9ecc26117ac9ef5069ff9f268260216808f5a4db55",
    "b9b021a55b7609f8d0f158a70a6c31caddbb512b7bfc2552b10e3ae6541af17c",
    FLAT,
    "f168848c714171fb21876e121ebaf5af2daf77f1e194521fd1489286cbe43270",
    "4947c4fb02d193f63495efd78ab3a567046861a52cfc5ff2f95a2dab31124477",
    "1e9eaf8b5c3adfb233f6019857894ac952170835a2531dffc2273bece8e98430",
    FLAT,
    "872eb3153e81552a690a41fe952667cb18e60811e6b7d1e3080791a6990b9fa0",
    "d5536c702657b7f4cf7f79b4ebde072b5c02129877401210cbf11acb2fc6c78c",
    "10f29e28864cc64681e0af2f9a3a9398bc8d95c1ddf43638895f0c9967e660b7",
    FLAT,
];

const STEP_4: [&str; 12] = [
    FLAT,
    "a5c806b62b71666f006afc9ecc26117ac9ef5069ff9f268260216808f5a4db55",
    "b9b021a55b7609f8d0f158a70a6c31caddbb512b7bfc2552b10e3ae6541af17c",
    FLAT,
    "7eef8d0aa8c03a8d342c1b837976c53b7b9477475495da90ac8ed1e27739dc99",
    "6c7bc459b8f0c5375507a4235c7d2602427168f5b41bafd4963563499cf8bde9",
    "7c292d99876266254184534a2f73edd8793d7e0137b7ff6ca1fac5ffb157cb3d",
    FLAT,
    "872eb3153e81552a690a41fe952667cb18e60811e6b7d1e3080791a6990b9fa0",
    "d5536c702657b7f4cf7f79b4ebde072b5c02129877401210cbf11acb2fc6c78c",
    "10f29e28864cc64681e0af2f9a3a9398bc8d95c1ddf43638895f0c9967e660b7",
    FLAT,
];

const PROBES: [(i32, i32, i32); 27] = [
    (8, -60, 8),
    (8, -59, 8),
    (8, -58, 8),
    (10, -60, 8),
    (8, -61, 8),
    (15, -58, 8),
    (15, -60, 8),
    (17, -60, 8),
    (19, -60, 8),
    (12, -60, 8),
    (3, -60, 3),
    (3, -61, 3),
    (5, -61, 3),
    (7, -61, 3),
    (7, -60, 3),
    (12, -60, 2),
    (12, -59, 2),
    (12, -58, 2),
    (13, -60, 2),
    (14, -60, 2),
    (11, -60, 2),
    (10, -60, 2),
    (5, -59, 13),
    (5, -58, 13),
    (5, -57, 13),
    (4, -59, 12),
    (7, -59, 13),
];

const PROBED: [[(i32, i32); 27]; 4] = [
    [
        (15, 14),
        (15, 13),
        (15, 12),
        (15, 12),
        (0, 0),
        (15, 5),
        (15, 7),
        (15, 5),
        (15, 3),
        (15, 10),
        (15, 4),
        (0, 0),
        (0, 0),
        (0, 0),
        (15, 8),
        (15, 4),
        (15, 3),
        (15, 2),
        (15, 3),
        (15, 2),
        (15, 5),
        (15, 6),
        (15, 5),
        (15, 4),
        (15, 3),
        (15, 5),
        (15, 7),
    ],
    [
        (15, 14),
        (15, 13),
        (15, 12),
        (15, 12),
        (0, 0),
        (12, 5),
        (12, 7),
        (14, 5),
        (15, 3),
        (15, 10),
        (15, 4),
        (0, 0),
        (0, 0),
        (0, 0),
        (15, 8),
        (15, 4),
        (15, 3),
        (15, 2),
        (15, 3),
        (15, 2),
        (15, 5),
        (15, 6),
        (15, 5),
        (15, 4),
        (15, 3),
        (15, 5),
        (15, 7),
    ],
    [
        (14, 14),
        (15, 11),
        (15, 10),
        (15, 12),
        (0, 0),
        (12, 5),
        (12, 7),
        (14, 5),
        (15, 3),
        (15, 10),
        (15, 4),
        (0, 0),
        (0, 0),
        (0, 0),
        (14, 8),
        (14, 14),
        (14, 13),
        (15, 10),
        (15, 13),
        (15, 10),
        (15, 13),
        (15, 12),
        (13, 14),
        (15, 9),
        (15, 8),
        (14, 12),
        (15, 12),
    ],
    [
        (14, 7),
        (15, 6),
        (15, 5),
        (15, 6),
        (0, 0),
        (15, 3),
        (15, 5),
        (14, 3),
        (15, 1),
        (15, 8),
        (15, 4),
        (0, 0),
        (0, 0),
        (0, 0),
        (14, 8),
        (15, 14),
        (15, 13),
        (15, 12),
        (15, 13),
        (15, 10),
        (15, 13),
        (15, 12),
        (15, 14),
        (15, 13),
        (15, 12),
        (14, 12),
        (15, 12),
    ],
];

fn probes(step: usize) -> Probes {
    let probes: Vec<_> = PROBES.iter().zip(PROBED[step]).map(|(&pos, (sky, block))| (pos, sky, block)).collect();
    probes.leak()
}

fn build_incrementally(storage: Option<RegionStorage>) -> ChunkCache {
    let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), storage);
    cache.prepare_ticking_chunk(ChunkPos::new(0, 0));
    cache.prepare_ticking_chunk(ChunkPos::new(1, 0));
    cache.run_until_idle();
    let expected = [STEP_1, STEP_2, STEP_3, STEP_4];
    for (i, step) in steps().into_iter().enumerate() {
        for (pos, state) in step {
            cache.set_block_state(pos, state);
        }
        cache.run_light_updates();
        check(&cache, &format!("step {}", i + 1), area(expected[i]).leak(), probes(i));
    }
    cache
}

#[test]
fn light_follows_placed_and_removed_blocks() {
    build_incrementally(None);
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("crisp-light-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn chunks_without_light_are_lit_from_scratch() {
    let mut built = build_incrementally(None);
    let dir = temp_dir("unlit");
    let mut storage = RegionStorage::new(dir.clone(), false);
    for x in -2..=3 {
        for z in -2..=2 {
            let Some(chunk) = built.chunk_if_present(ChunkPos::new(x, z)) else { continue };
            let mut nbt = serialization::write(chunk, 0, &[], &[], &[]);
            if let Nbt::Compound(fields) = &mut nbt {
                fields.retain(|(key, _)| key != "isLightOn");
            }
            storage.write(ChunkPos::new(x, z), &nbt).unwrap();
        }
    }
    storage.flush().unwrap();
    let mut cache = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
    cache.prepare_ticking_chunk(ChunkPos::new(0, 0));
    cache.prepare_ticking_chunk(ChunkPos::new(1, 0));
    check(&cache, "from scratch", area(STEP_4).leak(), probes(3));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saved_light_is_loaded_back() {
    let dir = temp_dir("saved");
    let mut built = build_incrementally(Some(RegionStorage::new(dir.clone(), false)));
    for x in -2..=3 {
        for z in -2..=2 {
            if let Some(chunk) = built.chunk_if_present(ChunkPos::new(x, z)) {
                chunk.unsaved = true;
            }
        }
    }
    built.save(0).unwrap();
    let mut loaded = ChunkCache::new(overworld(), FlatLevelSource::default(), Some(RegionStorage::new(dir.clone(), false)));
    loaded.prepare_ticking_chunk(ChunkPos::new(0, 0));
    loaded.prepare_ticking_chunk(ChunkPos::new(1, 0));
    loaded.run_until_idle();
    assert!(loaded.chunk_if_present(ChunkPos::new(0, 0)).unwrap().light_correct);
    let (a, b) = (&built.light_engine.engine, &loaded.light_engine.engine);
    for ((x, z), _) in area(STEP_4) {
        for section_y in a.min_light_section()..a.min_light_section() + a.light_section_count() as i32 {
            for layer in [LightLayer::Sky, LightLayer::Block] {
                let node = section::of(x, section_y, z);
                let values = |e: &LevelLightEngine| e.data_layer_data(layer, node).map(|d| d.to_bytes());
                assert_eq!(values(a), values(b), "{layer:?} light of section {x}, {section_y}, {z}");
            }
        }
    }
    check(&loaded, "loaded", &[], probes(3));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn light_shapes_are_whole_pixels() {
    for id in 0..BlockState::count() as u32 {
        let state = BlockState::by_id(id).unwrap();
        for b in state.occlusion_shape() {
            for c in [b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z] {
                assert!((c * 16.0 - (c * 16.0).round()).abs() < 1.0e-9, "{state:?} has an occlusion box edge at {c}");
            }
        }
    }
}

#[test]
fn slab_and_stair_faces_occlude() {
    let bottom = slab(SlabType::Bottom);
    let top = slab(SlabType::Top);
    let air = blocks::AIR.default_state();
    assert!(shape_occludes(air, bottom, Direction::Up));
    assert!(!shape_occludes(air, bottom, Direction::Down));
    assert!(shape_occludes(air, top, Direction::Down));
    assert!(!shape_occludes(bottom, bottom, Direction::East));
    assert!(shape_occludes(bottom, top, Direction::East));
    let stairs = blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::East);
    assert!(shape_occludes(air, stairs, Direction::West));
    assert!(!shape_occludes(air, stairs, Direction::East));
}
