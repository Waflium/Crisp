use std::collections::HashMap;

use crisp_core::ChunkPos;
use crisp_data::DimensionType;

use super::NoiseBasedChunkGenerator;
use super::material::WorldGenerationContext;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::chunk::{HeightmapType, LevelChunk};
use crate::world::level::chunk::proto_chunk::ProtoChunk;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv_int(mut hash: u64, value: i32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn fnv_str(mut hash: u64, value: &str) -> u64 {
    for byte in value.bytes().chain([0]) {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn overworld() -> &'static DimensionType {
    &crisp_data::DIMENSION_TYPES[crisp_data::SyncedRegistry::get("minecraft:dimension_type").id_of("minecraft:overworld").unwrap() as usize]
}

pub struct Hashes {
    pub blocks: u64,
    pub biomes: u64,
    pub heights: u64,
}

pub fn hashes(chunk: &mut ProtoChunk) -> Hashes {
    let mut blocks = FNV_OFFSET;
    for y in chunk.min_y..=chunk.max_y() {
        for z in 0..16 {
            for x in 0..16 {
                blocks = fnv_int(blocks, chunk.block_state(x, y, z).id() as i32);
            }
        }
    }
    let mut biomes = FNV_OFFSET;
    let (qx, qz) = (chunk.pos.min_block_x() >> 2, chunk.pos.min_block_z() >> 2);
    for qy in (chunk.min_y >> 2)..((chunk.max_y() + 1) >> 2) {
        for x in 0..4 {
            for z in 0..4 {
                biomes = fnv_str(biomes, chunk.noise_biome(qx + x, qy, qz + z).name());
            }
        }
    }
    let mut heights = FNV_OFFSET;
    for kind in HeightmapType::LIVE {
        for z in 0..16 {
            for x in 0..16 {
                heights = fnv_int(heights, chunk.height_at(kind, x, z));
            }
        }
    }
    Hashes { blocks, biomes, heights }
}


fn check(fixture: &str) {
    let mut generators: HashMap<i64, NoiseBasedChunkGenerator> = HashMap::new();
    let mut failures = Vec::new();
    for line in fixture.lines().map(|l| l.split('#').next().unwrap().trim()).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let seed: i64 = f[0].parse().unwrap();
        let pos = ChunkPos::new(f[1].parse().unwrap(), f[2].parse().unwrap());
        let generator = generators.entry(seed).or_insert_with(|| NoiseBasedChunkGenerator::overworld(seed));
        let mut chunk = generator.generate(pos, overworld());
        let h = hashes(&mut chunk);
        let got = format!("{seed} {} {} {:016x} {:016x} {:016x}", pos.x, pos.z, h.blocks, h.biomes, h.heights);
        if got != line {
            failures.push(format!("expected {line}\n     got {got}"));
        }
    }
    assert!(failures.is_empty(), "chunks differ from vanilla:\n{}", failures.join("\n"));
}

pub fn decorate(generator: &NoiseBasedChunkGenerator, chunks: &mut HashMap<ChunkPos, LevelChunk>, center: ChunkPos) {
    let dimension = overworld();
    let mut taken: Vec<(ChunkPos, LevelChunk)> = Vec::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let pos = ChunkPos::new(center.x + dx, center.z + dz);
            let chunk = chunks.remove(&pos).unwrap_or_else(|| generator.generate(pos, dimension).into_level_chunk(dimension));
            taken.push((pos, chunk));
        }
    }
    {
        let mut iter = taken.iter_mut();
        let cache: [Option<&mut LevelChunk>; 9] = std::array::from_fn(|_| iter.next().map(|(_, c)| c));
        let generation = WorldGenerationContext {
            min_gen_y: dimension.min_y.max(generator.min_y()),
            gen_depth: (dimension.height as i32).min(generator.gen_depth()),
            sea_level: generator.sea_level(),
        };
        let mut region = WorldGenRegion::new(center, cache, [true; 9], [false; 9], &generator.state, dimension.min_y, dimension.height as i32, generation);
        generator.apply_biome_decoration(&mut region);
    }
    chunks.extend(taken);
}

pub fn decorate_like_oracle(generator: &NoiseBasedChunkGenerator, pos: ChunkPos) -> LevelChunk {
    let dimension = overworld();
    let mut chunks: HashMap<ChunkPos, LevelChunk> = HashMap::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let center = ChunkPos::new(pos.x + dx, pos.z + dz);
            for nz in -1..=1 {
                for nx in -1..=1 {
                    let p = ChunkPos::new(center.x + nx, center.z + nz);
                    chunks.entry(p).or_insert_with(|| generator.generate(p, dimension).into_level_chunk(dimension));
                }
            }
            decorate(generator, &mut chunks, center);
        }
    }
    chunks.remove(&pos).expect("the decorated chunk")
}

#[test]
#[ignore]
fn features_match_vanilla_dump() {
    let dir = std::env::var("CRISP_FEATURE_DUMP").expect("CRISP_FEATURE_DUMP");
    let mut report = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "blocks") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let f: Vec<i64> = stem.split('_').map(|s| s.parse().unwrap()).collect();
        let (seed, pos) = (f[0], ChunkPos::new(f[1] as i32, f[2] as i32));
        let generator = NoiseBasedChunkGenerator::overworld(seed);
        let chunk = decorate_like_oracle(&generator, pos);
        let bytes = std::fs::read(&path).unwrap();
        let mut mismatches: HashMap<(String, String), usize> = HashMap::new();
        let mut i = 0;
        for y in chunk.min_y()..chunk.min_y() + chunk.height() {
            for z in 0..16 {
                for x in 0..16 {
                    let expected = i32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()) as u32;
                    i += 1;
                    let actual = chunk.block_state(crisp_core::BlockPos::new(pos.min_block_x() + x, y, pos.min_block_z() + z));
                    if actual.id() != expected {
                        let expected = crisp_data::BlockState::by_id(expected).map_or("?".into(), |s| s.block().name().to_string());
                        *mismatches.entry((expected, actual.block().name().to_string())).or_default() += 1;
                    }
                }
            }
        }
        let mut mismatches: Vec<_> = mismatches.into_iter().collect();
        mismatches.sort_by_key(|m| std::cmp::Reverse(m.1));
        report.push(format!("{stem}: {} differing blocks {:?}", mismatches.iter().map(|m| m.1).sum::<usize>(), &mismatches[..mismatches.len().min(12)]));
    }
    eprintln!("{}", report.join("\n"));
}

#[test]
#[ignore]
fn throughput() {
    let generator = NoiseBasedChunkGenerator::overworld(12345);
    let positions: Vec<ChunkPos> = (0..20).flat_map(|x| (0..20).map(move |z| ChunkPos::new(x - 10, z - 10))).collect();
    let start = std::time::Instant::now();
    for &pos in &positions {
        generator.generate(pos, overworld());
    }
    let single = positions.len() as f64 / start.elapsed().as_secs_f64();
    let start = std::time::Instant::now();
    for &pos in &positions {
        generator.create_biomes(ChunkPos::new(pos.x + 100, pos.z), overworld());
    }
    eprintln!("biomes alone: {:.0} chunks/s", positions.len() as f64 / start.elapsed().as_secs_f64());
    let generator = NoiseBasedChunkGenerator::overworld(54321);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let start = std::time::Instant::now();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                while let Some(&pos) = positions.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
                    generator.generate(pos, overworld());
                }
            });
        }
    });
    let parallel = positions.len() as f64 / start.elapsed().as_secs_f64();
    eprintln!("{single:.0} chunks/s on one thread, {parallel:.0} chunks/s on {threads} threads");
}

#[test]
fn generator_is_shareable_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NoiseBasedChunkGenerator>();
}

#[test]
fn spawn_origin_matches_vanilla() {
    for (seed, x, z) in [(12345, 6, -2), (-4172144997902289642, -28, -27), (0, -2, 0)] {
        assert_eq!(NoiseBasedChunkGenerator::overworld(seed).origin(), ChunkPos::new(x, z), "seed {seed}");
    }
}

#[test]
fn terrain_matches_vanilla() {
    check(
        "
        12345 0 0 c35ea558ff38ef0f 36904c9711f99c25 62c2237a1214cd67 # [lush_caves, ocean] {lava=39, deepslate_iron_ore=12}
        12345 4 -4 aeeb60c0be0d3650 16d92042f5bd7fd3 5784fcd85243d0be # [dark_forest, deep_dark, lush_caves] {}
        12345 -168 112 ce20e057cd49cc4f 0b3d58d6f13ffda5 0c04628b69b42fa5 # [badlands, eroded_badlands] {deepslate_iron_ore=7}
        12345 4 24 0b8b52cb3ccabbd4 6b15b65172be35cd ee0deecf60d596c9 # [deep_ocean, lush_caves] {}
        12345 -24 -12 82ffd1f40ad51088 b160a42d85fc9619 d3148b1c939eb290 # [deep_dark, jagged_peaks, ocean] {deepslate_iron_ore=5, lava=71}
        12345 24 36 4127ba8bad4af0a0 9850cb53adfadb3f c0893a7856c3c125 # [deep_dark, frozen_peaks] {snow_block=258, packed_ice=415}
        12345 -44 -16 a68db1fa4c98801b b16890c8ccef281d 78b5a72dbbcaaba5 # [deep_dark, stony_peaks] {}
        12345 -108 -108 fa09435754752ae8 1537203b5c63003e b56834eaffe8e486 # [dark_forest, lush_caves, swamp] {}
        12345 -108 -88 282a84e349db68f3 4699b13963d01967 8a1481f6d27f8925 # [dripstone_caves, lush_caves, mangrove_swamp] {deepslate_iron_ore=96, raw_iron_block=2, lava=19}
        12345 -264 -264 943cd712dca0392b 0e18141a876691ac c82075ba82f2e365 # [dripstone_caves, wooded_badlands] {}
        12345 80 44 fd284b5c09ab52a1 8fe743a601163515 89b2dbf67972cced # [frozen_ocean, sulfur_caves] {}
        12345 -1 -1 ffccc6cbf94e4b85 36904c9711f99c25 0cb764e09631910f # [lush_caves, ocean] {lava=94, deepslate_iron_ore=1}
        12345 40 16 17f4b20862cd7c31 6e544a030ac15a85 5268a669e046a1a9 # [cold_ocean, lush_caves] {deepslate_iron_ore=178, raw_iron_block=1}
        12345 46 24 9f104167ee90c6f3 93126513475b9608 063d56f99df70339 # [frozen_ocean, lush_caves, snowy_beach] {copper_ore=31, packed_ice=5858, snow_block=246}
        12345 22 100 733e926dd5642a0d 254d18baf73b78c4 ddbaacbc9feff5dc # [deep_frozen_ocean, lush_caves] {deepslate_iron_ore=62, raw_iron_block=1, packed_ice=267}
        12345 16 98 d98be2f72daadb39 998a56bf1a3ec2e7 c125a358d3a6f5bd # [deep_frozen_ocean, frozen_ocean, lush_caves] {packed_ice=4406, snow_block=92}
        12345 8 -8 f94d09dab05a62e4 88abd1c776c22663 8f6d8a8c13597fb5 # [deep_dark, frozen_peaks, grove, lush_caves] {packed_ice=739, snow_block=302}
        -4172144997902289642 0 28 ba1f41526b2e89e1 7f0cf8ba629af025 b93ead0d96e14a83 # [deep_lukewarm_ocean, warm_ocean] {}
        -4172144997902289642 -36 -36 b0207bbdd33bf91b 4cb6a29258415b25 42f7a87bbc71f025 # [swamp] {lava=5}
        -4172144997902289642 -56 20 aaf17af5ac61113f 4b10fdd28f883a91 0e92912382ccd0e3 # [desert, lush_caves] {lava=12}
        -4172144997902289642 84 -40 342d4f1b9cea30e6 e0db10b86b33a85f 9829e0b27ef4c0e5 # [deep_dark, forest, frozen_peaks] {packed_ice=80, snow_block=30}
        -4172144997902289642 148 -88 1b091f67ddd49149 5999f3920c2b3c65 b1299e44a11a1225 # [badlands, savanna] {}
        -4172144997902289642 228 -44 0895572b5fe92072 b930ccf143d1f325 d9604eb5c05f7725 # [eroded_badlands] {}
        0 0 0 4fab78d6940496a6 242c08e26cf29a45 b2a9e2ae4b53c5ce # [forest, river] {}
        0 -30 17 9fa18e6f38172d9e a2b7d896db159325 aeaa3ec0e92c7065 # [forest] {lava=2}
        ",
    );
}
