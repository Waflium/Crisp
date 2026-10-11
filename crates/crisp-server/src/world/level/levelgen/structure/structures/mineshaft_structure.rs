use crisp_core::BlockPos;
use crisp_core::mth::random_between_inclusive;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use super::Piece;
use super::mineshaft_pieces::MineShaftRoom;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};

pub struct MineshaftStructure {
    r#type: Type,
}

impl MineshaftStructure {
    pub fn parse(json: &Value) -> Result<MineshaftStructure, String> {
        let name = json.get("mineshaft_type").and_then(Value::as_str).ok_or("mineshaft without mineshaft_type")?;
        Ok(MineshaftStructure { r#type: Type::by_name(name).ok_or_else(|| format!("unknown mineshaft type {name}"))? })
    }

    pub fn find_generation_point(&self, context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
        context.random.next_double();
        let chunk_pos = context.chunk_pos;
        let start_pos = BlockPos::new(chunk_pos.min_block_x() + 8, 50, chunk_pos.min_block_z());
        let mut mineshaft_pieces_builder = StructurePiecesBuilder::default();
        let y_offset = self.generate_pieces_and_adjust(&mut mineshaft_pieces_builder, context);
        Some(GenerationStub { position: start_pos.offset(0, y_offset, 0), generator: Box::new(move |builder, _| *builder = mineshaft_pieces_builder) })
    }

    fn generate_pieces_and_adjust(&self, builder: &mut StructurePiecesBuilder, context: &mut GenerationContext) -> i32 {
        let chunk_pos = context.chunk_pos;
        let random = &mut context.random;
        let chunk_generator = context.chunk_generator;
        let mut mine_shaft_room = MineShaftRoom::new(0, random, chunk_pos.min_block_x() + 2, chunk_pos.min_block_z() + 2, self.r#type);
        builder.add_piece(Piece::MineShaftRoom(mine_shaft_room.clone()));
        mine_shaft_room.add_children(builder, random);
        builder.pieces[0] = Piece::MineShaftRoom(mine_shaft_room);
        let sea_level = chunk_generator.sea_level();
        if self.r#type == Type::Mesa {
            let center = builder.get_bounding_box().get_center();
            let surface_height = chunk_generator.get_base_height(center.x, center.z, HeightmapType::WorldSurfaceWg, context.min_y, context.height);
            let target_y_for_center = if surface_height <= sea_level { sea_level } else { random_between_inclusive(random, sea_level, surface_height) };
            let dy = target_y_for_center - center.y;
            builder.offset_pieces_vertically(dy);
            dy
        } else {
            builder.move_below_sea_level(sea_level, chunk_generator.min_y(), random, 10)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Normal,
    Mesa,
}

impl Type {
    fn by_name(name: &str) -> Option<Type> {
        match name {
            "normal" => Some(Type::Normal),
            "mesa" => Some(Type::Mesa),
            _ => None,
        }
    }

    pub fn by_id(id: i32) -> Type {
        if id == 1 { Type::Mesa } else { Type::Normal }
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    pub fn get_wood_state(self) -> BlockState {
        match self {
            Type::Normal => blocks::OAK_LOG.default_state(),
            Type::Mesa => blocks::DARK_OAK_LOG.default_state(),
        }
    }

    pub fn get_planks_state(self) -> BlockState {
        match self {
            Type::Normal => blocks::OAK_PLANKS.default_state(),
            Type::Mesa => blocks::DARK_OAK_PLANKS.default_state(),
        }
    }

    pub fn get_fence_state(self) -> BlockState {
        match self {
            Type::Normal => blocks::OAK_FENCE.default_state(),
            Type::Mesa => blocks::DARK_OAK_FENCE.default_state(),
        }
    }
}
