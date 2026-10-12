use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use serde_json::Value;

use super::nether_fossil_pieces;
use crate::world::level::levelgen::heightproviders::HeightProvider;
use crate::world::level::levelgen::material::WorldGenerationContext;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};

pub struct NetherFossilStructure {
    pub height: HeightProvider,
}

impl NetherFossilStructure {
    pub fn parse(json: &Value) -> Result<NetherFossilStructure, String> {
        Ok(NetherFossilStructure { height: HeightProvider::parse(json.get("height").ok_or("missing height")?)? })
    }

    pub fn find_generation_point(&self, context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
        let random = &mut context.random;
        let block_x = context.chunk_pos.min_block_x() + random.next_int_bounded(16);
        let block_z = context.chunk_pos.min_block_z() + random.next_int_bounded(16);
        let sea_level = context.chunk_generator.sea_level();
        let generation_context = WorldGenerationContext {
            min_gen_y: context.min_y.max(context.chunk_generator.min_y()),
            gen_depth: context.height.min(context.chunk_generator.gen_depth()),
            sea_level,
        };
        let mut y = self.height.sample(random, &generation_context);
        let column = context.chunk_generator.get_base_column(block_x, block_z, context.min_y, context.height);
        while y > sea_level {
            let current = column.get_block(y);
            y -= 1;
            let below = column.get_block(y);
            if current.is_air() && (below.is_block(blocks::SOUL_SAND) || below.is_face_sturdy_full(Direction::Up)) {
                break;
            }
        }
        if y <= sea_level {
            return None;
        }
        let position = BlockPos::new(block_x, y, block_z);
        Some(GenerationStub {
            position,
            generator: Box::new(move |builder, context| nether_fossil_pieces::add_pieces(builder, &mut context.random, position)),
        })
    }
}
