use std::collections::BTreeSet;

use crisp_core::ChunkPos;

use super::structure::StructureHolder;
use super::structures::StructureType;
use crate::world::level::chunk::{ChunkGenerator, ChunkStructures};
use crate::world::level::chunk::chunk_generator_structure_state::ChunkGeneratorStructureState;
use crate::world::level::levelgen::tests::{oracle_dimension_type, oracle_generator};

#[test]
#[ignore]
fn structure_starts_match_vanilla_dump() {
    let dump = std::fs::read_to_string(std::env::var("CRISP_STARTS_DUMP").expect("CRISP_STARTS_DUMP")).unwrap();
    let range: Vec<i32> = std::env::var("CRISP_STARTS_RANGE").expect("CRISP_STARTS_RANGE").split(',').map(|s| s.parse().unwrap()).collect();
    let seed = 12345;
    let implemented = |id: &str| StructureHolder::by_name(id).is_some_and(|holder| !matches!(holder.value().structure_type, StructureType::NotImplemented));
    let expected: BTreeSet<String> = dump
        .lines()
        .filter_map(|line| line.strip_prefix("START "))
        .filter(|line| line.split(' ').nth(2).is_some_and(implemented))
        .map(str::to_owned)
        .collect();
    let generator = ChunkGenerator::Noise(std::sync::Arc::new(oracle_generator(seed)));
    let state = ChunkGeneratorStructureState::create_for_normal(&generator, seed);
    let mut actual = BTreeSet::new();
    for x in range[0]..=range[2] {
        for z in range[1]..=range[3] {
            let pos = ChunkPos::new(x, z);
            let mut structures = ChunkStructures::default();
            generator.create_structures(&state, pos, oracle_dimension_type(), &mut structures);
            for (holder, start) in &structures.starts {
                actual.insert(format!("{x} {z} {} {}", holder.key(), crate::nbt::to_string(&start.create_tag(pos))));
            }
        }
    }
    if let Ok(out) = std::env::var("CRISP_STARTS_OUT") {
        std::fs::write(out, actual.iter().map(|line| format!("{line}\n")).collect::<String>()).unwrap();
    }
    let missing: Vec<_> = expected.difference(&actual).collect();
    let extra: Vec<_> = actual.difference(&expected).collect();
    eprintln!("{} expected, {} actual, {} missing, {} extra", expected.len(), actual.len(), missing.len(), extra.len());
    for line in missing.iter().take(5) {
        eprintln!("MISSING {}", &line[..line.len().min(600)]);
    }
    for line in extra.iter().take(5) {
        eprintln!("EXTRA {}", &line[..line.len().min(600)]);
    }
    assert!(missing.is_empty() && extra.is_empty());
}
