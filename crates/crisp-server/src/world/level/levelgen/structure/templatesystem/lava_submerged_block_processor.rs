use crisp_core::shapes;
use crisp_data::blocks;

use super::structure_template::StructureBlockInfo;
use crate::server::level::world_gen_region::WorldGenRegion;

pub fn process_block(level: &WorldGenRegion, processed_block_info: StructureBlockInfo) -> Option<StructureBlockInfo> {
    let pos = processed_block_info.pos;
    let was_lava_before = level.get_block_state(pos).is_block(blocks::LAVA);
    if was_lava_before && !shapes::is_shape_full_block(processed_block_info.state.voxel_shape()) {
        Some(StructureBlockInfo { pos, state: blocks::LAVA.default_state(), nbt: processed_block_info.nbt })
    } else {
        Some(processed_block_info)
    }
}
