use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::abstract_furnace_block_entity::AbstractFurnaceBlockEntity;
use crate::world::level::block::entity::brewing_stand_block_entity::BrewingStandBlockEntity;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind};
use crate::world::level::block::chest_block;

pub const DEFAULT_DISTANCE_BUFFER: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContainerRef {
    Block { pos: BlockPos, id: u64 },
    Compound { first: (BlockPos, u64), second: (BlockPos, u64) },
    Composter(BlockPos),
    Jukebox(BlockPos),
    Lectern { pos: BlockPos, id: u64 },
    ChiseledBookShelf(BlockPos),
    Shelf(BlockPos),
    DecoratedPot(BlockPos),
    PlayerEnderChest(EntityId),
    ChestVehicle(EntityId),
}

impl ContainerRef {
    pub fn block(level: &ServerLevel, pos: BlockPos) -> ContainerRef {
        let id = level.chunks.block_entity_ref(pos).map_or(0, BlockEntity::id);
        ContainerRef::Block { pos, id }
    }

    pub fn lectern_book_access(level: &ServerLevel, pos: BlockPos) -> ContainerRef {
        let id = level.chunks.block_entity_ref(pos).map_or(0, BlockEntity::id);
        ContainerRef::Lectern { pos, id }
    }

    pub fn single_block(self) -> Option<BlockPos> {
        match self {
            ContainerRef::Block { pos, .. } => Some(pos),
            _ => None,
        }
    }

    pub fn is_single(self, pos: BlockPos, id: u64) -> bool {
        self == ContainerRef::Block { pos, id }
    }

    pub fn contains(self, pos: BlockPos, id: u64) -> bool {
        match self {
            ContainerRef::Block { .. } => self.is_single(pos, id),
            ContainerRef::Compound { first, second } => first == (pos, id) || second == (pos, id),
            ContainerRef::Lectern { pos: p, id: i } => (p, i) == (pos, id),
            _ => false,
        }
    }

    pub fn parts(self) -> Vec<(BlockPos, u64)> {
        match self {
            ContainerRef::Block { pos, id } | ContainerRef::Lectern { pos, id } => vec![(pos, id)],
            ContainerRef::Compound { first, second } => vec![first, second],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ComposterContainer {
    Empty,
    Input,
    Output,
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
            _ => None,
        }
    }

    fn list_backed_items(&mut self, container: ContainerRef) -> Option<&mut Vec<ItemStack>> {
        match container {
            ContainerRef::ChiseledBookShelf(pos) => self.chiseled_book_shelf(pos).map(|bookshelf| bookshelf.get_items_mut()),
            ContainerRef::Shelf(pos) => self.shelf(pos).map(|shelf| shelf.get_items_mut()),
            ContainerRef::PlayerEnderChest(player) => self.players.get_mut(&player).map(|p| &mut p.ender_chest_inventory.items),
            ContainerRef::ChestVehicle(entity) => {
                self.unpack_chest_vehicle_loot_table(entity, None);
                self.container_entity(entity).map(|container| container.get_item_stacks())
            }
            _ => None,
        }
    }

    fn container_items(&mut self, pos: BlockPos) -> Option<&mut Vec<ItemStack>> {
        self.unpack_loot_table(pos, None);
        Some(&mut self.get_block_entity(pos)?.container_base_mut()?.items)
    }

    fn composter_container(&mut self, pos: BlockPos) -> ComposterContainer {
        use crate::world::level::BlockGetter;
        let content_level = self.block_state(pos).get(crisp_data::properties::LEVEL_COMPOSTER);
        if content_level == 8 {
            ComposterContainer::Output
        } else if content_level < 7 {
            ComposterContainer::Input
        } else {
            ComposterContainer::Empty
        }
    }

    pub fn container_size(&mut self, container: ContainerRef) -> usize {
        if let ContainerRef::Composter(pos) = container {
            return if self.composter_container(pos) == ComposterContainer::Empty { 0 } else { 1 };
        }
        if let ContainerRef::Jukebox(_) | ContainerRef::Lectern { .. } | ContainerRef::DecoratedPot(_) = container {
            return 1;
        }
        if let ContainerRef::ChestVehicle(entity) = container {
            return self.container_entity(entity).map_or(0, |container| container.get_item_stacks().len());
        }
        if let ContainerRef::ChiseledBookShelf(_) | ContainerRef::Shelf(_) | ContainerRef::PlayerEnderChest(_) = container {
            return self.list_backed_items(container).map_or(0, |items| items.len());
        }
        container.parts().into_iter().map(|(pos, _)| self.get_block_entity(pos).map_or(0, |e| size_of(e))).sum()
    }

    pub fn container_is_empty(&mut self, container: ContainerRef) -> bool {
        if let ContainerRef::Composter(pos) = container {
            return self.composter_container(pos) != ComposterContainer::Output;
        }
        if let ContainerRef::Jukebox(pos) = container {
            return self.jukebox_get_the_item(pos).is_empty();
        }
        if let ContainerRef::Lectern { pos, .. } = container {
            return self.lectern(pos).is_none_or(|lectern| lectern.get_book().is_empty());
        }
        if let ContainerRef::DecoratedPot(pos) = container {
            return self.decorated_pot_get_the_item(pos).is_empty();
        }
        if let ContainerRef::ChestVehicle(entity) = container {
            return self.container_entity(entity).is_none_or(|container| container.is_chest_vehicle_empty());
        }
        if let ContainerRef::ChiseledBookShelf(_) | ContainerRef::Shelf(_) | ContainerRef::PlayerEnderChest(_) = container {
            return self.list_backed_items(container).is_none_or(|items| items.iter().all(ItemStack::is_empty));
        }
        if let Some(crafter) = container.single_block().and_then(|pos| self.crafter(pos)) {
            return crafter.is_empty();
        }
        container.parts().into_iter().all(|(pos, _)| self.container_items(pos).is_none_or(|items| items.iter().all(ItemStack::is_empty)))
    }

    pub fn get_item(&mut self, container: ContainerRef, slot: usize) -> ItemStack {
        if let ContainerRef::Composter(pos) = container {
            return if slot == 0 && self.composter_container(pos) == ComposterContainer::Output {
                ItemStack::new(crisp_data::items::BONE_MEAL, 1)
            } else {
                ItemStack::empty()
            };
        }
        if let ContainerRef::Jukebox(pos) = container {
            return if slot == 0 { self.jukebox_get_the_item(pos) } else { ItemStack::empty() };
        }
        if let ContainerRef::Lectern { pos, .. } = container {
            return if slot == 0 { self.lectern(pos).map_or_else(ItemStack::empty, |lectern| lectern.get_book().clone()) } else { ItemStack::empty() };
        }
        if let ContainerRef::DecoratedPot(pos) = container {
            return if slot == 0 { self.decorated_pot_get_the_item(pos) } else { ItemStack::empty() };
        }
        if let ContainerRef::ChiseledBookShelf(_) | ContainerRef::Shelf(_) | ContainerRef::PlayerEnderChest(_) | ContainerRef::ChestVehicle(_) = container {
            return self.list_backed_items(container).and_then(|items| items.get(slot).cloned()).unwrap_or_else(ItemStack::empty);
        }
        let Some((pos, slot)) = self.container_part(container, slot) else { return ItemStack::empty() };
        if let Some(crafter) = self.crafter(pos) {
            return crafter.get_item(slot).clone();
        }
        self.container_items(pos).and_then(|items| items.get(slot).cloned()).unwrap_or_else(ItemStack::empty)
    }

    pub fn put_item(&mut self, container: ContainerRef, slot: usize, stack: ItemStack) {
        if let ContainerRef::DecoratedPot(pos) = container {
            if slot == 0 {
                self.decorated_pot_set_the_item(pos, stack);
            }
            return;
        }
        if let ContainerRef::ChiseledBookShelf(_) | ContainerRef::Shelf(_) | ContainerRef::PlayerEnderChest(_) | ContainerRef::ChestVehicle(_) = container {
            if let Some(items) = self.list_backed_items(container)
                && slot < items.len()
            {
                items[slot] = stack;
            }
            return;
        }
        let Some((pos, slot)) = self.container_part(container, slot) else { return };
        if let Some(items) = self.container_items(pos)
            && slot < items.len()
        {
            items[slot] = stack;
        }
    }

    pub fn set_item(&mut self, container: ContainerRef, slot: usize, mut stack: ItemStack) {
        if let ContainerRef::Composter(pos) = container {
            if slot == 0 && self.composter_container(pos) == ComposterContainer::Input {
                crate::world::level::block::composter_block::insert_through_container(self, pos, &stack);
            }
            return;
        }
        if let ContainerRef::Jukebox(pos) = container {
            if slot == 0 {
                self.jukebox_set_the_item(pos, stack);
            }
            return;
        }
        if let ContainerRef::ChiseledBookShelf(pos) = container {
            return self.chiseled_book_shelf_set_item(pos, slot, stack);
        }
        if let ContainerRef::DecoratedPot(pos) = container {
            if slot == 0 {
                self.decorated_pot_set_the_item(pos, stack);
            }
            return;
        }
        if let ContainerRef::Shelf(pos) = container {
            if let Some(items) = self.list_backed_items(container) {
                stack.limit_size(stack.max_stack_size().min(99));
                items[slot] = stack;
            }
            return self.shelf_set_changed(pos);
        }
        if let ContainerRef::PlayerEnderChest(_) | ContainerRef::ChestVehicle(_) = container {
            if let Some(items) = self.list_backed_items(container) {
                stack.limit_size(stack.max_stack_size().min(99));
                items[slot] = stack;
            }
            return;
        }
        let Some((pos, slot)) = self.container_part(container, slot) else { return };
        if self.crafter(pos).is_some_and(|crafter| crafter.is_slot_disabled(slot as i32)) {
            self.crafter_set_slot_state(pos, slot as i32, true);
        }
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
        if let ContainerRef::Composter(_) = container {
            let item = self.get_item(container, slot);
            return if count > 0 { item } else { ItemStack::empty() };
        }
        if let ContainerRef::Jukebox(pos) = container {
            return if slot == 0 { self.jukebox_split_the_item(pos) } else { ItemStack::empty() };
        }
        if let ContainerRef::Lectern { pos, .. } = container {
            return self.lectern_book_access_remove_item(pos, slot, count);
        }
        if let ContainerRef::ChiseledBookShelf(pos) = container {
            return self.chiseled_book_shelf_remove_item(pos, slot);
        }
        if let ContainerRef::DecoratedPot(pos) = container {
            return if slot != 0 { ItemStack::empty() } else { self.decorated_pot_split_the_item(pos, count) };
        }
        if let ContainerRef::Shelf(_) | ContainerRef::PlayerEnderChest(_) | ContainerRef::ChestVehicle(_) = container {
            let result = match self.list_backed_items(container).and_then(|items| items.get_mut(slot)) {
                Some(stack) if !stack.is_empty() && count > 0 => stack.split(count),
                _ => ItemStack::empty(),
            };
            if let ContainerRef::Shelf(pos) = container
                && !result.is_empty()
            {
                self.shelf_set_changed(pos);
            }
            return result;
        }
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
        if let ContainerRef::Composter(_) = container {
            return self.get_item(container, slot);
        }
        if let ContainerRef::Jukebox(_) = container {
            return self.remove_item(container, slot, 1);
        }
        if let ContainerRef::DecoratedPot(_) = container {
            return self.remove_item(container, slot, 99);
        }
        if let ContainerRef::Lectern { pos, .. } = container {
            return self.lectern_book_access_remove_item_no_update(pos, slot);
        }
        if let ContainerRef::PlayerEnderChest(_) | ContainerRef::ChestVehicle(_) = container {
            return self.list_backed_items(container).and_then(|items| items.get_mut(slot)).map(|s| std::mem::replace(s, ItemStack::empty())).unwrap_or_else(ItemStack::empty);
        }
        if let ContainerRef::ChiseledBookShelf(_) | ContainerRef::Shelf(_) = container {
            let max_stack_size = if let ContainerRef::Shelf(_) = container { 99 } else { 1 };
            return match self.list_backed_items(container).and_then(|items| items.get_mut(slot)) {
                Some(stack) if !stack.is_empty() => stack.split(max_stack_size),
                _ => ItemStack::empty(),
            };
        }
        let Some((pos, slot)) = self.container_part(container, slot) else { return ItemStack::empty() };
        let Some(items) = self.container_items(pos) else { return ItemStack::empty() };
        items.get_mut(slot).map(|s| std::mem::replace(s, ItemStack::empty())).unwrap_or_else(ItemStack::empty)
    }

    pub fn container_set_changed(&mut self, container: ContainerRef) {
        if let ContainerRef::Composter(pos) = container {
            if self.composter_container(pos) == ComposterContainer::Output {
                crate::world::level::block::composter_block::empty(self, pos);
            }
            return;
        }
        if let ContainerRef::Jukebox(pos) | ContainerRef::ChiseledBookShelf(pos) | ContainerRef::DecoratedPot(pos) = container {
            return self.block_entity_set_changed(pos);
        }
        if let ContainerRef::Shelf(pos) = container {
            return self.shelf_set_changed(pos);
        }
        for (pos, _) in container.parts() {
            self.block_entity_set_changed(pos);
        }
    }

    pub fn container_max_stack_size(&self, container: ContainerRef, stack: &ItemStack) -> i32 {
        if let ContainerRef::Composter(_) | ContainerRef::Jukebox(_) | ContainerRef::Lectern { .. } | ContainerRef::ChiseledBookShelf(_) = container {
            return stack.max_stack_size().min(1);
        }
        stack.max_stack_size().min(99)
    }

    pub fn can_place_item(&mut self, container: ContainerRef, slot: usize, stack: &ItemStack) -> bool {
        if let ContainerRef::Composter(_) = container {
            return true;
        }
        if let ContainerRef::Jukebox(pos) = container {
            return self.jukebox_can_place_item(pos, slot, stack);
        }
        if let ContainerRef::Lectern { .. } = container {
            return false;
        }
        if let ContainerRef::DecoratedPot(_) | ContainerRef::PlayerEnderChest(_) | ContainerRef::ChestVehicle(_) = container {
            return true;
        }
        if let ContainerRef::ChiseledBookShelf(pos) = container {
            return self.chiseled_book_shelf(pos).is_some_and(|bookshelf| bookshelf.can_place_item(slot, stack));
        }
        if let ContainerRef::Shelf(_) = container {
            let max_stack_size = self.container_max_stack_size(container, stack);
            return self.list_backed_items(container).is_some_and(|items| items[slot].is_empty() || items[slot].count < max_stack_size);
        }
        let Some((pos, slot)) = self.container_part(container, slot) else { return false };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::Furnace(furnace)) => furnace.can_place_item(slot, stack),
            Some(BlockEntityKind::BrewingStand(brewing_stand)) => brewing_stand.can_place_item(slot, stack),
            Some(BlockEntityKind::Crafter(crafter)) => crafter.can_place_item(slot, stack),
            _ => true,
        }
    }

    pub fn can_take_item(&mut self, container: ContainerRef, into: ContainerRef, _slot: usize, stack: &ItemStack) -> bool {
        if let ContainerRef::Jukebox(_) = container {
            return (0..self.container_size(into)).any(|slot| self.get_item(into, slot).is_empty());
        }
        if let ContainerRef::ChiseledBookShelf(_) = container {
            return (0..self.container_size(into)).any(|slot| {
                let to_item = self.get_item(into, slot);
                to_item.is_empty()
                    || ItemStack::is_same_item_same_components(stack, &to_item) && to_item.count + stack.count <= self.container_max_stack_size(into, &to_item)
            });
        }
        true
    }

    pub fn slots_for_face(&mut self, container: ContainerRef, direction: Direction) -> Option<Vec<usize>> {
        if let ContainerRef::Composter(pos) = container {
            let face = match self.composter_container(pos) {
                ComposterContainer::Input => Direction::Up,
                ComposterContainer::Output => Direction::Down,
                ComposterContainer::Empty => return Some(Vec::new()),
            };
            return Some(if direction == face { vec![0] } else { Vec::new() });
        }
        let pos = container.single_block()?;
        match self.get_block_entity(pos).map(|e| &e.kind)? {
            BlockEntityKind::ShulkerBox(_) => Some((0..27).collect()),
            BlockEntityKind::Furnace(_) => Some(AbstractFurnaceBlockEntity::slots_for_face(direction).to_vec()),
            BlockEntityKind::BrewingStand(_) => Some(BrewingStandBlockEntity::get_slots_for_face(direction).to_vec()),
            _ => None,
        }
    }

    pub fn can_place_item_through_face(&mut self, container: ContainerRef, slot: usize, stack: &ItemStack, direction: Option<Direction>) -> bool {
        if let ContainerRef::Composter(pos) = container {
            return self.composter_container(pos) == ComposterContainer::Input
                && direction == Some(Direction::Up)
                && stack.has(crate::world::item::component::COMPOSTABLE);
        }
        let Some(pos) = container.single_block() else { return true };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::ShulkerBox(_)) => !stack.get_item().info().block.is_some_and(|b| b.is_instance_of("ShulkerBoxBlock")),
            Some(BlockEntityKind::Furnace(furnace)) => furnace.can_place_item(slot, stack),
            Some(BlockEntityKind::BrewingStand(brewing_stand)) => brewing_stand.can_place_item(slot, stack),
            _ => true,
        }
    }

    pub fn can_take_item_through_face(&mut self, container: ContainerRef, slot: usize, stack: &ItemStack, direction: Direction) -> bool {
        if let ContainerRef::Composter(pos) = container {
            return self.composter_container(pos) == ComposterContainer::Output && direction == Direction::Down && stack.is(crisp_data::items::BONE_MEAL);
        }
        let Some(pos) = container.single_block() else { return true };
        match self.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::Furnace(_)) => AbstractFurnaceBlockEntity::can_take_item_through_face(slot, stack, direction),
            Some(BlockEntityKind::BrewingStand(_)) => BrewingStandBlockEntity::can_take_item_through_face(slot, stack),
            _ => true,
        }
    }

    pub fn container_still_valid(&mut self, container: ContainerRef, player: EntityId) -> bool {
        if let ContainerRef::ChestVehicle(entity) = container {
            return self.is_chest_vehicle_still_valid(entity, player);
        }
        if let ContainerRef::PlayerEnderChest(owner) = container {
            let Some((pos, id)) = self.player(owner).ender_chest_inventory.active_chest() else { return true };
            return self.get_block_entity(pos).is_some_and(|e| e.id() == id) && self.is_within_block_interaction_range(player, pos, DEFAULT_DISTANCE_BUFFER);
        }
        let still_valid_block_entity = container.parts().into_iter().all(|(pos, id)| {
            self.get_block_entity(pos).is_some_and(|e| e.id() == id) && self.is_within_block_interaction_range(player, pos, DEFAULT_DISTANCE_BUFFER)
        });
        if let ContainerRef::Lectern { pos, .. } = container {
            return still_valid_block_entity && self.lectern(pos).is_some_and(|lectern| lectern.has_book());
        }
        still_valid_block_entity
    }

    pub fn container_start_open(&mut self, container: ContainerRef, player: EntityId) {
        if let ContainerRef::PlayerEnderChest(owner) = container {
            if let Some((pos, _)) = self.player(owner).ender_chest_inventory.active_chest() {
                self.ender_chest_start_open(pos, player);
            }
            return;
        }
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
        if let ContainerRef::PlayerEnderChest(owner) = container {
            if let Some((pos, _)) = self.player(owner).ender_chest_inventory.active_chest() {
                self.ender_chest_stop_open(pos, player);
            }
            self.player_mut(owner).ender_chest_inventory.set_active_chest(None);
            return;
        }
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
        if state.is_block(crisp_data::blocks::COMPOSTER) {
            return Some(ContainerRef::Composter(pos));
        }
        if !state.has_block_entity() {
            return None;
        }
        let entity = self.get_block_entity(pos)?;
        if let BlockEntityKind::Jukebox(_) = entity.kind {
            return Some(ContainerRef::Jukebox(pos));
        }
        if let BlockEntityKind::ChiseledBookShelf(_) = entity.kind {
            return Some(ContainerRef::ChiseledBookShelf(pos));
        }
        if let BlockEntityKind::Shelf(_) = entity.kind {
            return Some(ContainerRef::Shelf(pos));
        }
        if let BlockEntityKind::DecoratedPot(_) = entity.kind {
            return Some(ContainerRef::DecoratedPot(pos));
        }
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
        let container = match self.get_block_entity(pos).map(|e| (&e.kind, e.container_base().is_some())) {
            Some((BlockEntityKind::Jukebox(_), _)) => Some(ContainerRef::Jukebox(pos)),
            Some((BlockEntityKind::ChiseledBookShelf(_), _)) => Some(ContainerRef::ChiseledBookShelf(pos)),
            Some((BlockEntityKind::Shelf(_), _)) => Some(ContainerRef::Shelf(pos)),
            Some((BlockEntityKind::DecoratedPot(_), _)) => Some(ContainerRef::DecoratedPot(pos)),
            Some((_, true)) => Some(ContainerRef::block(self, pos)),
            _ => None,
        };
        self.get_redstone_signal_from_container(container)
    }
}
