use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::abstract_furnace_block_entity::AbstractFurnaceBlockEntity;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind};
use crate::world::level::block::chest_block;

pub const DEFAULT_DISTANCE_BUFFER: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContainerRef {
    Block { pos: BlockPos, id: u64 },
    Compound { first: (BlockPos, u64), second: (BlockPos, u64) },
}

impl ContainerRef {
    pub fn block(level: &ServerLevel, pos: BlockPos) -> ContainerRef {
        let id = level.chunks.block_entity_ref(pos).map_or(0, BlockEntity::id);
        ContainerRef::Block { pos, id }
    }

    pub fn single_block(self) -> Option<BlockPos> {
        match self {
            ContainerRef::Block { pos, .. } => Some(pos),
            ContainerRef::Compound { .. } => None,
        }
    }

    pub fn is_single(self, pos: BlockPos, id: u64) -> bool {
        self == ContainerRef::Block { pos, id }
    }

    pub fn contains(self, pos: BlockPos, id: u64) -> bool {
        match self {
            ContainerRef::Block { .. } => self.is_single(pos, id),
            ContainerRef::Compound { first, second } => first == (pos, id) || second == (pos, id),
        }
    }

    pub fn parts(self) -> Vec<(BlockPos, u64)> {
        match self {
            ContainerRef::Block { pos, id } => vec![(pos, id)],
            ContainerRef::Compound { first, second } => vec![first, second],
        }
    }
}

fn size_of(entity: &BlockEntity) -> usize {
    entity.container_base().map_or(0, |b| b.items.len())
}

impl ServerLevel {
    fn container_part(&mut self, container: ContainerRef, slot: usize) -> Option<(BlockPos, usize)> {
        match container {
            ContainerRef::Block { pos, .. } => Some((pos, slot)),
            ContainerRef::Compound { first, second } => {
                let first_size = self.get_block_entity(first.0).map_or(0, |e| size_of(e));
                if slot >= first_size { Some((second.0, slot - first_size)) } else { Some((first.0, slot)) }
            }
        }
    }

    fn container_items(&mut self, pos: BlockPos) -> Option<&mut Vec<ItemStack>> {
        self.unpack_loot_table(pos, None);
        Some(&mut self.get_block_entity(pos)?.container_base_mut()?.items)
    }

    pub fn container_size(&mut self, container: ContainerRef) -> usize {
        container.parts().into_iter().map(|(pos, _)| self.get_block_entity(pos).map_or(0, |e| size_of(e))).sum()
    }

    pub fn container_is_empty(&mut self, container: ContainerRef) -> bool {
        container.parts().into_iter().all(|(pos, _)| self.container_items(pos).is_none_or(|items| items.iter().all(ItemStack::is_empty)))
    }

    pub fn get_item(&mut self, container: ContainerRef, slot: usize) -> ItemStack {
        let Some((pos, slot)) = self.container_part(container, slot) else { return ItemStack::empty() };
        self.container_items(pos).and_then(|items| items.get(slot).cloned()).unwrap_or_else(ItemStack::empty)
    }

    pub fn put_item(&mut self, container: ContainerRef, slot: usize, stack: ItemStack) {
        let Some((pos, slot)) = self.container_part(container, slot) else { return };
        if let Some(items) = self.container_items(pos)
            && slot < items.len()
        {
            items[slot] = stack;
        }
    }

    pub fn set_item(&mut self, container: ContainerRef, slot: usize, mut stack: ItemStack) {
        let Some((pos, slot)) = self.container_part(container, slot) else { return };
        self.unpack_loot_table(pos, None);
        let Some(entity) = self.get_block_entity(pos) else { return };
        let changed = match &mut entity.kind {
            BlockEntityKind::Furnace(furnace) => furnace.set_item(slot, stack),
            BlockEntityKind::Hopper(hopper) => {
                stack.limit_size(stack.max_stack_size().min(99));
                hopper.base.items[slot] = stack;
                false
            }
            _ => {
                let Some(base) = entity.container_base_mut() else { return };
                stack.limit_size(stack.max_stack_size().min(99));
                base.items[slot] = stack;
                true
            }
        };
        if changed {
            self.block_entity_set_changed(pos);
        }
    }

    pub fn remove_item(&mut self, container: ContainerRef, slot: usize, count: i32) -> ItemStack {
        let Some((pos, slot)) = self.container_part(container, slot) else { return ItemStack::empty() };
        let is_hopper = matches!(self.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Hopper(_)));
        let Some(items) = self.container_items(pos) else { return ItemStack::empty() };
        let result = match items.get_mut(slot) {
            Some(stack) if !stack.is_empty() && count > 0 => stack.split(count),
            _ => ItemStack::empty(),
        };
        if !result.is_empty() && !is_hopper {
            self.block_entity_set_changed(pos);
        }
        result
    }

    pub fn remove_item_no_update(&mut self, container: ContainerRef, slot: usize) -> ItemStack {
        let Some((pos, slot)) = self.container_part(container, slot) else { return ItemStack::empty() };
        let Some(items) = self.container_items(pos) else { return ItemStack::empty() };
        items.get_mut(slot).map(|s| std::mem::replace(s, ItemStack::empty())).unwrap_or_else(ItemStack::empty)
    }

    pub fn container_set_changed(&mut self, container: ContainerRef) {
        for (pos, _) in container.parts() {
            self.block_entity_set_changed(pos);
        }
    }

    pub fn container_max_stack_size(&self, _container: ContainerRef, stack: &ItemStack) -> i32 {
        stack.max_stack_size().min(99)
    }

    pub fn can_place_item(&mut self, container: ContainerRef, slot: usize, stack: &ItemStack) -> bool {
        let Some((pos, slot)) = self.container_part(container, slot) else { return false };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::Furnace(furnace)) => furnace.can_place_item(slot, stack),
            _ => true,
        }
    }

    pub fn can_take_item(&mut self, _container: ContainerRef, _into: ContainerRef, _slot: usize, _stack: &ItemStack) -> bool {
        true
    }

    pub fn slots_for_face(&mut self, container: ContainerRef, direction: Direction) -> Option<Vec<usize>> {
        let pos = container.single_block()?;
        match self.get_block_entity(pos).map(|e| &e.kind)? {
            BlockEntityKind::ShulkerBox(_) => Some((0..27).collect()),
            BlockEntityKind::Furnace(_) => Some(AbstractFurnaceBlockEntity::slots_for_face(direction).to_vec()),
            _ => None,
        }
    }

    pub fn can_place_item_through_face(&mut self, container: ContainerRef, slot: usize, stack: &ItemStack, _direction: Option<Direction>) -> bool {
        let Some(pos) = container.single_block() else { return true };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::ShulkerBox(_)) => !stack.get_item().info().block.is_some_and(|b| b.is_instance_of("ShulkerBoxBlock")),
            Some(BlockEntityKind::Furnace(furnace)) => furnace.can_place_item(slot, stack),
            _ => true,
        }
    }

    pub fn can_take_item_through_face(&mut self, container: ContainerRef, slot: usize, stack: &ItemStack, direction: Direction) -> bool {
        let Some(pos) = container.single_block() else { return true };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::Furnace(_)) => AbstractFurnaceBlockEntity::can_take_item_through_face(slot, stack, direction),
            _ => true,
        }
    }

    pub fn container_still_valid(&mut self, container: ContainerRef, player: EntityId) -> bool {
        container.parts().into_iter().all(|(pos, id)| {
            self.get_block_entity(pos).is_some_and(|e| e.id() == id) && self.is_within_block_interaction_range(player, pos, DEFAULT_DISTANCE_BUFFER)
        })
    }

    pub fn container_start_open(&mut self, container: ContainerRef, player: EntityId) {
        for (pos, _) in container.parts() {
            let Some(entity) = self.get_block_entity(pos) else { continue };
            if entity.is_removed() || self.is_spectator(player) {
                continue;
            }
            match self.get_block_entity(pos).map(|e| &e.kind) {
                Some(BlockEntityKind::Chest(_) | BlockEntityKind::Barrel(_)) => {
                    self.increment_openers(pos, crate::server::level::game_mode::BLOCK_INTERACTION_RANGE);
                }
                Some(BlockEntityKind::ShulkerBox(_)) => self.shulker_box_start_open(pos),
                _ => {}
            }
        }
    }

    pub fn container_stop_open(&mut self, container: ContainerRef, player: EntityId) {
        for (pos, _) in container.parts() {
            let Some(entity) = self.get_block_entity(pos) else { continue };
            if entity.is_removed() || self.is_spectator(player) {
                continue;
            }
            match self.get_block_entity(pos).map(|e| &e.kind) {
                Some(BlockEntityKind::Chest(_) | BlockEntityKind::Barrel(_)) => self.decrement_openers(pos),
                Some(BlockEntityKind::ShulkerBox(_)) => self.shulker_box_stop_open(pos),
                _ => {}
            }
        }
    }

    pub fn get_block_container(&mut self, pos: BlockPos, state: BlockState) -> Option<ContainerRef> {
        if !state.has_block_entity() {
            return None;
        }
        let entity = self.get_block_entity(pos)?;
        entity.container_base()?;
        if matches!(entity.kind, BlockEntityKind::Chest(_)) && state.block().is_instance_of("ChestBlock") {
            return chest_block::get_container(state, self, pos, true);
        }
        Some(ContainerRef::block(self, pos))
    }

    pub fn get_container_at(&mut self, pos: BlockPos) -> Option<ContainerRef> {
        use crate::world::level::BlockGetter;
        let state = self.block_state(pos);
        self.get_block_container(pos, state)
    }

    pub fn get_redstone_signal_from_container(&mut self, container: Option<ContainerRef>) -> i32 {
        let Some(container) = container else { return 0 };
        let size = self.container_size(container);
        let mut total_percent = 0.0f32;
        for slot in 0..size {
            let stack = self.get_item(container, slot);
            if !stack.is_empty() {
                total_percent += stack.count as f32 / self.container_max_stack_size(container, &stack) as f32;
            }
        }
        total_percent /= size as f32;
        crisp_core::mth::lerp_discrete(total_percent, 0, 15)
    }

    pub fn get_redstone_signal_from_block_entity(&mut self, pos: BlockPos) -> i32 {
        let is_container = self.get_block_entity(pos).is_some_and(|e| e.container_base().is_some());
        let container = is_container.then(|| ContainerRef::block(self, pos));
        self.get_redstone_signal_from_container(container)
    }
}
