use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::packets::play::clientbound;
use crisp_protocol::nbt::Nbt;

use super::ServerLevel;
use crate::network::connection::encode;
use crate::world::container::ContainerRef;
use crate::world::entity::attributes::Attribute;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind, BlockEntityTicker};
use crate::world::level::chunk::block_entities::EntityCreationType;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;

impl ServerLevel {
    pub fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut BlockEntity> {
        if !self.is_in_valid_bounds(pos) {
            return None;
        }
        self.chunks.block_entity(pos, EntityCreationType::Immediate)
    }

    pub fn set_block_entity(&mut self, entity: BlockEntity) {
        if self.is_in_valid_bounds(entity.pos()) {
            self.chunks.add_and_register_block_entity(entity);
        }
    }

    pub fn remove_block_entity(&mut self, pos: BlockPos) -> Option<BlockEntity> {
        if self.is_in_valid_bounds(pos) { self.chunks.remove_block_entity(pos) } else { None }
    }

    pub fn block_entity_changed(&mut self, pos: BlockPos) {
        if self.chunks.loaded_chunk(pos.chunk()).is_some()
            && let Some(chunk) = self.chunks.chunk_if_present(pos.chunk())
        {
            chunk.unsaved = true;
        }
    }

    pub fn block_entity_set_changed(&mut self, pos: BlockPos) {
        let Some(state) = self.chunks.block_entity_ref(pos).map(BlockEntity::block_state) else { return };
        self.block_entity_set_changed_static(pos, state);
    }

    pub fn block_entity_set_changed_static(&mut self, pos: BlockPos, state: BlockState) {
        self.block_entity_changed(pos);
        if !state.is_air() {
            self.update_neighbour_for_output_signal(pos, state.block());
        }
    }

    pub fn pre_remove_side_effects(&mut self, pos: BlockPos) {
        let Some(entity) = self.get_block_entity(pos) else { return };
        let (is_container, is_furnace) = match &entity.kind {
            BlockEntityKind::ShulkerBox(_) | BlockEntityKind::Beehive(_) | BlockEntityKind::Spawner(_) | BlockEntityKind::Other(_) => (false, false),
            BlockEntityKind::Furnace(_) => (true, true),
            _ => (true, false),
        };
        if is_container {
            let container = ContainerRef::block(self, pos);
            crate::world::containers::drop_contents(self, pos, container);
        }
        if is_furnace {
            self.get_recipes_to_award_and_pop_experience(pos, crisp_core::Vec3::at_center_of(pos));
        }
    }

    pub fn unpack_loot_table(&mut self, pos: BlockPos, player: Option<crate::world::entity::EntityId>) {
        let Some(entity) = self.chunks.block_entity(pos, EntityCreationType::Check) else { return };
        if !entity.is_randomizable() {
            return;
        }
        let Some(base) = entity.container_base_mut() else { return };
        let Some(loot_table_key) = base.loot_table.take() else { return };
        let loot_table_seed = base.loot_table_seed;
        let loot_table = LootTable::get(&loot_table_key);
        let container = ContainerRef::block(self, pos);
        let luck = player.map(|player| self.attribute_value(player, Attribute::Luck) as f32);
        let mut params = LootParams::new(self);
        params.origin = Some(crisp_core::Vec3::at_center_of(pos));
        if let Some(luck) = luck {
            params.luck = luck;
            params.this_entity = player;
        }
        loot_table.fill(container, params, loot_table_seed);
    }

    pub fn block_entity_update_packet(&mut self, pos: BlockPos) -> Option<Vec<u8>> {
        let entity = self.get_block_entity(pos)?;
        if !entity.has_update_packet() {
            return None;
        }
        let tag = entity.get_update_tag();
        let ty = entity.ty();
        Some(encode(clientbound::BLOCK_ENTITY_DATA, |p| {
            p.position(pos.x, pos.y, pos.z);
            p.varint(ty.0 as i32);
            tag.write(p);
        }))
    }

    pub fn tick_block_entities(&mut self) {
        let tick_block_entities = self.tick_rate_manager.runs_normally();
        let tickers = &mut self.chunks.block_entity_tickers;
        tickers.ticking = true;
        let pending = std::mem::take(&mut tickers.pending);
        tickers.list.extend(pending);
        let mut list = std::mem::take(&mut tickers.list);
        let mut kept = 0;
        for i in 0..list.len() {
            let (id, pos) = list[i];
            let Some(ticker) = self.chunks.bound_ticker(id, pos) else { continue };
            list[kept] = list[i];
            kept += 1;
            if tick_block_entities && self.in_block_ticking_range(pos.chunk()) {
                self.tick_bound_block_entity(pos, ticker);
            }
        }
        list.truncate(kept);
        let tickers = &mut self.chunks.block_entity_tickers;
        list.append(&mut tickers.list);
        tickers.list = list;
        tickers.ticking = false;
    }

    fn tick_bound_block_entity(&mut self, pos: BlockPos, ticker: BlockEntityTicker) {
        if !self.world_border.is_within_bounds_pos(pos) || !self.chunks.is_ticking(pos.chunk()) || !self.entity_manager.are_entities_loaded(pos.chunk().pack()) {
            return;
        }
        let Some(entity) = self.chunks.block_entity_ref(pos) else { return };
        let ty = entity.ty();
        let state = self.chunks.loaded_chunk(pos.chunk()).map(|c| c.block_state(pos)).unwrap_or_else(|| crisp_data::blocks::AIR.default_state());
        if !ty.is_valid(state) {
            return;
        }
        match ticker {
            BlockEntityTicker::Furnace => self.furnace_server_tick(pos, state),
            BlockEntityTicker::Hopper => self.hopper_push_items_tick(pos, state),
            BlockEntityTicker::ShulkerBox => self.shulker_box_tick(pos, state),
        }
    }

    pub fn block_entity_data(&mut self, pos: BlockPos) -> Option<Nbt> {
        Some(self.get_block_entity(pos)?.save_with_full_metadata())
    }
}
