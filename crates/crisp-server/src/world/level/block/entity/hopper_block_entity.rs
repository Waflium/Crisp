use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::properties::{ENABLED, FACING_HOPPER};
use crisp_data::{BlockState, block_tags};
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::{BlockEntityKind, int_or};
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;

pub const MOVE_ITEM_SPEED: i32 = 8;
pub const HOPPER_CONTAINER_SIZE: usize = 5;
const NO_COOLDOWN_TIME: i32 = -1;
pub const SUCK_AABB: AABB = AABB { min_x: 0.0, min_y: 11.0 / 16.0, min_z: 0.0, max_x: 1.0, max_y: 2.0, max_z: 1.0 };

#[derive(Clone)]
pub struct HopperBlockEntity {
    pub base: BaseContainerBlockEntity,
    pub cooldown_time: i32,
    pub ticked_game_time: i64,
    pub facing: Direction,
}

impl HopperBlockEntity {
    pub fn new(state: BlockState) -> HopperBlockEntity {
        HopperBlockEntity { base: BaseContainerBlockEntity::with_size(HOPPER_CONTAINER_SIZE), cooldown_time: NO_COOLDOWN_TIME, ticked_game_time: 0, facing: state.get(FACING_HOPPER) }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.items = vec![ItemStack::empty(); HOPPER_CONTAINER_SIZE];
        if !self.base.try_load_loot_table(input) {
            self.base.load_all_items(input, HOPPER_CONTAINER_SIZE);
        }
        self.cooldown_time = int_or(input, "TransferCooldown", NO_COOLDOWN_TIME);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        if !self.base.try_save_loot_table(output) {
            save_all_items(output, &self.base.items, true);
        }
        output.push(("TransferCooldown".into(), Nbt::Int(self.cooldown_time)));
    }

    pub fn set_block_state(&mut self, state: BlockState) {
        self.facing = state.get(FACING_HOPPER);
    }

    fn is_on_cooldown(&self) -> bool {
        self.cooldown_time > 0
    }

    pub fn is_on_custom_cooldown(&self) -> bool {
        self.cooldown_time > MOVE_ITEM_SPEED
    }

    fn inventory_full(&self) -> bool {
        self.base.items.iter().all(|s| !s.is_empty() && s.count == s.max_stack_size())
    }
}

enum SuckAction {
    Container,
    Item(EntityId),
}

impl ServerLevel {
    fn hopper(&mut self, pos: BlockPos) -> Option<&mut HopperBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Hopper(hopper) => Some(hopper),
            _ => None,
        }
    }

    pub fn hopper_push_items_tick(&mut self, pos: BlockPos, state: BlockState) {
        let game_time = self.game_time;
        let Some(hopper) = self.hopper(pos) else { return };
        hopper.cooldown_time -= 1;
        hopper.ticked_game_time = game_time;
        if !hopper.is_on_cooldown() {
            hopper.cooldown_time = 0;
            self.hopper_try_move_items(pos, state, SuckAction::Container);
        }
    }

    fn hopper_try_move_items(&mut self, pos: BlockPos, state: BlockState, action: SuckAction) -> bool {
        let Some(hopper) = self.hopper(pos) else { return false };
        if hopper.is_on_cooldown() || !state.get(ENABLED) {
            return false;
        }
        let (is_empty, full) = (hopper.base.items.iter().all(ItemStack::is_empty), hopper.inventory_full());
        let mut changed = false;
        if !is_empty {
            changed = self.hopper_eject_items(pos);
        }
        if !full {
            changed |= match action {
                SuckAction::Container => self.suck_in_items(pos),
                SuckAction::Item(item) => self.hopper_add_item_entity(ContainerRef::block(self, pos), item, false),
            };
        }
        if changed {
            if let Some(hopper) = self.hopper(pos) {
                hopper.cooldown_time = MOVE_ITEM_SPEED;
            }
            self.block_entity_set_changed_static(pos, state);
            return true;
        }
        false
    }

    fn hopper_eject_items(&mut self, pos: BlockPos) -> bool {
        let Some(facing) = self.hopper(pos).map(|h| h.facing) else { return false };
        let Some(container) = self.get_container_at(pos.relative(facing)) else { return false };
        let this = ContainerRef::block(self, pos);
        let direction = facing.opposite();
        if self.is_full_container(container, direction) {
            return false;
        }
        for slot in 0..self.container_size(this) {
            let stack = self.get_item(this, slot);
            if !stack.is_empty() {
                let original_count = stack.count;
                let one = self.remove_item(this, slot, 1);
                let result = self.add_item(Some(this), container, one, Some(direction));
                if result.is_empty() {
                    self.container_set_changed(container);
                    return true;
                }
                let mut restored = self.get_item(this, slot);
                restored.count = original_count;
                if original_count == 1 {
                    self.set_item(this, slot, restored);
                } else {
                    self.put_item(this, slot, restored);
                }
            }
        }
        false
    }

    pub fn get_slots(&mut self, container: ContainerRef, direction: Direction) -> Vec<usize> {
        match self.slots_for_face(container, direction) {
            Some(slots) => slots,
            None => (0..self.container_size(container)).collect(),
        }
    }

    fn is_full_container(&mut self, container: ContainerRef, direction: Direction) -> bool {
        for slot in self.get_slots(container, direction) {
            let stack = self.get_item(container, slot);
            if stack.count < stack.max_stack_size() {
                return false;
            }
        }
        true
    }

    fn suck_in_items(&mut self, pos: BlockPos) -> bool {
        let this = ContainerRef::block(self, pos);
        let above = pos.above();
        let above_state = self.block_state(above);
        if let Some(container) = self.get_source_container(pos, above, above_state) {
            for slot in self.get_slots(container, Direction::Down) {
                if self.try_take_in_item_from_slot(this, container, slot, Direction::Down) {
                    return true;
                }
            }
            return false;
        }
        let is_blocked = above_state.is_collision_shape_full_block() && !above_state.is(&block_tags::DOES_NOT_BLOCK_HOPPERS);
        if !is_blocked {
            for item in self.get_items_at_and_above(pos) {
                if self.hopper_add_item_entity(this, item, true) {
                    return true;
                }
            }
        }
        false
    }

    fn try_take_in_item_from_slot(&mut self, hopper: ContainerRef, container: ContainerRef, slot: usize, direction: Direction) -> bool {
        let stack = self.get_item(container, slot);
        if !stack.is_empty() && self.can_take_item_from_container(hopper, container, &stack, slot, direction) {
            let original_count = stack.count;
            let one = self.remove_item(container, slot, 1);
            let result = self.add_item(Some(container), hopper, one, None);
            if result.is_empty() {
                self.container_set_changed(container);
                return true;
            }
            let mut restored = self.get_item(container, slot);
            restored.count = original_count;
            if original_count == 1 {
                self.set_item(container, slot, restored);
            } else {
                self.put_item(container, slot, restored);
            }
        }
        false
    }

    pub fn hopper_add_item_entity(&mut self, container: ContainerRef, item: EntityId, remove_now: bool) -> bool {
        let Some(LevelEntity::Item(entity)) = self.entities.get(&item) else { return false };
        let copy = entity.item.copy();
        let result = self.add_item(None, container, copy, None);
        let Some(LevelEntity::Item(entity)) = self.entities.get_mut(&item) else { return false };
        if result.is_empty() {
            entity.set_item(ItemStack::empty());
            entity.entity.discard();
            if remove_now {
                self.remove_entity(item);
            }
            true
        } else {
            entity.set_item(result);
            false
        }
    }

    pub fn add_item(&mut self, from: Option<ContainerRef>, container: ContainerRef, mut stack: ItemStack, direction: Option<Direction>) -> ItemStack {
        let slots = match direction.and_then(|d| self.slots_for_face(container, d)) {
            Some(slots) => slots,
            None => (0..self.container_size(container)).collect(),
        };
        for slot in slots {
            if stack.is_empty() {
                break;
            }
            stack = self.try_move_in_item(from, container, stack, slot, direction);
        }
        stack
    }

    fn can_place_item_in_container(&mut self, container: ContainerRef, stack: &ItemStack, slot: usize, direction: Option<Direction>) -> bool {
        self.can_place_item(container, slot, stack) && self.can_place_item_through_face(container, slot, stack, direction)
    }

    fn can_take_item_from_container(&mut self, into: ContainerRef, from: ContainerRef, stack: &ItemStack, slot: usize, direction: Direction) -> bool {
        self.can_take_item(from, into, slot, stack) && self.can_take_item_through_face(from, slot, stack, direction)
    }

    fn try_move_in_item(&mut self, from: Option<ContainerRef>, container: ContainerRef, mut stack: ItemStack, slot: usize, direction: Option<Direction>) -> ItemStack {
        let current = self.get_item(container, slot);
        if self.can_place_item_in_container(container, &stack, slot, direction) {
            let mut success = false;
            let was_empty = self.container_is_empty(container);
            if current.is_empty() {
                self.set_item(container, slot, stack);
                stack = ItemStack::empty();
                success = true;
            } else if can_merge_items(&current, &stack) {
                let space = stack.max_stack_size() - current.count;
                let count = stack.count.min(space);
                stack.shrink(count);
                let mut grown = current;
                grown.grow(count);
                self.put_item(container, slot, grown);
                success = count > 0;
            }
            if success {
                if was_empty && let Some(target) = container.single_block() {
                    let from_ticked = from.and_then(|f| f.single_block()).and_then(|p| self.hopper(p).map(|h| h.ticked_game_time));
                    if let Some(hopper) = self.hopper(target)
                        && !hopper.is_on_custom_cooldown()
                    {
                        let skip_tick_count = if from_ticked.is_some_and(|t| hopper.ticked_game_time >= t) { 1 } else { 0 };
                        hopper.cooldown_time = MOVE_ITEM_SPEED - skip_tick_count;
                    }
                }
                self.container_set_changed(container);
            }
        }
        stack
    }

    fn get_source_container(&mut self, hopper: BlockPos, pos: BlockPos, state: BlockState) -> Option<ContainerRef> {
        let _ = hopper;
        self.get_block_container(pos, state)
    }

    fn get_items_at_and_above(&self, pos: BlockPos) -> Vec<EntityId> {
        let area = SUCK_AABB.move_by(pos.x as f64, pos.y as f64, pos.z as f64);
        self.entities
            .iter()
            .filter(|(_, e)| !e.entity().is_removed() && e.as_item().is_some() && e.entity().bounding_box().intersects(&area))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn hopper_entity_inside(&mut self, pos: BlockPos, state: BlockState, entity: EntityId) {
        let Some(LevelEntity::Item(item)) = self.entities.get(&entity) else { return };
        let bb = item.entity.bounding_box().move_by(-pos.x as f64, -pos.y as f64, -pos.z as f64);
        if !item.item.is_empty() && bb.intersects(&SUCK_AABB) {
            self.hopper_try_move_items(pos, state, SuckAction::Item(entity));
        }
    }
}

fn can_merge_items(a: &ItemStack, b: &ItemStack) -> bool {
    a.count <= a.max_stack_size() && ItemStack::is_same_item_same_components(a, b)
}
