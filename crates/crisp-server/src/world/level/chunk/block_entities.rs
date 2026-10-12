use std::collections::HashMap;

use crisp_core::fastutil::OpenHashMap;
use crisp_core::hash_map::FastMap;

use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::LevelChunk;
use crate::util::java_collections::JavaObjectHashSet;
use crate::world::level::block::entity::{BlockEntity, BlockEntityTicker};

#[derive(Clone, Default)]
pub struct BlockEntityMap {
    map: OpenHashMap<BlockPos, BlockEntity>,
}

impl BlockEntityMap {
    pub fn get(&self, pos: &BlockPos) -> Option<&BlockEntity> {
        self.map.get(*pos)
    }

    pub fn get_mut(&mut self, pos: &BlockPos) -> Option<&mut BlockEntity> {
        self.map.get_mut(*pos)
    }

    pub fn contains_key(&self, pos: &BlockPos) -> bool {
        self.map.contains_key(*pos)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn insert(&mut self, pos: BlockPos, entity: BlockEntity) -> Option<BlockEntity> {
        self.map.insert(pos, entity)
    }

    pub fn remove(&mut self, pos: &BlockPos) -> Option<BlockEntity> {
        self.map.remove(*pos)
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn keys_in_order(&self) -> Vec<BlockPos> {
        self.map.keys().collect()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut BlockEntity> {
        self.map.values_mut()
    }
}

#[derive(Clone, Default)]
pub struct PendingBlockEntities {
    order: JavaObjectHashSet<BlockPos>,
    tags: FastMap<BlockPos, Nbt>,
}

impl PendingBlockEntities {
    pub fn get(&self, pos: &BlockPos) -> Option<&Nbt> {
        self.tags.get(pos)
    }

    pub fn contains_key(&self, pos: &BlockPos) -> bool {
        self.tags.contains_key(pos)
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn insert(&mut self, pos: BlockPos, tag: Nbt) -> Option<Nbt> {
        self.order.insert(pos);
        self.tags.insert(pos, tag)
    }

    pub fn remove(&mut self, pos: &BlockPos) -> Option<Nbt> {
        self.order.remove(pos);
        self.tags.remove(pos)
    }

    pub fn clear(&mut self) {
        self.order.clear();
        self.tags.clear();
    }

    pub fn keys(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.order.iter()
    }
}

fn put_keep_packed(tag: Nbt, keep_packed: bool) -> Nbt {
    let Nbt::Compound(mut fields) = tag else { return tag };
    let value = Nbt::Byte(keep_packed as i8);
    match fields.iter_mut().find(|(key, _)| key == "keepPacked") {
        Some((_, existing)) => *existing = value,
        None => fields.push(("keepPacked".into(), value)),
    }
    Nbt::Compound(fields)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityCreationType {
    Immediate,
    Check,
}

#[derive(Default)]
pub struct BlockEntityTickers {
    pub list: Vec<(u64, BlockPos)>,
    pub pending: Vec<(u64, BlockPos)>,
    pub ticking: bool,
    next_id: u64,
}

impl BlockEntityTickers {
    fn add(&mut self, pos: BlockPos) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        if self.ticking { self.pending.push((id, pos)) } else { self.list.push((id, pos)) }
        id
    }
}

#[derive(Default)]
pub struct ChunkBlockEntities {
    pub map: BlockEntityMap,
    pub pending: PendingBlockEntities,
    pub tickers_in_level: HashMap<BlockPos, (u64, BlockEntityTicker)>,
}

impl LevelChunk {
    fn is_in_level(&self) -> bool {
        self.loaded
    }

    pub fn get_block_entity(&mut self, pos: BlockPos, creation: EntityCreationType, tickers: &mut BlockEntityTickers) -> Option<&mut BlockEntity> {
        if !self.block_entities.map.contains_key(&pos)
            && let Some(tag) = self.block_entities.pending.remove(&pos)
            && self.promote_pending_block_entity(pos, &tag, tickers)
        {
            return self.block_entities.map.get_mut(&pos);
        }
        match self.block_entities.map.get(&pos) {
            None => {
                if creation == EntityCreationType::Immediate {
                    let state = self.block_state(pos);
                    if state.has_block_entity()
                        && let Some(entity) = BlockEntity::new_for(pos, state)
                    {
                        self.add_and_register_block_entity(entity, tickers);
                    }
                }
                self.block_entities.map.get_mut(&pos)
            }
            Some(entity) if entity.is_removed() => {
                self.block_entities.map.remove(&pos);
                None
            }
            Some(_) => self.block_entities.map.get_mut(&pos),
        }
    }

    pub fn add_and_register_block_entity(&mut self, entity: BlockEntity, tickers: &mut BlockEntityTickers) {
        let pos = entity.pos();
        if self.set_block_entity(entity) && self.is_in_level() {
            self.update_block_entity_ticker(pos, tickers);
        }
    }

    pub fn set_block_entity(&mut self, mut entity: BlockEntity) -> bool {
        let pos = entity.pos();
        let state = self.block_state(pos);
        if !state.has_block_entity() {
            warn!("Trying to set block entity {:?} at position {pos:?}, but state {state:?} does not allow it", entity.ty());
            return false;
        }
        let cached = entity.block_state();
        if state != cached {
            if !entity.ty().is_valid(state) {
                warn!("Trying to set block entity {:?} at position {pos:?}, but state {state:?} does not allow it", entity.ty());
                return false;
            }
            if state.block() != cached.block() {
                warn!("Block state mismatch on block entity {:?} in position {pos:?}, {state:?} != {cached:?}, updating", entity.ty());
            }
            entity.set_block_state(state);
        }
        entity.set_level(self.min_y());
        entity.clear_removed();
        let id = entity.id();
        if let Some(mut previous) = self.block_entities.map.insert(pos, entity)
            && previous.id() != id
        {
            previous.set_removed();
        }
        true
    }

    pub fn remove_block_entity(&mut self, pos: BlockPos) -> Option<BlockEntity> {
        let mut removed = None;
        if self.is_in_level()
            && let Some(mut entity) = self.block_entities.map.remove(&pos)
        {
            entity.set_removed();
            removed = Some(entity);
        }
        self.block_entities.tickers_in_level.remove(&pos);
        removed
    }

    fn promote_pending_block_entity(&mut self, pos: BlockPos, tag: &Nbt, tickers: &mut BlockEntityTickers) -> bool {
        let state = self.block_state(pos);
        let entity = if tag.get("id").and_then(Nbt::as_str) == Some("DUMMY") {
            if state.has_block_entity() {
                BlockEntity::new_for(pos, state)
            } else {
                warn!("Tried to load a DUMMY block entity @ {pos:?} but found not block entity block {state:?} at location");
                None
            }
        } else {
            BlockEntity::load_static(pos, state, tag)
        };
        match entity {
            Some(entity) => {
                self.add_and_register_block_entity(entity, tickers);
                self.block_entities.map.contains_key(&pos)
            }
            None => {
                warn!("Tried to load a block entity for block {state:?} but failed at location {pos:?}");
                false
            }
        }
    }

    pub fn update_block_entity_ticker(&mut self, pos: BlockPos, tickers: &mut BlockEntityTickers) {
        let Some(entity) = self.block_entities.map.get(&pos) else { return };
        match entity.ticker(entity.block_state()) {
            None => {
                self.block_entities.tickers_in_level.remove(&pos);
            }
            Some(ticker) => match self.block_entities.tickers_in_level.get_mut(&pos) {
                Some(existing) => existing.1 = ticker,
                None if self.loaded => {
                    let id = tickers.add(pos);
                    self.block_entities.tickers_in_level.insert(pos, (id, ticker));
                }
                None => {}
            },
        }
    }

    pub fn register_all_block_entities_after_level_load(&mut self, tickers: &mut BlockEntityTickers) {
        for pos in self.block_entities.map.keys_in_order() {
            self.update_block_entity_ticker(pos, tickers);
        }
    }

    pub fn clear_all_block_entities(&mut self) {
        self.block_entities.map.values_mut().for_each(BlockEntity::set_removed);
        self.block_entities.map.clear();
        self.block_entities.tickers_in_level.clear();
    }

    pub fn bound_ticker(&self, id: u64, pos: BlockPos) -> Option<BlockEntityTicker> {
        self.block_entities.tickers_in_level.get(&pos).filter(|(bound, _)| *bound == id).map(|(_, ticker)| *ticker)
    }

    pub fn get_block_entities_pos(&self) -> Vec<BlockPos> {
        let pending = &self.block_entities.pending;
        let mut result = JavaObjectHashSet::with_capacity((pending.len().max(12) as f64 / 0.75).ceil() as usize);
        for pos in pending.keys() {
            result.insert(pos);
        }
        for pos in self.block_entities.map.keys_in_order() {
            result.insert(pos);
        }
        result.iter().collect()
    }

    pub fn block_entities_for_saving(&mut self, tickers: &mut BlockEntityTickers) -> Vec<Nbt> {
        let positions = self.get_block_entities_pos();
        positions.into_iter().filter_map(|pos| self.get_block_entity_nbt_for_saving(pos, tickers)).collect()
    }

    fn get_block_entity_nbt_for_saving(&mut self, pos: BlockPos, tickers: &mut BlockEntityTickers) -> Option<Nbt> {
        if self.proto.is_some() {
            return match self.block_entities.map.get(&pos) {
                Some(entity) => Some(entity.save_with_full_metadata()),
                None => self.block_entities.pending.get(&pos).cloned(),
            };
        }
        if let Some(entity) = self.get_block_entity(pos, EntityCreationType::Check, tickers)
            && !entity.is_removed()
        {
            return Some(put_keep_packed(entity.save_with_full_metadata(), false));
        }
        self.block_entities.pending.get(&pos).map(|tag| put_keep_packed(tag.clone(), true))
    }

    pub fn load_block_entities(&mut self, tags: &[Nbt]) {
        for tag in tags {
            let keep_packed = tag.get("keepPacked").and_then(crate::world::level::block::entity::int_value).is_some_and(|b| b as i8 != 0);
            let pos = BlockEntity::get_pos_from_tag(self.pos, tag);
            if keep_packed {
                self.block_entities.pending.insert(pos, tag.clone());
            } else {
                let state = self.block_state(pos);
                if let Some(entity) = BlockEntity::load_static(pos, state, tag) {
                    self.set_block_entity(entity);
                }
            }
        }
    }
}
