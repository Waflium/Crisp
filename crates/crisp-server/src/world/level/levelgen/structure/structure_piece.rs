use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, ChunkPos, Direction};
use crisp_data::properties::{FACING, HORIZONTAL_FACING};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use super::BoundingBox;
use super::structures::Piece;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::LevelAccessor;

const SHAPE_CHECK_BLOCKS: [crisp_data::Block; 12] = [
    blocks::NETHER_BRICK_FENCE,
    blocks::TORCH,
    blocks::WALL_TORCH,
    blocks::OAK_FENCE,
    blocks::SPRUCE_FENCE,
    blocks::DARK_OAK_FENCE,
    blocks::PALE_OAK_FENCE,
    blocks::ACACIA_FENCE,
    blocks::BIRCH_FENCE,
    blocks::JUNGLE_FENCE,
    blocks::LADDER,
    blocks::IRON_BARS,
];

#[derive(Clone)]
pub struct StructurePiece {
    pub piece_type: &'static str,
    pub gen_depth: i32,
    pub bounding_box: BoundingBox,
    orientation: Option<Direction>,
    mirror: Mirror,
    rotation: Rotation,
}

pub fn make_bounding_box(x: i32, y: i32, z: i32, direction: Direction, width: i32, height: i32, depth: i32) -> BoundingBox {
    if direction.axis() == crisp_core::Axis::Z {
        BoundingBox::new(x, y, z, x + width - 1, y + height - 1, z + depth - 1)
    } else {
        BoundingBox::new(x, y, z, x + depth - 1, y + height - 1, z + width - 1)
    }
}

pub fn get_random_horizontal_direction(random: &mut impl RandomSource) -> Direction {
    Direction::get_random_horizontal_direction(random)
}

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

impl StructurePiece {
    pub fn new(piece_type: &'static str, gen_depth: i32, bounding_box: BoundingBox) -> StructurePiece {
        StructurePiece { piece_type, gen_depth, bounding_box, orientation: None, mirror: Mirror::None, rotation: Rotation::None }
    }

    pub fn load(piece_type: &'static str, tag: &Nbt) -> Option<StructurePiece> {
        let mut piece = StructurePiece::new(piece_type, int(tag, "GD"), BoundingBox::from_nbt(tag.get("BB")?)?);
        let orientation = int(tag, "O");
        piece.set_orientation(if orientation == -1 { None } else { Some(Direction::from_2d_data_value(orientation)) });
        Some(piece)
    }

    pub fn create_tag(&self, additional: Vec<(String, Nbt)>) -> Nbt {
        let mut fields = vec![
            ("id".to_owned(), Nbt::String(self.piece_type.to_owned())),
            ("BB".to_owned(), self.bounding_box.to_nbt()),
            ("O".to_owned(), Nbt::Int(self.orientation.map_or(-1, |o| o.get_2d_data_value()))),
            ("GD".to_owned(), Nbt::Int(self.gen_depth)),
        ];
        fields.extend(additional);
        Nbt::Compound(fields)
    }

    pub fn get_bounding_box(&self) -> BoundingBox {
        self.bounding_box
    }

    pub fn is_close_to_chunk(&self, pos: ChunkPos, distance: i32) -> bool {
        let cx = pos.min_block_x();
        let cz = pos.min_block_z();
        self.bounding_box.intersects_xz(cx - distance, cz - distance, cx + 15 + distance, cz + 15 + distance)
    }

    pub fn get_orientation(&self) -> Option<Direction> {
        self.orientation
    }

    pub fn set_orientation(&mut self, orientation: Option<Direction>) {
        self.orientation = orientation;
        (self.mirror, self.rotation) = match orientation {
            Some(Direction::South) => (Mirror::LeftRight, Rotation::None),
            Some(Direction::West) => (Mirror::LeftRight, Rotation::Clockwise90),
            Some(Direction::East) => (Mirror::None, Rotation::Clockwise90),
            _ => (Mirror::None, Rotation::None),
        };
    }

    pub fn get_rotation(&self) -> Rotation {
        self.rotation
    }

    pub fn get_mirror(&self) -> Mirror {
        self.mirror
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.bounding_box.move_in_place(dx, dy, dz);
    }

    pub fn get_world_pos(&self, x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos::new(self.get_world_x(x, z), self.get_world_y(y), self.get_world_z(x, z))
    }

    pub fn get_world_x(&self, x: i32, z: i32) -> i32 {
        match self.orientation {
            None => x,
            Some(Direction::North | Direction::South) => self.bounding_box.min.x + x,
            Some(Direction::West) => self.bounding_box.max.x - z,
            Some(Direction::East) => self.bounding_box.min.x + z,
            Some(_) => x,
        }
    }

    pub fn get_world_y(&self, y: i32) -> i32 {
        if self.orientation.is_none() { y } else { y + self.bounding_box.min.y }
    }

    pub fn get_world_z(&self, x: i32, z: i32) -> i32 {
        match self.orientation {
            None => z,
            Some(Direction::North) => self.bounding_box.max.z - z,
            Some(Direction::South) => self.bounding_box.min.z + z,
            Some(Direction::West | Direction::East) => self.bounding_box.min.z + x,
            Some(_) => z,
        }
    }

    pub fn place_block(&self, level: &mut impl WorldGenLevel, mut block_state: BlockState, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) {
        let pos = self.get_world_pos(x, y, z);
        if !chunk_bb.is_inside(pos) {
            return;
        }
        if self.mirror != Mirror::None {
            block_state = block::mirror(block_state, self.mirror);
        }
        if self.rotation != Rotation::None {
            block_state = block::rotate(block_state, self.rotation);
        }
        level.set_block(pos, block_state, 2);
        let fluid_state = level.get_fluid_state(pos);
        if !fluid_state.is_empty() {
            level.schedule_fluid_tick(pos, fluid_state.fluid, 0);
        }
        if SHAPE_CHECK_BLOCKS.contains(&block_state.block()) {
            LevelAccessor::mark_pos_for_post_processing(level, pos);
        }
    }

    pub fn get_block(&self, level: &mut impl WorldGenLevel, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) -> BlockState {
        let pos = self.get_world_pos(x, y, z);
        if !chunk_bb.is_inside(pos) { blocks::AIR.default_state() } else { level.get_block_state(pos) }
    }

    pub fn is_interior(&self, level: &mut impl WorldGenLevel, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) -> bool {
        let pos = self.get_world_pos(x, y + 1, z);
        chunk_bb.is_inside(pos) && pos.y < level.get_height(HeightmapType::OceanFloorWg, pos.x, pos.z)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_air_box(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    self.place_block(level, blocks::AIR.default_state(), x, y, z, chunk_bb);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_box(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge_block: BlockState,
        fill_block: BlockState,
        skip_air: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if !skip_air || !self.get_block(level, x, y, z, chunk_bb).is_air() {
                        if y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1 {
                            self.place_block(level, fill_block, x, y, z, chunk_bb);
                        } else {
                            self.place_block(level, edge_block, x, y, z, chunk_bb);
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_box_with_selector(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        skip_air: bool,
        random: &mut impl RandomSource,
        selector: &mut impl BlockSelector,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if !skip_air || !self.get_block(level, x, y, z, chunk_bb).is_air() {
                        selector.next(random, x, y, z, y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1);
                        self.place_block(level, selector.get_next(), x, y, z, chunk_bb);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_maybe_box(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        random: &mut impl RandomSource,
        probability: f32,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge_block: BlockState,
        fill_block: BlockState,
        skip_air: bool,
        has_to_be_inside: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if random.next_float() <= probability
                        && (!skip_air || !self.get_block(level, x, y, z, chunk_bb).is_air())
                        && (!has_to_be_inside || self.is_interior(level, x, y, z, chunk_bb))
                    {
                        if y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1 {
                            self.place_block(level, fill_block, x, y, z, chunk_bb);
                        } else {
                            self.place_block(level, edge_block, x, y, z, chunk_bb);
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn maybe_generate_block(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, random: &mut impl RandomSource, probability: f32, x: i32, y: i32, z: i32, block_state: BlockState) {
        if random.next_float() < probability {
            self.place_block(level, block_state, x, y, z, chunk_bb);
        }
    }

    pub fn fill_column_down(&self, level: &mut impl WorldGenLevel, block_state: BlockState, x: i32, start_y: i32, z: i32, chunk_bb: &BoundingBox) {
        let mut pos = self.get_world_pos(x, start_y, z);
        if chunk_bb.is_inside(pos) {
            while is_replaceable_by_structures(level.get_block_state(pos)) && pos.y > level.get_min_y() + 1 {
                level.set_block(pos, block_state, 2);
                pos = pos.below();
            }
        }
    }

    pub fn create_chest(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, random: &mut impl RandomSource, pos: BlockPos, loot_table: &str, block_state: Option<BlockState>) -> bool {
        if !chunk_bb.is_inside(pos) || level.get_block_state(pos).is_block(blocks::CHEST) {
            return false;
        }
        let block_state = block_state.unwrap_or_else(|| reorient(level, pos, blocks::CHEST.default_state()));
        level.set_block(pos, block_state, 2);
        if let Some(entity) = level.get_block_entity(pos)
            && matches!(entity.kind, BlockEntityKind::Chest(_))
            && let Some(base) = entity.container_base_mut()
        {
            base.loot_table = Some(loot_table.to_owned());
            base.loot_table_seed = random.next_long();
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_dispenser(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        random: &mut impl RandomSource,
        x: i32,
        y: i32,
        z: i32,
        facing: Direction,
        loot_table: &str,
    ) -> bool {
        let pos = self.get_world_pos(x, y, z);
        if !chunk_bb.is_inside(pos) || level.get_block_state(pos).is_block(blocks::DISPENSER) {
            return false;
        }
        self.place_block(level, blocks::DISPENSER.default_state().set(FACING, facing), x, y, z, chunk_bb);
        if let Some(entity) = level.get_block_entity(pos)
            && let BlockEntityKind::Dispenser(dispenser) = &mut entity.kind
        {
            dispenser.base.loot_table = Some(loot_table.to_owned());
            dispenser.base.loot_table_seed = random.next_long();
        }
        true
    }
}

pub trait BlockSelector {
    fn next(&mut self, random: &mut impl RandomSource, world_x: i32, world_y: i32, world_z: i32, is_edge: bool);

    fn get_next(&self) -> BlockState;
}

pub fn find_collision_piece<'a>(pieces: &'a [Piece], bounding_box: &BoundingBox) -> Option<&'a Piece> {
    pieces.iter().find(|piece| piece.piece().bounding_box.intersects(bounding_box))
}

pub fn is_replaceable_by_structures(state: BlockState) -> bool {
    state.is_air() || state.liquid() || state.is_block(blocks::GLOW_LICHEN) || state.is_block(blocks::SEAGRASS) || state.is_block(blocks::TALL_SEAGRASS)
}

pub fn reorient(level: &mut impl WorldGenLevel, block_pos: BlockPos, block_state: BlockState) -> BlockState {
    let mut solid_neighbor = None;
    for direction in Direction::PLANE_HORIZONTAL {
        let state = level.get_block_state(block_pos.relative(direction));
        if state.is_block(blocks::CHEST) {
            return block_state;
        }
        if state.is_solid_render() {
            if solid_neighbor.is_some() {
                solid_neighbor = None;
                break;
            }
            solid_neighbor = Some(direction);
        }
    }
    if let Some(solid_neighbor) = solid_neighbor {
        return block_state.set(HORIZONTAL_FACING, solid_neighbor.opposite());
    }
    let mut lock_dir = block_state.get(HORIZONTAL_FACING);
    let mut relative_pos = block_pos.relative(lock_dir);
    if level.get_block_state(relative_pos).is_solid_render() {
        lock_dir = lock_dir.opposite();
        relative_pos = block_pos.relative(lock_dir);
    }
    if level.get_block_state(relative_pos).is_solid_render() {
        lock_dir = lock_dir.clockwise();
        relative_pos = block_pos.relative(lock_dir);
    }
    if level.get_block_state(relative_pos).is_solid_render() {
        lock_dir = lock_dir.opposite();
    }
    block_state.set(HORIZONTAL_FACING, lock_dir)
}
