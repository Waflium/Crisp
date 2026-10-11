use crisp_core::random::RandomSource;
use crisp_core::shapes::BitSetDiscreteVoxelShape;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use super::structure_place_settings::StructurePlaceSettings;
use crate::server::level::UPDATE_SHAPE_ORDER;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;

pub struct StructureTemplate {
    palettes: Vec<Palette>,
    size: BlockPos,
}

pub struct Palette {
    blocks: Vec<StructureBlockInfo>,
}

impl Palette {
    pub fn blocks(&self) -> &[StructureBlockInfo] {
        &self.blocks
    }
}

#[derive(Clone)]
pub struct StructureBlockInfo {
    pub pos: BlockPos,
    pub state: BlockState,
    pub nbt: Option<Nbt>,
}

fn add_to_lists(info: StructureBlockInfo, full_block_list: &mut Vec<StructureBlockInfo>, block_entities_list: &mut Vec<StructureBlockInfo>, other_blocks_list: &mut Vec<StructureBlockInfo>) {
    if info.nbt.is_some() {
        block_entities_list.push(info);
    } else if !info.state.block().info().dynamic_shape && info.state.is_collision_shape_full_block() {
        full_block_list.push(info);
    } else {
        other_blocks_list.push(info);
    }
}

fn build_info_list(mut full_block_list: Vec<StructureBlockInfo>, mut block_entities_list: Vec<StructureBlockInfo>, mut other_blocks_list: Vec<StructureBlockInfo>) -> Vec<StructureBlockInfo> {
    let comparator = |o: &StructureBlockInfo| (o.pos.y, o.pos.x, o.pos.z);
    full_block_list.sort_by_key(comparator);
    other_blocks_list.sort_by_key(comparator);
    block_entities_list.sort_by_key(comparator);
    let mut block_info_list = full_block_list;
    block_info_list.append(&mut other_blocks_list);
    block_info_list.append(&mut block_entities_list);
    block_info_list
}

fn int_list(tag: Option<&Nbt>) -> [i32; 3] {
    let list = tag.map(Nbt::as_list).unwrap_or(&[]);
    let int_or = |i: usize| match list.get(i) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    };
    [int_or(0), int_or(1), int_or(2)]
}

impl StructureTemplate {
    pub const EMPTY: StructureTemplate = StructureTemplate { palettes: Vec::new(), size: BlockPos::ZERO };

    pub fn get_size(&self) -> BlockPos {
        self.size
    }

    pub fn get_size_rotated(&self, rotation: Rotation) -> BlockPos {
        match rotation {
            Rotation::Counterclockwise90 | Rotation::Clockwise90 => BlockPos::new(self.size.z, self.size.y, self.size.x),
            _ => self.size,
        }
    }

    pub fn calculate_relative_position(settings: &StructurePlaceSettings, pos: BlockPos) -> BlockPos {
        transform(pos, settings.mirror, settings.rotation, settings.rotation_pivot)
    }

    pub fn place_in_world<R: RandomSource>(
        &self,
        level: &mut WorldGenRegion,
        position: BlockPos,
        reference_pos: BlockPos,
        settings: &StructurePlaceSettings,
        random: &mut R,
        update_mode: i32,
    ) -> bool {
        if self.palettes.is_empty() {
            return false;
        }
        let block_info_list = settings.get_random_palette(&self.palettes, position, random).blocks();
        if block_info_list.is_empty() || self.size.x < 1 || self.size.y < 1 || self.size.z < 1 {
            return false;
        }
        let bounding_box = settings.bounding_box;
        let mut to_fill = Vec::with_capacity(if settings.apply_waterlogging { block_info_list.len() } else { 0 });
        let mut locked_fluids = Vec::with_capacity(if settings.apply_waterlogging { block_info_list.len() } else { 0 });
        let mut placed: Vec<BlockPos> = Vec::with_capacity(block_info_list.len());
        let (mut min_x, mut min_y, mut min_z) = (i32::MAX, i32::MAX, i32::MAX);
        let (mut max_x, mut max_y, mut max_z) = (i32::MIN, i32::MIN, i32::MIN);
        let processed_block_info_list = process_block_infos(level, position, reference_pos, settings, block_info_list, random);
        for block_info in processed_block_info_list {
            let block_pos = block_info.pos;
            if bounding_box.is_some_and(|bounding_box| !bounding_box.is_inside(block_pos)) {
                continue;
            }
            let previous_fluid_state = settings.apply_waterlogging.then(|| level.get_fluid_state(block_pos));
            let state = block::rotate(block::mirror(block_info.state, settings.mirror), settings.rotation);
            if block_info.nbt.is_some() {
                level.set_block(block_pos, blocks::BARRIER.default_state(), 820);
            }
            if !level.set_block(block_pos, state, update_mode) {
                continue;
            }
            min_x = min_x.min(block_pos.x);
            min_y = min_y.min(block_pos.y);
            min_z = min_z.min(block_pos.z);
            max_x = max_x.max(block_pos.x);
            max_y = max_y.max(block_pos.y);
            max_z = max_z.max(block_pos.z);
            placed.push(block_pos);
            if let Some(Nbt::Compound(mut nbt)) = block_info.nbt
                && let Some(block_entity) = level.get_block_entity(block_pos)
            {
                if block_entity.is_randomizable() {
                    let seed = Nbt::Long(random.next_long());
                    match nbt.iter_mut().find(|(key, _)| key == "LootTableSeed") {
                        Some((_, existing)) => *existing = seed,
                        None => nbt.push(("LootTableSeed".into(), seed)),
                    }
                }
                block_entity.load_with_components(&Nbt::Compound(nbt));
            }
            if let Some(previous_fluid_state) = previous_fluid_state {
                if state.fluid_state().is_source() {
                    locked_fluids.push(block_pos);
                } else if block::of(state).is_liquid_block_container(state) {
                    block::of(state).place_liquid(level, block_pos, state, previous_fluid_state);
                    if !previous_fluid_state.is_source() {
                        to_fill.push(block_pos);
                    }
                }
            }
        }
        let mut filled = true;
        let directions = [Direction::Up, Direction::North, Direction::East, Direction::South, Direction::West];
        while filled && !to_fill.is_empty() {
            filled = false;
            to_fill.retain(|&pos| {
                let mut to_place = level.get_fluid_state(pos);
                for direction in directions {
                    if to_place.is_source() {
                        break;
                    }
                    let neighbor_pos = pos.relative(direction);
                    let neighbor = level.get_fluid_state(neighbor_pos);
                    if neighbor.is_source() && !locked_fluids.contains(&neighbor_pos) {
                        to_place = neighbor;
                    }
                }
                if to_place.is_source() {
                    let state = level.get_block_state(pos);
                    if block::of(state).is_liquid_block_container(state) {
                        block::of(state).place_liquid(level, pos, state, to_place);
                        filled = true;
                        return false;
                    }
                }
                true
            });
        }
        if min_x <= max_x && !settings.known_shape {
            let mut shape = BitSetDiscreteVoxelShape::new(max_x - min_x + 1, max_y - min_y + 1, max_z - min_z + 1);
            for block_pos in &placed {
                shape.fill(block_pos.x - min_x, block_pos.y - min_y, block_pos.z - min_z);
            }
            update_shape_at_edge(level, update_mode, &shape, min_x, min_y, min_z);
            for &block_pos in &placed {
                let state = level.get_block_state(block_pos);
                let new_state = update_from_neighbour_shapes(state, level, block_pos);
                if state != new_state {
                    level.set_block(block_pos, new_state, (update_mode & -2) | 16);
                }
            }
        }
        true
    }

    pub fn get_zero_position_with_transform(&self, zero_pos: BlockPos, mirror: Mirror, rotation: Rotation) -> BlockPos {
        get_zero_position_with_transform(zero_pos, mirror, rotation, self.size.x, self.size.z)
    }

    pub fn get_bounding_box(&self, settings: &StructurePlaceSettings, position: BlockPos) -> BoundingBox {
        get_bounding_box(position, settings.rotation, settings.rotation_pivot, settings.mirror, self.size)
    }

    pub fn load(tag: &Nbt) -> StructureTemplate {
        let [x, y, z] = int_list(tag.get("size"));
        let size = BlockPos::new(x, y, z);
        let block_list = tag.get("blocks").map(Nbt::as_list).unwrap_or(&[]);
        let palettes = match tag.get("palettes") {
            Some(Nbt::List(palette_list_list)) => palette_list_list.iter().map(|palette_list| load_palette(palette_list.as_list(), block_list)).collect(),
            _ => vec![load_palette(tag.get("palette").map(Nbt::as_list).unwrap_or(&[]), block_list)],
        };
        StructureTemplate { palettes, size }
    }
}

fn load_palette(palette_list: &[Nbt], block_list: &[Nbt]) -> Palette {
    let palette: Vec<BlockState> = palette_list.iter().map(crate::nbt::read_block_state).collect();
    let mut full_block_list = Vec::new();
    let mut block_entities_list = Vec::new();
    let mut other_blocks_list = Vec::new();
    for block_tag in block_list.iter().filter(|tag| matches!(tag, Nbt::Compound(_))) {
        let [x, y, z] = int_list(block_tag.get("pos"));
        let state_index = match block_tag.get("state") {
            Some(Nbt::Int(index)) => *index,
            _ => 0,
        };
        let state = usize::try_from(state_index).ok().and_then(|index| palette.get(index).copied()).unwrap_or(blocks::AIR.default_state());
        let nbt = block_tag.get("nbt").filter(|nbt| matches!(nbt, Nbt::Compound(_))).cloned();
        add_to_lists(StructureBlockInfo { pos: BlockPos::new(x, y, z), state, nbt }, &mut full_block_list, &mut block_entities_list, &mut other_blocks_list);
    }
    Palette { blocks: build_info_list(full_block_list, block_entities_list, other_blocks_list) }
}

pub fn update_shape_at_edge(level: &mut WorldGenRegion, update_mode: i32, shape: &BitSetDiscreteVoxelShape, start_x: i32, start_y: i32, start_z: i32) {
    shape.for_all_faces(|direction, x, y, z| {
        let pos = BlockPos::new(start_x + x, start_y + y, start_z + z);
        let neighbor_pos = pos.relative(direction);
        let state = level.get_block_state(pos);
        let neighbor_state = level.get_block_state(neighbor_pos);
        let new_state = block::of(state).update_shape(state, level, pos, direction, neighbor_pos, neighbor_state);
        if state != new_state {
            level.set_block(pos, new_state, update_mode & -2);
        }
        let new_neighbor_state = block::of(neighbor_state).update_shape(neighbor_state, level, neighbor_pos, direction.opposite(), pos, new_state);
        if neighbor_state != new_neighbor_state {
            level.set_block(neighbor_pos, new_neighbor_state, update_mode & -2);
        }
    });
}

fn update_from_neighbour_shapes(state: BlockState, level: &mut WorldGenRegion, pos: BlockPos) -> BlockState {
    let mut new_state = state;
    for direction in UPDATE_SHAPE_ORDER {
        let neighbor_pos = pos.relative(direction);
        let neighbor_state = level.get_block_state(neighbor_pos);
        new_state = block::of(new_state).update_shape(new_state, level, pos, direction, neighbor_pos, neighbor_state);
    }
    new_state
}

pub fn process_block_infos<R: RandomSource>(
    level: &mut WorldGenRegion,
    position: BlockPos,
    reference_pos: BlockPos,
    settings: &StructurePlaceSettings,
    block_info_list: &[StructureBlockInfo],
    random: &mut R,
) -> Vec<StructureBlockInfo> {
    let mut processed_block_info_list = Vec::with_capacity(block_info_list.len());
    let chunk_bb = settings.bounding_box;
    for block_info in block_info_list {
        let block_pos = StructureTemplate::calculate_relative_position(settings, block_info.pos);
        let block_pos = block_pos.offset(position.x, position.y, position.z);
        if chunk_bb.is_some_and(|chunk_bb| !chunk_bb.is_inside(block_pos)) {
            continue;
        }
        let mut processed_block_info = Some(StructureBlockInfo { pos: block_pos, state: block_info.state, nbt: block_info.nbt.clone() });
        for processor in settings.processors {
            let Some(info) = processed_block_info else { break };
            processed_block_info = processor.process_block(level, position, reference_pos, block_info.pos, info, settings, random);
        }
        if let Some(processed_block_info) = processed_block_info {
            processed_block_info_list.push(processed_block_info);
        }
    }
    processed_block_info_list
}

pub fn transform(pos: BlockPos, mirror: Mirror, rotation: Rotation, pivot: BlockPos) -> BlockPos {
    let (mut x, y, mut z) = (pos.x, pos.y, pos.z);
    match mirror {
        Mirror::LeftRight => z = -z,
        Mirror::FrontBack => x = -x,
        Mirror::None => {}
    }
    let (pivot_x, pivot_z) = (pivot.x, pivot.z);
    match rotation {
        Rotation::Counterclockwise90 => BlockPos::new(pivot_x - pivot_z + z, y, pivot_x + pivot_z - x),
        Rotation::Clockwise90 => BlockPos::new(pivot_x + pivot_z - z, y, pivot_z - pivot_x + x),
        Rotation::Clockwise180 => BlockPos::new(pivot_x + pivot_x - x, y, pivot_z + pivot_z - z),
        Rotation::None => BlockPos::new(x, y, z),
    }
}

pub fn get_zero_position_with_transform(zero_pos: BlockPos, mirror: Mirror, rotation: Rotation, size_x: i32, size_z: i32) -> BlockPos {
    let size_x = size_x - 1;
    let size_z = size_z - 1;
    let mirror_delta_x = if mirror == Mirror::FrontBack { size_x } else { 0 };
    let mirror_delta_z = if mirror == Mirror::LeftRight { size_z } else { 0 };
    match rotation {
        Rotation::Counterclockwise90 => zero_pos.offset(mirror_delta_z, 0, size_x - mirror_delta_x),
        Rotation::Clockwise90 => zero_pos.offset(size_z - mirror_delta_z, 0, mirror_delta_x),
        Rotation::Clockwise180 => zero_pos.offset(size_x - mirror_delta_x, 0, size_z - mirror_delta_z),
        Rotation::None => zero_pos.offset(mirror_delta_x, 0, mirror_delta_z),
    }
}

pub fn get_bounding_box(position: BlockPos, rotation: Rotation, pivot: BlockPos, mirror: Mirror, size: BlockPos) -> BoundingBox {
    let delta = size.offset(-1, -1, -1);
    let corner1 = transform(BlockPos::ZERO, mirror, rotation, pivot);
    let corner2 = transform(delta, mirror, rotation, pivot);
    BoundingBox::from_corners(corner1, corner2).move_by(position)
}
