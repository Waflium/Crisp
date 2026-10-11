use std::sync::OnceLock;

use crisp_core::ChunkPos;
use crisp_core::random::{LegacyRandomSource, RandomSource};

use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::NoiseBasedChunkGenerator;
use crate::world::level::levelgen::density_function::SamplerContext;
use crate::world::level::levelgen::quart_pos;
use crate::world::level::levelgen::structure::placement::concentric_rings_structure_placement::ConcentricRingsStructurePlacement;
use crate::world::level::levelgen::structure::placement::structure_placement::StructurePlacement;
use crate::world::level::levelgen::structure::structure_set::{StructureSet, StructureSetHolder};

pub struct ChunkGeneratorStructureState {
    level_seed: i64,
    concentric_rings_seed: i64,
    origin: OnceLock<ChunkPos>,
    possible_structure_sets: Vec<StructureSetHolder>,
    ring_positions: Vec<OnceLock<Vec<ChunkPos>>>,
}

fn has_biomes_for_structure_set(structure_set: &StructureSet, possible_biomes: &[Biome]) -> bool {
    structure_set.structures.iter().any(|entry| entry.structure.value().biomes().contents().iter().any(|&id| possible_biomes.contains(&Biome(id as u16))))
}

impl ChunkGeneratorStructureState {
    pub fn create_for_normal(generator: &NoiseBasedChunkGenerator, level_seed: i64) -> ChunkGeneratorStructureState {
        let possible_biomes = generator.biome_source.possible_biomes();
        let possible_structure_sets = StructureSetHolder::all().filter(|set| has_biomes_for_structure_set(set.value(), &possible_biomes)).collect();
        ChunkGeneratorStructureState {
            level_seed,
            concentric_rings_seed: level_seed,
            origin: OnceLock::new(),
            possible_structure_sets,
            ring_positions: StructureSetHolder::all().map(|_| OnceLock::new()).collect(),
        }
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

    pub fn ensure_structures_generated(&self, generator: &NoiseBasedChunkGenerator) {
        self.origin.get_or_init(|| generator.origin());
        let possible_biomes = generator.biome_source.possible_biomes();
        for &set in &self.possible_structure_sets {
            let structure_set = set.value();
            let has_any_placeable_structures = structure_set
                .structures
                .iter()
                .any(|entry| entry.structure.value().biomes().contents().iter().any(|&id| possible_biomes.contains(&Biome(id as u16))));
            if has_any_placeable_structures && let StructurePlacement::ConcentricRings(placement) = &structure_set.placement {
                self.ring_positions[set.0 as usize].get_or_init(|| self.generate_ring_positions(generator, placement));
            }
        }
    }

    fn generate_ring_positions(&self, generator: &NoiseBasedChunkGenerator, placement: &ConcentricRingsStructurePlacement) -> Vec<ChunkPos> {
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
                                match find_biome_horizontal(generator, (*initial_x << 4) + 8, 0, (*initial_z << 4) + 8, 112, &placement.preferred_biomes, biome_search_generator) {
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

fn find_biome_horizontal(
    generator: &NoiseBasedChunkGenerator,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    search_radius: i32,
    allowed: &crate::core::holder_set::HolderSet,
    random: &mut impl RandomSource,
) -> Option<(i32, i32, i32)> {
    let noise_center_x = quart_pos::from_block(origin_x);
    let noise_center_z = quart_pos::from_block(origin_z);
    let noise_radius = quart_pos::from_block(search_radius);
    let noise_y = quart_pos::from_block(origin_y);
    let mut ctx = SamplerContext::cached();
    let mut result = None;
    let mut found = 0;
    for z in -noise_radius..=noise_radius {
        for x in -noise_radius..=noise_radius {
            let noise_x = noise_center_x + x;
            let noise_z = noise_center_z + z;
            let biome = generator.get_noise_biome(&mut ctx, noise_x, noise_y, noise_z);
            if allowed.contains(biome.0 as i32) {
                if result.is_none() || random.next_int_bounded(found + 1) == 0 {
                    result = Some((quart_pos::to_block(noise_x), origin_y, quart_pos::to_block(noise_z)));
                }
                found += 1;
            }
        }
    }
    result
}
