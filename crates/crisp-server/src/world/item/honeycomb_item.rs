use crisp_data::{BlockState, WAXABLES};

use crate::world::level::block::weathering_copper::with_properties_of;

pub fn get_waxed(state: BlockState) -> Option<BlockState> {
    WAXABLES.iter().find(|&&(unwaxed, _)| unwaxed == state.block()).map(|&(_, waxed)| with_properties_of(waxed, state))
}
