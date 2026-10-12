use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::quart_pos;

pub struct TheEndBiomeSource {
    end: Biome,
    highlands: Biome,
    midlands: Biome,
    islands: Biome,
    barrens: Biome,
}

impl TheEndBiomeSource {
    pub fn create() -> TheEndBiomeSource {
        let biome = |name: &str| Biome::by_name(name).expect("an end biome");
        TheEndBiomeSource {
            end: biome("minecraft:the_end"),
            highlands: biome("minecraft:end_highlands"),
            midlands: biome("minecraft:end_midlands"),
            islands: biome("minecraft:small_end_islands"),
            barrens: biome("minecraft:end_barrens"),
        }
    }

    pub fn collect_possible_biomes(&self) -> Vec<Biome> {
        vec![self.end, self.highlands, self.midlands, self.islands, self.barrens]
    }

    pub fn get_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32, erosion: impl FnOnce(i32, i32, i32) -> f64) -> Biome {
        let block_x = quart_pos::to_block(quart_x);
        let block_y = quart_pos::to_block(quart_y);
        let block_z = quart_pos::to_block(quart_z);
        let chunk_x = block_x >> 4;
        let chunk_z = block_z >> 4;
        if chunk_x as i64 * chunk_x as i64 + chunk_z as i64 * chunk_z as i64 <= 4096 {
            return self.end;
        }
        let weird_block_x = ((block_x >> 4) * 2 + 1) * 8;
        let weird_block_z = ((block_z >> 4) * 2 + 1) * 8;
        let height_value = erosion(weird_block_x, block_y, weird_block_z);
        if height_value > 0.25 {
            self.highlands
        } else if height_value >= -0.0625 {
            self.midlands
        } else if height_value < -0.21875 {
            self.islands
        } else {
            self.barrens
        }
    }
}
