use std::collections::{HashMap, VecDeque};

use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use crisp_data::properties::SideChainPart;

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub trait SideChainPartBlock {
    fn get_side_chain_part(&self, state: BlockState) -> SideChainPart;

    fn set_side_chain_part(&self, state: BlockState, new_part: SideChainPart) -> BlockState;

    fn get_facing(&self, state: BlockState) -> Direction;

    fn is_connectable(&self, state: BlockState) -> bool;

    fn get_max_chain_length(&self) -> i32;

    fn get_all_blocks_connected_to(&self, level: &mut ServerLevel, pos: BlockPos) -> Vec<BlockPos> {
        let state = level.block_state(pos);
        if !self.is_connectable(state) {
            return Vec::new();
        }
        let mut neighbors = Neighbors::new(self.get_facing(state), pos);
        let mut results = VecDeque::new();
        results.push_back(pos);
        for steps in 1..self.get_max_chain_length() {
            let neighbor = neighbors.left(self, level, steps);
            if neighbor.connects_towards(SideChainPart::Left) {
                results.push_front(neighbor.pos());
            }
            if neighbor.is_unconnectable_or_chain_end() {
                break;
            }
        }
        for steps in 1..self.get_max_chain_length() {
            let neighbor = neighbors.right(self, level, steps);
            if neighbor.connects_towards(SideChainPart::Right) {
                results.push_back(neighbor.pos());
            }
            if neighbor.is_unconnectable_or_chain_end() {
                break;
            }
        }
        results.into()
    }

    fn update_neighbors_after_powering_down(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let mut neighbors = Neighbors::new(self.get_facing(state), pos);
        neighbors.left(self, level, 1).disconnect_from_right(self, level);
        neighbors.right(self, level, 1).disconnect_from_left(self, level);
    }

    fn update_self_and_neighbors_on_powering_up(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, old_state: BlockState) {
        if !self.is_connectable(state) || self.is_being_updated_by_neighbor(state, old_state) {
            return;
        }
        let mut neighbors = Neighbors::new(self.get_facing(state), pos);
        let mut new_part_for_self = SideChainPart::Unconnected;
        let left = neighbors.left(self, level, 1);
        let existing_chain_on_the_left = if left.is_connectable() { self.get_all_blocks_connected_to(level, left.pos()).len() as i32 } else { 0 };
        let right = neighbors.right(self, level, 1);
        let existing_chain_on_the_right = if right.is_connectable() { self.get_all_blocks_connected_to(level, right.pos()).len() as i32 } else { 0 };
        let mut current_chain_length = 1;
        if self.can_connect(existing_chain_on_the_left, current_chain_length) {
            new_part_for_self = new_part_for_self.when_connected_to_the_left();
            left.connect_to_the_right(self, level);
            current_chain_length += existing_chain_on_the_left;
        }
        if self.can_connect(existing_chain_on_the_right, current_chain_length) {
            new_part_for_self = new_part_for_self.when_connected_to_the_right();
            right.connect_to_the_left(self, level);
        }
        self.set_part(level, pos, new_part_for_self);
    }

    fn can_connect(&self, new_blocks_to_connect_to: i32, current_chain_length: i32) -> bool {
        new_blocks_to_connect_to > 0 && current_chain_length + new_blocks_to_connect_to <= self.get_max_chain_length()
    }

    fn is_being_updated_by_neighbor(&self, state: BlockState, old_state: BlockState) -> bool {
        let is_getting_connected = self.get_side_chain_part(state).is_connected();
        let has_been_connected_before = self.is_connectable(old_state) && self.get_side_chain_part(old_state).is_connected();
        is_getting_connected || has_been_connected_before
    }

    fn set_part(&self, level: &mut ServerLevel, pos: BlockPos, new_part: SideChainPart) {
        let state = level.block_state(pos);
        if self.get_side_chain_part(state) != new_part {
            level.set_block(pos, self.set_side_chain_part(state, new_part), UPDATE_ALL);
        }
    }
}

#[derive(Clone, Copy)]
enum Neighbor {
    Empty(BlockPos),
    SideChain(BlockPos, SideChainPart),
}

impl Neighbor {
    fn pos(self) -> BlockPos {
        match self {
            Neighbor::Empty(pos) | Neighbor::SideChain(pos, _) => pos,
        }
    }

    fn is_connectable(self) -> bool {
        matches!(self, Neighbor::SideChain(..))
    }

    fn is_unconnectable_or_chain_end(self) -> bool {
        match self {
            Neighbor::Empty(_) => true,
            Neighbor::SideChain(_, part) => part.is_chain_end(),
        }
    }

    fn connects_towards(self, end_part: SideChainPart) -> bool {
        match self {
            Neighbor::Empty(_) => false,
            Neighbor::SideChain(_, part) => part.is_connection_towards(end_part),
        }
    }

    fn connect_to_the_right<B: SideChainPartBlock + ?Sized>(self, block: &B, level: &mut ServerLevel) {
        if let Neighbor::SideChain(pos, part) = self {
            block.set_part(level, pos, part.when_connected_to_the_right());
        }
    }

    fn connect_to_the_left<B: SideChainPartBlock + ?Sized>(self, block: &B, level: &mut ServerLevel) {
        if let Neighbor::SideChain(pos, part) = self {
            block.set_part(level, pos, part.when_connected_to_the_left());
        }
    }

    fn disconnect_from_right<B: SideChainPartBlock + ?Sized>(self, block: &B, level: &mut ServerLevel) {
        if let Neighbor::SideChain(pos, part) = self {
            block.set_part(level, pos, part.when_disconnected_from_the_right());
        }
    }

    fn disconnect_from_left<B: SideChainPartBlock + ?Sized>(self, block: &B, level: &mut ServerLevel) {
        if let Neighbor::SideChain(pos, part) = self {
            block.set_part(level, pos, part.when_disconnected_from_the_left());
        }
    }
}

struct Neighbors {
    facing: Direction,
    center: BlockPos,
    cache: HashMap<BlockPos, Neighbor>,
}

impl Neighbors {
    fn new(facing: Direction, center: BlockPos) -> Neighbors {
        Neighbors { facing, center, cache: HashMap::new() }
    }

    fn create_new_neighbor<B: SideChainPartBlock + ?Sized>(&self, block: &B, level: &mut ServerLevel, pos: BlockPos) -> Neighbor {
        let neighbor = level.block_state(pos);
        if block.is_connectable(neighbor) && block.get_facing(neighbor) == self.facing {
            Neighbor::SideChain(pos, block.get_side_chain_part(neighbor))
        } else {
            Neighbor::Empty(pos)
        }
    }

    fn get_or_create_neighbor<B: SideChainPartBlock + ?Sized>(&mut self, block: &B, level: &mut ServerLevel, dir: Direction, steps: i32) -> Neighbor {
        let pos = self.center.relative_n(dir, steps);
        if let Some(&neighbor) = self.cache.get(&pos) {
            return neighbor;
        }
        let neighbor = self.create_new_neighbor(block, level, pos);
        self.cache.insert(pos, neighbor);
        neighbor
    }

    fn left<B: SideChainPartBlock + ?Sized>(&mut self, block: &B, level: &mut ServerLevel, steps: i32) -> Neighbor {
        self.get_or_create_neighbor(block, level, self.facing.clockwise(), steps)
    }

    fn right<B: SideChainPartBlock + ?Sized>(&mut self, block: &B, level: &mut ServerLevel, steps: i32) -> Neighbor {
        self.get_or_create_neighbor(block, level, self.facing.counter_clockwise(), steps)
    }
}
