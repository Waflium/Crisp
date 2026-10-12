use crisp_core::BlockPos;
use crisp_data::{BlockState, game_rule};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment_helper;

pub fn spawn_infestation(level: &mut ServerLevel, pos: BlockPos) {
    if let Some(mut silverfish) = ServerLevel::create_entity("minecraft:silverfish") {
        silverfish.entity_mut().snap_to(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5, 0.0, 0.0);
        let id = silverfish.entity().id;
        level.add_fresh_entity(silverfish);
        level.broadcast_entity_event(id, 20);
    }
}

pub fn spawn_after_break(level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>) {
    if level.game_rules.get_bool(game_rule::BLOCK_DROPS) && !tool.is_some_and(|tool| enchantment_helper::has_tag(&tool, "minecraft:prevents_infested_spawns")) {
        spawn_infestation(level, pos);
    }
}

pub struct InfestedBlock;

impl BlockBehaviour for InfestedBlock {
    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, _drop_experience: bool) {
        spawn_after_break(level, pos, tool);
    }
}
