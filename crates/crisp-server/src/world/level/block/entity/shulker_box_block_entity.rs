use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;

pub const EVENT_SET_OPEN_COUNT: i32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimationStatus {
    Closed,
    Opening,
    Opened,
    Closing,
}

#[derive(Clone)]
pub struct ShulkerBoxBlockEntity {
    pub base: BaseContainerBlockEntity,
    pub open_count: i32,
    pub animation_status: AnimationStatus,
    pub progress: f32,
    pub progress_old: f32,
}

impl Default for ShulkerBoxBlockEntity {
    fn default() -> ShulkerBoxBlockEntity {
        ShulkerBoxBlockEntity { base: BaseContainerBlockEntity::with_size(27), open_count: 0, animation_status: AnimationStatus::Closed, progress: 0.0, progress_old: 0.0 }
    }
}

impl ShulkerBoxBlockEntity {
    pub const CONTAINER_SIZE: usize = 27;

    pub fn trigger_event(&mut self, b0: i32, b1: i32) -> bool {
        if b0 != EVENT_SET_OPEN_COUNT {
            return false;
        }
        self.open_count = b1;
        if b1 == 0 {
            self.animation_status = AnimationStatus::Closing;
        }
        if b1 == 1 {
            self.animation_status = AnimationStatus::Opening;
        }
        true
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.items = vec![ItemStack::empty(); Self::CONTAINER_SIZE];
        if !self.base.try_load_loot_table(input) {
            self.base.load_all_items(input, Self::CONTAINER_SIZE);
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        if !self.base.try_save_loot_table(output) {
            save_all_items(output, &self.base.items, false);
        }
    }

    pub fn is_closed(&self) -> bool {
        self.animation_status == AnimationStatus::Closed
    }
}

impl ServerLevel {
    fn shulker_box(&mut self, pos: BlockPos) -> Option<&mut ShulkerBoxBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::ShulkerBox(shulker) => Some(shulker),
            _ => None,
        }
    }

    pub fn shulker_box_tick(&mut self, pos: BlockPos, state: BlockState) {
        let Some(shulker) = self.shulker_box(pos) else { return };
        shulker.progress_old = shulker.progress;
        let mut updates = 0;
        match shulker.animation_status {
            AnimationStatus::Closed => shulker.progress = 0.0,
            AnimationStatus::Opening => {
                shulker.progress += 0.1;
                if shulker.progress_old == 0.0 {
                    updates += 1;
                }
                if shulker.progress >= 1.0 {
                    shulker.animation_status = AnimationStatus::Opened;
                    shulker.progress = 1.0;
                    updates += 1;
                }
            }
            AnimationStatus::Opened => shulker.progress = 1.0,
            AnimationStatus::Closing => {
                shulker.progress -= 0.1;
                if shulker.progress_old == 1.0 {
                    updates += 1;
                }
                if shulker.progress <= 0.0 {
                    shulker.animation_status = AnimationStatus::Closed;
                    shulker.progress = 0.0;
                    updates += 1;
                }
            }
        }
        for _ in 0..updates {
            self.update_neighbour_shapes(state, pos, UPDATE_ALL, crate::world::level::update_flags::UPDATE_LIMIT);
            self.update_neighbors_at(pos, state.block());
        }
    }

    pub fn shulker_box_start_open(&mut self, pos: BlockPos) {
        let Some(shulker) = self.shulker_box(pos) else { return };
        if shulker.open_count < 0 {
            shulker.open_count = 0;
        }
        shulker.open_count += 1;
        let open_count = shulker.open_count;
        let block = self.block_state(pos).block();
        self.block_event(pos, block, EVENT_SET_OPEN_COUNT, open_count);
        if open_count == 1 {
            let pitch = self.random.next_float() * 0.1 + 0.9;
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.shulker_box.open"), SoundSource::Blocks, 0.5, pitch);
        }
    }

    pub fn shulker_box_stop_open(&mut self, pos: BlockPos) {
        let Some(shulker) = self.shulker_box(pos) else { return };
        shulker.open_count -= 1;
        let open_count = shulker.open_count;
        let block = self.block_state(pos).block();
        self.block_event(pos, block, EVENT_SET_OPEN_COUNT, open_count);
        if open_count <= 0 {
            let pitch = self.random.next_float() * 0.1 + 0.9;
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.shulker_box.close"), SoundSource::Blocks, 0.5, pitch);
        }
    }
}

use crate::world::level::BlockGetter as _;
