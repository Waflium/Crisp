use crisp_core::BlockPos;
use crisp_data::{BlockState, Fluid};

use super::BlockBehaviour;
use super::pointed_dripstone_block;
use crate::server::level::ServerLevel;

pub fn tick(this: &(impl BlockBehaviour + ?Sized), cauldron_state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if let Some(stalactite_pos) = pointed_dripstone_block::find_stalactite_tip_above_cauldron(level, pos) {
        let fluid = pointed_dripstone_block::get_cauldron_fill_fluid_type(level, stalactite_pos);
        if fluid != Fluid::Empty && this.can_receive_stalactite_drip(fluid) {
            this.receive_stalactite_drip(cauldron_state, level, pos, fluid);
        }
    }
}
