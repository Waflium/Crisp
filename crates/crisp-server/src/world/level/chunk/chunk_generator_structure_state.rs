use std::sync::OnceLock;

use crisp_core::ChunkPos;
use crisp_core::random::{LegacyRandomSource, RandomSource};

use crate::world::level::chunk::Biome;
use super::{ChunkGenerator, FlatLevelSource};
use crate::world::level::levelgen::flat::StructureOverrides;
use crate::world::level::levelgen::structure::placement::concentric_rings_structure_placement::ConcentricRingsStructurePlacement;
use crate::world::level::levelgen::structure::placement::structure_placement::StructurePlacement;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_set::{StructureSet, StructureSetHolder};

pub struct ChunkGeneratorStructureState {
    level_seed: i64,
    concentric_rings_seed: i64,
    origin: OnceLock<ChunkPos>,
    possible_structure_sets: Vec<StructureSetHolder>,
    placements_for_structure: OnceLock<Vec<Vec<StructureSetHolder>>>,
    ring_positions: Vec<OnceLock<Vec<ChunkPos>>>,
}

fn has_biomes_for_structure_set(structure_set: &StructureSet, possible_biomes: &[Biome]) -> bool {
    structure_set.structures.iter().any(|entry| entry.structure.value().biomes().contents().iter().any(|&id| possible_biomes.contains(&Biome(id as u16))))
}

impl ChunkGeneratorStructureState {
    fn new(level_seed: i64, concentric_rings_seed: i64, possible_structure_sets: Vec<StructureSetHolder>) -> ChunkGeneratorStructureState {
        ChunkGeneratorStructureState {
            level_seed,
            concentric_rings_seed,
            origin: OnceLock::new(),
            possible_structure_sets,
            placements_for_structure: OnceLock::new(),
            ring_positions: StructureSetHolder::all().map(|_| OnceLock::new()).collect(),
        }
    }

    pub fn create_for_flat(flat: &FlatLevelSource, level_seed: i64) -> ChunkGeneratorStructureState {
        let possible_biomes = [flat.biome];
        let structure_overrides: Vec<StructureSetHolder> = match &flat.structure_overrides {
            Some(StructureOverrides::Direct(ids)) => ids.iter().filter_map(|id| StructureSetHolder::by_name(id)).collect(),
            Some(StructureOverrides::Tag(_)) => Vec::new(),
            None => StructureSetHolder::all().collect(),
        };
        let structures = structure_overrides.into_iter().filter(|set| has_biomes_for_structure_set(set.value(), &possible_biomes)).collect();
        ChunkGeneratorStructureState::new(level_seed, 0, structures)
    }

    pub fn create_for_normal(generator: &ChunkGenerator, level_seed: i64) -> ChunkGeneratorStructureState {
        let possible_biomes = generator.possible_biomes();
        let structures = StructureSetHolder::all().filter(|set| has_biomes_for_structure_set(set.value(), &possible_biomes)).collect();
        ChunkGeneratorStructureState::new(level_seed, level_seed, structures)
    }

    pub fn possible_structure_sets(&self) -> &[StructureSetHolder] {
        &self.possible_structure_sets
    }

    pub fn get_level_seed(&self) -> i64 {
        self.level_seed
    }

    pub fn get_dimension_origin(&self) -> ChunkPos {
        *self.origin.get().expect("the dimension origin")
    }

    pub fn ensure_structures_generated(&self, generator: &ChunkGenerator) {
        self.origin.get_or_init(|| generator.origin());
        self.placements_for_structure.get_or_init(|| self.generate_positions(generator));
    }

    fn generate_positions(&self, generator: &ChunkGenerator) -> Vec<Vec<StructureSetHolder>> {
        let possible_biomes = generator.possible_biomes();
        let mut placements_for_structure = vec![Vec::new(); crisp_data::STRUCTURES.len()];
        for &set in &self.possible_structure_sets {
            let structure_set = set.value();
            let mut has_any_placeable_structures = false;
            for entry in &structure_set.structures {
                if entry.structure.value().biomes().contents().iter().any(|&id| possible_biomes.contains(&Biome(id as u16))) {
                    placements_for_structure[entry.structure.0 as usize].push(set);
                    has_any_placeable_structures = true;
                }
            }
            if has_any_placeable_structures && let StructurePlacement::ConcentricRings(placement) = &structure_set.placement {
                self.ring_positions[set.0 as usize].get_or_init(|| self.generate_ring_positions(generator, placement));
            }
        }
        placements_for_structure
    }

    fn generate_ring_positions(&self, generator: &ChunkGenerator, placement: &ConcentricRingsStructurePlacement) -> Vec<ChunkPos> {
        if placement.count == 0 {
            return Vec::new();
        }
        let distance = placement.distance;
        let count = placement.count;
        let mut spread = placement.spread;
        let mut tasks: Vec<(i32, i32, LegacyRandomSource)> = Vec::with_capacity(count as usize);
        let mut random = LegacyRandomSource::new(0);
        random.set_seed(self.concentric_rings_seed);
        let mut angle = random.next_double() * std::f64::consts::PI * 2.0;
        let mut position_in_circle = 0;
        let mut circle = 0;
        for i in 0..count {
            let dist = (4 * distance + distance * circle * 6) as f64 + (random.next_double() - 0.5) * (distance as f64 * 2.5);
            let initial_x = (angle.cos() * dist).round() as i32;
            let initial_z = (angle.sin() * dist).round() as i32;
            let biome_search_generator = random.fork();
            tasks.push((initial_x, initial_z, biome_search_generator));
            angle += std::f64::consts::PI * 2.0 / spread as f64;
            position_in_circle += 1;
            if position_in_circle == spread {
                circle += 1;
                position_in_circle = 0;
                spread += 2 * spread / (circle + 1);
                spread = spread.min(count - i);
                angle += random.next_double() * std::f64::consts::PI * 2.0;
            }
        }
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let chunk_size = tasks.len().div_ceil(threads);
        std::thread::scope(|scope| {
            let handles: Vec<_> = tasks
                .chunks_mut(chunk_size)
                .map(|batch| {
                    scope.spawn(move || {
                        batch
                            .iter_mut()
                            .map(|(initial_x, initial_z, biome_search_generator)| {
                                match generator.find_biome_horizontal((*initial_x << 4) + 8, 0, (*initial_z << 4) + 8, 112, &placement.preferred_biomes, biome_search_generator) {
                                    Some((x, _, z)) => ChunkPos::new(x >> 4, z >> 4),
                                    None => ChunkPos::new(*initial_x, *initial_z),
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|handle| handle.join().expect("ring position search")).collect()
        })
    }

    pub fn get_ring_positions_for(&self, set: StructureSetHolder) -> Option<&Vec<ChunkPos>> {
        self.ring_positions[set.0 as usize].get()
    }

    pub fn get_placements_for_structure(&self, structure: StructureHolder, generator: &ChunkGenerator) -> &[StructureSetHolder] {
        self.ensure_structures_generated(generator);
        &self.placements_for_structure.get().expect("structure placements")[structure.0 as usize]
    }

    pub fn has_structure_chunk_in_range(&self, structure_set: StructureSetHolder, source_x: i32, source_z: i32, range: i32) -> bool {
        let placement = &structure_set.value().placement;
        for test_x in source_x - range..=source_x + range {
            for test_z in source_z - range..=source_z + range {
                if placement.is_structure_chunk(structure_set, self, test_x, test_z) {
                    return true;
                }
            }
        }
        false
    }
}
