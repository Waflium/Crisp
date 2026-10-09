use std::sync::Arc;

use super::{NodeMap, NodeSet, node, section};
use crate::world::level::chunk::data_layer::DataLayer;
use crate::world::level::light_layer::LightLayer;

#[derive(Clone)]
pub struct DataLayerStorageMap {
    map: NodeMap<Arc<DataLayer>>,
    top_sections: NodeMap<i32>,
    current_lowest_y: i32,
}

impl DataLayerStorageMap {
    fn new() -> DataLayerStorageMap {
        DataLayerStorageMap { map: NodeMap::default(), top_sections: NodeMap::default(), current_lowest_y: i32::MAX }
    }

    pub fn layer(&self, section: i64) -> Option<&DataLayer> {
        self.map.get(&section).map(|l| &**l)
    }

    fn top_section(&self, zero_node: i64) -> i32 {
        self.top_sections.get(&zero_node).copied().unwrap_or(self.current_lowest_y)
    }
}

mod section_state {
    pub const HAS_DATA_BIT: u8 = 32;
    pub const NEIGHBOR_COUNT_BITS: u8 = 31;

    pub fn with_data(state: u8, has_data: bool) -> u8 {
        if has_data { state | HAS_DATA_BIT } else { state & !HAS_DATA_BIT }
    }

    pub fn with_neighbor_count(state: u8, count: i32) -> u8 {
        debug_assert!((0..=26).contains(&count), "Neighbor count was not within range [0; 26]");
        state & !NEIGHBOR_COUNT_BITS | count as u8 & NEIGHBOR_COUNT_BITS
    }

    pub fn neighbor_count(state: u8) -> i32 {
        (state & NEIGHBOR_COUNT_BITS) as i32
    }
}

pub struct LayerLightSectionStorage {
    layer: LightLayer,
    section_states: NodeMap<u8>,
    columns_with_sources: NodeSet,
    pub(super) visible: DataLayerStorageMap,
    pub(super) updating: DataLayerStorageMap,
    changed_sections: NodeSet,
    sections_affected_by_light_updates: NodeSet,
    queued_sections: NodeMap<Arc<DataLayer>>,
    columns_to_retain_queued_data_for: NodeSet,
    to_remove: NodeSet,
    has_inconsistencies: bool,
}

impl LayerLightSectionStorage {
    pub fn new(layer: LightLayer) -> LayerLightSectionStorage {
        LayerLightSectionStorage {
            layer,
            section_states: NodeMap::default(),
            columns_with_sources: NodeSet::default(),
            visible: DataLayerStorageMap::new(),
            updating: DataLayerStorageMap::new(),
            changed_sections: NodeSet::default(),
            sections_affected_by_light_updates: NodeSet::default(),
            queued_sections: NodeMap::default(),
            columns_to_retain_queued_data_for: NodeSet::default(),
            to_remove: NodeSet::default(),
            has_inconsistencies: false,
        }
    }

    fn is_sky(&self) -> bool {
        self.layer == LightLayer::Sky
    }

    pub fn storing_light_for_section(&self, section: i64) -> bool {
        self.updating.map.contains_key(&section)
    }

    pub fn data_layer_to_write(&mut self, section: i64) -> Option<&mut DataLayer> {
        let layer = self.updating.map.get_mut(&section)?;
        if self.changed_sections.insert(section) {
            *layer = Arc::new(layer.copy());
        }
        Some(Arc::make_mut(layer))
    }

    pub fn data_layer_data(&self, section: i64) -> Option<&DataLayer> {
        match self.queued_sections.get(&section) {
            Some(layer) => Some(layer),
            None => self.visible.layer(section),
        }
    }

    pub fn stored_level(&self, block_node: i64) -> u8 {
        let section = node::to_section(block_node);
        self.updating.layer(section).map_or(0, |layer| layer.get(node::x(block_node) & 15, node::y(block_node) & 15, node::z(block_node) & 15))
    }

    pub fn set_stored_level(&mut self, block_node: i64, level: u8) {
        let section = node::to_section(block_node);
        let Some(layer) = self.updating.map.get_mut(&section) else { return };
        if self.changed_sections.insert(section) {
            *layer = Arc::new(layer.copy());
        }
        Arc::make_mut(layer).set(node::x(block_node) & 15, node::y(block_node) & 15, node::z(block_node) & 15, level);
        around_and_at_block_pos(block_node, |s| {
            self.sections_affected_by_light_updates.insert(s);
        });
    }

    fn mark_section_and_neighbors_as_affected(&mut self, section_node: i64) {
        let (x, y, z) = (section::x(section_node), section::y(section_node), section::z(section_node));
        for offset_z in -1..=1 {
            for offset_x in -1..=1 {
                for offset_y in -1..=1 {
                    self.sections_affected_by_light_updates.insert(section::of(x + offset_x, y + offset_y, z + offset_z));
                }
            }
        }
    }

    fn create_data_layer(&self, section_node: i64) -> Arc<DataLayer> {
        if let Some(queued) = self.queued_sections.get(&section_node) {
            return queued.clone();
        }
        if !self.is_sky() {
            return Arc::new(DataLayer::new());
        }
        let top_section = self.updating.top_section(section::zero_node_of(section_node));
        if top_section != self.updating.current_lowest_y && section::y(section_node) < top_section {
            let mut above = section::offset(section_node, 0, 1, 0);
            loop {
                if let Some(data) = self.updating.layer(above) {
                    return Arc::new(data.repeat_first_layer());
                }
                above = section::offset(above, 0, 1, 0);
            }
        }
        Arc::new(if self.light_on_in_section(section_node) { DataLayer::filled(15) } else { DataLayer::new() })
    }

    pub fn has_inconsistencies(&self) -> bool {
        self.has_inconsistencies
    }

    pub fn mark_new_inconsistencies(&mut self) {
        if !self.has_inconsistencies {
            return;
        }
        self.has_inconsistencies = false;
        let to_remove: Vec<i64> = self.to_remove.iter().copied().collect();
        for &node in &to_remove {
            let queued = self.queued_sections.remove(&node);
            let stored = self.updating.map.remove(&node);
            if self.columns_to_retain_queued_data_for.contains(&section::zero_node_of(node)) {
                if let Some(queued) = queued {
                    self.queued_sections.insert(node, queued);
                } else if let Some(stored) = stored {
                    self.queued_sections.insert(node, stored);
                }
            }
        }
        for &node in &to_remove {
            self.on_node_removed(node);
            self.changed_sections.insert(node);
        }
        self.to_remove.clear();
        let (updating, changed) = (&mut self.updating, &mut self.changed_sections);
        self.queued_sections.retain(|&section_node, data| {
            let Some(layer) = updating.map.get_mut(&section_node) else { return true };
            if layer.id() != data.id() {
                *layer = data.clone();
                changed.insert(section_node);
            }
            false
        });
    }

    fn on_node_added(&mut self, section_node: i64) {
        if !self.is_sky() {
            return;
        }
        let y = section::y(section_node);
        if self.updating.current_lowest_y > y {
            self.updating.current_lowest_y = y;
        }
        let zero_node = section::zero_node_of(section_node);
        if self.updating.top_section(zero_node) < y + 1 {
            self.updating.top_sections.insert(zero_node, y + 1);
        }
    }

    fn on_node_removed(&mut self, section_node: i64) {
        if !self.is_sky() {
            return;
        }
        let zero_node = section::zero_node_of(section_node);
        let mut y = section::y(section_node);
        if self.updating.top_section(zero_node) == y + 1 {
            let mut new_top = section_node;
            while !self.storing_light_for_section(new_top) && self.has_light_data_at_or_below(y) {
                y -= 1;
                new_top = section::offset(new_top, 0, -1, 0);
            }
            if self.storing_light_for_section(new_top) {
                self.updating.top_sections.insert(zero_node, y + 1);
            } else {
                self.updating.top_sections.remove(&zero_node);
            }
        }
    }

    pub fn set_light_enabled(&mut self, zero_node: i64, enable: bool) {
        if enable {
            self.columns_with_sources.insert(zero_node);
        } else {
            self.columns_with_sources.remove(&zero_node);
        }
    }

    pub fn light_on_in_section(&self, section_node: i64) -> bool {
        self.columns_with_sources.contains(&section::zero_node_of(section_node))
    }

    pub fn retain_data(&mut self, zero_node: i64, retain: bool) {
        if retain {
            self.columns_to_retain_queued_data_for.insert(zero_node);
        } else {
            self.columns_to_retain_queued_data_for.remove(&zero_node);
        }
    }

    pub fn queue_section_data(&mut self, section_node: i64, data: Option<DataLayer>) {
        match data {
            Some(data) => {
                self.queued_sections.insert(section_node, Arc::new(data));
                self.has_inconsistencies = true;
            }
            None => {
                self.queued_sections.remove(&section_node);
            }
        }
    }

    pub fn update_section_status(&mut self, section_node: i64, section_empty: bool) {
        let state = self.section_states.get(&section_node).copied().unwrap_or(0);
        let new_state = section_state::with_data(state, !section_empty);
        if state == new_state {
            return;
        }
        self.put_section_state(section_node, new_state);
        let increment = if section_empty { -1 } else { 1 };
        for offset_x in -1..=1 {
            for offset_y in -1..=1 {
                for offset_z in -1..=1 {
                    if offset_x != 0 || offset_y != 0 || offset_z != 0 {
                        let neighbor = section::offset(section_node, offset_x, offset_y, offset_z);
                        let neighbor_state = self.section_states.get(&neighbor).copied().unwrap_or(0);
                        let count = section_state::neighbor_count(neighbor_state) + increment;
                        self.put_section_state(neighbor, section_state::with_neighbor_count(neighbor_state, count));
                    }
                }
            }
        }
    }

    fn put_section_state(&mut self, section_node: i64, state: u8) {
        if state != 0 {
            if self.section_states.insert(section_node, state).unwrap_or(0) == 0 {
                self.initialize_section(section_node);
            }
        } else if self.section_states.remove(&section_node).unwrap_or(0) != 0 {
            self.to_remove.insert(section_node);
            self.has_inconsistencies = true;
        }
    }

    fn initialize_section(&mut self, section_node: i64) {
        if !self.to_remove.remove(&section_node) {
            let layer = self.create_data_layer(section_node);
            self.updating.map.insert(section_node, layer);
            self.changed_sections.insert(section_node);
            self.on_node_added(section_node);
            self.mark_section_and_neighbors_as_affected(section_node);
            self.has_inconsistencies = true;
        }
    }

    pub fn swap_section_map(&mut self, changes: &mut Vec<(LightLayer, i64)>) {
        if !self.changed_sections.is_empty() {
            self.visible = self.updating.clone();
            self.changed_sections.clear();
        }
        for section_node in self.sections_affected_by_light_updates.drain() {
            changes.push((self.layer, section_node));
        }
    }

    pub fn block_light_value(&self, block_node: i64) -> u8 {
        self.visible.layer(node::to_section(block_node)).map_or(0, |l| l.get(node::x(block_node) & 15, node::y(block_node) & 15, node::z(block_node) & 15))
    }

    pub fn sky_light_value(&self, mut block_node: i64) -> u8 {
        let mut section_node = node::to_section(block_node);
        let mut section_y = section::y(section_node);
        let sections = &self.visible;
        let top_section = sections.top_section(section::zero_node_of(section_node));
        if top_section != sections.current_lowest_y && section_y < top_section {
            let mut layer = sections.layer(section_node);
            if layer.is_none() {
                block_node = node::flat_index(block_node);
                while layer.is_none() {
                    section_y += 1;
                    if section_y >= top_section {
                        return 15;
                    }
                    section_node = section::offset(section_node, 0, 1, 0);
                    layer = sections.layer(section_node);
                }
            }
            layer.unwrap().get(node::x(block_node) & 15, node::y(block_node) & 15, node::z(block_node) & 15)
        } else {
            15
        }
    }

    pub fn has_light_data_at_or_below(&self, section_y: i32) -> bool {
        section_y >= self.updating.current_lowest_y
    }

    pub fn is_above_data(&self, section_node: i64) -> bool {
        let top_section = self.updating.top_section(section::zero_node_of(section_node));
        top_section == self.updating.current_lowest_y || section::y(section_node) >= top_section
    }

    pub fn top_section_y(&self, zero_node: i64) -> i32 {
        self.updating.top_section(zero_node)
    }

    pub fn bottom_section_y(&self) -> i32 {
        self.updating.current_lowest_y
    }
}

fn around_and_at_block_pos(block_node: i64, mut f: impl FnMut(i64)) {
    let (x, y, z) = (node::x(block_node), node::y(block_node), node::z(block_node));
    let (min_x, max_x) = ((x - 1) >> 4, (x + 1) >> 4);
    let (min_y, max_y) = ((y - 1) >> 4, (y + 1) >> 4);
    let (min_z, max_z) = ((z - 1) >> 4, (z + 1) >> 4);
    if min_x == max_x && min_y == max_y && min_z == max_z {
        f(section::of(min_x, min_y, min_z));
    } else {
        for sx in min_x..=max_x {
            for sy in min_y..=max_y {
                for sz in min_z..=max_z {
                    f(section::of(sx, sy, sz));
                }
            }
        }
    }
}
