use crisp_core::{BlockPos, ChunkPos, Direction};
use crisp_data::{BlockState, blocks};

use super::light_engine::{LightEngine, PULL_LIGHT_IN_ENTRY, Propagator, opacity, queue_entry, state_at};
use super::{LightChunkGetter, is_empty_shape, node, shape_occludes};
use crate::world::level::light_layer::LightLayer;

pub struct BlockLightEngine {
    pub base: LightEngine,
}

impl BlockLightEngine {
    pub fn new() -> BlockLightEngine {
        BlockLightEngine { base: LightEngine::new(LightLayer::Block) }
    }

    fn emission(&self, block_node: i64, state: BlockState) -> i32 {
        let emission = state.light_emission() as i32;
        if emission > 0 && self.base.storage.light_on_in_section(node::to_section(block_node)) { emission } else { 0 }
    }

    pub fn propagate_light_sources<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, pos: ChunkPos) {
        self.base.set_light_enabled(pos, true);
        let Some(chunk) = chunks.chunk_for_lighting(pos.x, pos.z) else { return };
        for (index, section) in chunk.sections.iter().enumerate() {
            if !section.states.maybe_has(|s: BlockState| s.light_emission() != 0) {
                continue;
            }
            let origin_y = chunk.min_y() + index as i32 * 16;
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let state = section.block_state(x, y, z);
                        if state.light_emission() != 0 {
                            let pos = BlockPos::new(pos.min_block_x() + x as i32, origin_y + y as i32, pos.min_block_z() + z as i32);
                            let data = queue_entry::increase_light_from_emission(state.light_emission() as i32, is_empty_shape(state));
                            self.base.enqueue_increase(pos.as_long(), data);
                        }
                    }
                }
            }
        }
    }
}

impl Propagator for BlockLightEngine {
    fn base(&mut self) -> &mut LightEngine {
        &mut self.base
    }

    fn check_node<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, block_node: i64) {
        if !self.base.storage.storing_light_for_section(node::to_section(block_node)) {
            return;
        }
        let state = state_at(chunks, block_node);
        let light_emission = self.emission(block_node, state);
        let old_level = self.base.storage.stored_level(block_node) as i32;
        if light_emission < old_level {
            self.base.storage.set_stored_level(block_node, 0);
            self.base.enqueue_decrease(block_node, queue_entry::decrease_all_directions(old_level));
        } else {
            self.base.enqueue_decrease(block_node, PULL_LIGHT_IN_ENTRY);
        }
        if light_emission > 0 {
            self.base.enqueue_increase(block_node, queue_entry::increase_light_from_emission(light_emission, is_empty_shape(state)));
        }
    }

    fn propagate_increase<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, from_node: i64, increase_data: i64, from_level: i32) {
        let mut from_state = None;
        for direction in Direction::VALUES {
            if !queue_entry::should_propagate_in_direction(increase_data, direction) {
                continue;
            }
            let to_node = node::offset(from_node, direction);
            if !self.base.storage.storing_light_for_section(node::to_section(to_node)) {
                continue;
            }
            let to_level = self.base.storage.stored_level(to_node) as i32;
            if from_level - 1 <= to_level {
                continue;
            }
            let to_state = state_at(chunks, to_node);
            let new_to_level = from_level - opacity(to_state);
            if new_to_level <= to_level {
                continue;
            }
            let from_state = *from_state.get_or_insert_with(|| {
                if queue_entry::is_from_empty_shape(increase_data) { blocks::AIR.default_state() } else { state_at(chunks, from_node) }
            });
            if !shape_occludes(from_state, to_state, direction) {
                self.base.storage.set_stored_level(to_node, new_to_level as u8);
                if new_to_level > 1 {
                    let data = queue_entry::increase_skip_one_direction(new_to_level, is_empty_shape(to_state), direction.opposite());
                    self.base.enqueue_increase(to_node, data);
                }
            }
        }
    }

    fn propagate_decrease<C: LightChunkGetter + ?Sized>(&mut self, chunks: &C, from_node: i64, decrease_data: i64) {
        let old_from_level = queue_entry::from_level(decrease_data);
        for direction in Direction::VALUES {
            if !queue_entry::should_propagate_in_direction(decrease_data, direction) {
                continue;
            }
            let to_node = node::offset(from_node, direction);
            if !self.base.storage.storing_light_for_section(node::to_section(to_node)) {
                continue;
            }
            let to_level = self.base.storage.stored_level(to_node) as i32;
            if to_level == 0 {
                continue;
            }
            if to_level < old_from_level {
                let to_state = state_at(chunks, to_node);
                let to_emission = self.emission(to_node, to_state);
                self.base.storage.set_stored_level(to_node, 0);
                if to_emission < to_level {
                    self.base.enqueue_decrease(to_node, queue_entry::decrease_skip_one_direction(to_level, direction.opposite()));
                }
                if to_emission > 0 {
                    self.base.enqueue_increase(to_node, queue_entry::increase_light_from_emission(to_emission, is_empty_shape(to_state)));
                }
            } else {
                self.base.enqueue_increase(to_node, queue_entry::increase_only_one_direction(to_level, false, direction.opposite()));
            }
        }
    }
}
