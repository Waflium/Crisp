use crisp_core::{BlockPos, ChunkPos, SectionPos};
use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::server::level::chunk_level::ChunkStatus;
use crate::util::codec::{self, DataResult};
use crate::world::item::{ItemStack, map_item};
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::saveddata::maps::map_decoration_type::MapDecorationTypeHolder;
use crate::world::level::saveddata::maps::map_decoration_types::WOODLAND_MANSION;
use crate::world::level::saveddata::maps::map_item_saved_data::MapItemSavedData;
use crate::world::level::storage::loot::loot_context::LootContext;

pub const DEFAULT_DECORATION: MapDecorationTypeHolder = WOODLAND_MANSION;
pub const DEFAULT_ZOOM: i8 = 2;
pub const DEFAULT_SEARCH_RADIUS: i32 = 50;
pub const DEFAULT_SKIP_EXISTING: bool = true;

pub struct ExplorationMapFunction {
    destination: HolderSet,
    map_decoration: MapDecorationTypeHolder,
    zoom: i8,
    search_radius: i32,
    skip_known_structures: bool,
}

impl ExplorationMapFunction {
    pub fn decode(tag: &Nbt) -> DataResult<ExplorationMapFunction> {
        Ok(ExplorationMapFunction {
            destination: codec::decode_field(tag, "destination", |t| HolderSet::decode("minecraft:worldgen/structure", t))?,
            map_decoration: codec::decode_or(tag, "decoration", DEFAULT_DECORATION, |t| {
                let key = codec::identifier(t)?;
                MapDecorationTypeHolder::by_name(&key).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:map_decoration_type]: {key}"))
            })?,
            zoom: codec::decode_or(tag, "zoom", DEFAULT_ZOOM, |t| Ok(codec::int(t)? as i8))?,
            search_radius: codec::decode_or(tag, "search_radius", DEFAULT_SEARCH_RADIUS, codec::int)?,
            skip_known_structures: codec::decode_or(tag, "skip_existing_chunks", DEFAULT_SKIP_EXISTING, codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        let Some(loot_pos) = context.params.origin else { return item_stack };
        let level = context.level();
        let loot_block_pos = BlockPos::containing_vec(loot_pos);
        let destination: Vec<StructureHolder> = self.destination.contents().iter().map(|&id| StructureHolder(id as u16)).collect();
        if self.skip_known_structures {
            claim_structure_at(level, &destination, loot_block_pos);
        }
        let Some(nearest_map_structure) = level.find_nearest_map_structure(&destination, loot_block_pos, self.search_radius, self.skip_known_structures) else {
            return item_stack;
        };
        map_item::apply_new_saved_data(level, &mut item_stack, nearest_map_structure.x, nearest_map_structure.z, self.zoom, true, true);
        map_item::render_biome_preview_map(level, &item_stack);
        MapItemSavedData::add_target_decoration(&mut item_stack, nearest_map_structure, "+", self.map_decoration);
        item_stack
    }
}

fn claim_structure_at(level: &mut ServerLevel, destination: &[StructureHolder], pos: BlockPos) {
    let chunk_pos = ChunkPos::new(SectionPos::block_to_section_coord(pos.x), SectionPos::block_to_section_coord(pos.z));
    level.chunks.get_chunk_structures(chunk_pos, ChunkStatus::StructureReferences);
    let starts = level.chunks.starts_for_structure_matching(chunk_pos, |structure| destination.contains(&structure));
    if let Some(start) = starts.into_iter().find(|start| start.get_bounding_box().is_inside(pos))
        && start.is_valid()
        && start.can_be_referenced()
    {
        start.add_reference();
        level.structure_check.increment_reference(start.chunk_pos, start.structure);
    }
}
