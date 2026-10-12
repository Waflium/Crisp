use crisp_core::BlockPos;
use crisp_data::blocks;
use serde_json::Value;

use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct EndGatewayFeature {
    exit: Option<BlockPos>,
    exact: bool,
}

impl EndGatewayFeature {
    pub fn parse(json: &Value) -> Result<EndGatewayFeature, String> {
        let exit = match json.get("exit").and_then(Value::as_array) {
            Some(v) if v.len() == 3 => {
                let coordinate = |i: usize| v[i].as_i64().map(|c| c as i32).ok_or("bad exit");
                Some(BlockPos::new(coordinate(0)?, coordinate(1)?, coordinate(2)?))
            }
            _ => None,
        };
        Ok(EndGatewayFeature { exit, exact: json.get("exact").and_then(Value::as_bool).ok_or("missing exact")? })
    }

    pub fn known_exit(exit: BlockPos, exact: bool) -> EndGatewayFeature {
        EndGatewayFeature { exit: Some(exit), exact }
    }

    pub fn delayed_exit_search() -> EndGatewayFeature {
        EndGatewayFeature { exit: None, exact: false }
    }

    pub fn place(&self, level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
        for pos in BlockPos::between_closed(origin.offset(-1, -2, -1), origin.offset(1, 2, 1)) {
            let same_x = pos.x == origin.x;
            let same_y = pos.y == origin.y;
            let same_z = pos.z == origin.z;
            let end = (pos.y - origin.y).abs() == 2;
            if same_x && same_y && same_z {
                level.set_block_and_update(pos, blocks::END_GATEWAY.default_state());
                if let Some(target_pos) = self.exit
                    && let Some(block_entity) = level.get_block_entity(pos)
                    && let BlockEntityKind::EndGateway(exit_gateway) = &mut block_entity.kind
                {
                    exit_gateway.set_exit_position(target_pos, self.exact);
                }
            } else if same_y {
                level.set_block_and_update(pos, blocks::AIR.default_state());
            } else if end && same_x && same_z || (same_x || same_z) && !end {
                level.set_block_and_update(pos, blocks::BEDROCK.default_state());
            } else {
                level.set_block_and_update(pos, blocks::AIR.default_state());
            }
        }
        true
    }
}
