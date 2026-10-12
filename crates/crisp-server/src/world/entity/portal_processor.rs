use crisp_core::BlockPos;

use crate::world::level::block::portal::{Portal, Transition};

#[derive(Clone, Copy, Debug)]
pub struct PortalProcessor {
    portal: Portal,
    entry_position: BlockPos,
    portal_time: i32,
    inside_portal_this_tick: bool,
}

impl PortalProcessor {
    pub fn new(portal: Portal, portal_entry_position: BlockPos) -> PortalProcessor {
        PortalProcessor { portal, entry_position: portal_entry_position, portal_time: 0, inside_portal_this_tick: true }
    }

    pub fn process_portal_teleportation(&mut self, allowed_to_teleport: bool, portal_transition_time: impl FnOnce() -> i32) -> bool {
        if !self.inside_portal_this_tick {
            self.decay_tick();
            return false;
        }
        self.inside_portal_this_tick = false;
        allowed_to_teleport && {
            let portal_time = self.portal_time;
            self.portal_time += 1;
            portal_time >= portal_transition_time()
        }
    }

    pub fn portal(&self) -> Portal {
        self.portal
    }

    pub fn get_portal_local_transition(&self) -> Transition {
        self.portal.get_local_transition()
    }

    fn decay_tick(&mut self) {
        self.portal_time = (self.portal_time - 4).max(0);
    }

    pub fn has_expired(&self) -> bool {
        self.portal_time <= 0
    }

    pub fn get_entry_position(&self) -> BlockPos {
        self.entry_position
    }

    pub fn update_entry_position(&mut self, entry_position: BlockPos) {
        self.entry_position = entry_position;
    }

    pub fn get_portal_time(&self) -> i32 {
        self.portal_time
    }

    pub fn is_inside_portal_this_tick(&self) -> bool {
        self.inside_portal_this_tick
    }

    pub fn set_as_inside_portal_this_tick(&mut self, inside_portal: bool) {
        self.inside_portal_this_tick = inside_portal;
    }

    pub fn is_same_portal(&self, portal: Portal) -> bool {
        self.portal == portal
    }
}
