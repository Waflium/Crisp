use super::MultiNoiseBiomeSource;
use super::the_end_biome_source::TheEndBiomeSource;
use crate::world::level::chunk::Biome;

pub enum BiomeSource {
    MultiNoise(MultiNoiseBiomeSource),
    TheEnd(TheEndBiomeSource),
}

impl BiomeSource {
    pub fn possible_biomes(&self) -> Vec<Biome> {
        match self {
            BiomeSource::MultiNoise(source) => source.possible_biomes(),
            BiomeSource::TheEnd(source) => source.collect_possible_biomes(),
        }
    }
}
