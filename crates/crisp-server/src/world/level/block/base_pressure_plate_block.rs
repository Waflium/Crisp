use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::{Block, BlockState, blocks, registry, registry_id};

use super::{can_support_center, can_support_rigid_block};
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::level::LevelReader;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub const TOUCH_AABB: AABB = AABB { min_x: 0.0625, min_y: 0.0, min_z: 0.0625, max_x: 0.9375, max_y: 0.25, max_z: 0.9375 };

pub trait BasePressurePlateBlock {
    fn block(&self) -> Block;

    fn sounds(&self) -> &PlateSounds;

    fn get_pressed_time(&self) -> i32 {
        20
    }

    fn get_signal_strength(&self, level: &mut ServerLevel, pos: BlockPos) -> i32;

    fn get_signal_for_state(&self, state: BlockState) -> i32;

    fn set_signal_for_state(&self, state: BlockState, signal: i32) -> BlockState;
}

pub struct PlateSounds {
    pub click_on: i32,
    pub click_off: i32,
}

impl PlateSounds {
    pub fn from_fields(fields: &serde_json::Value) -> PlateSounds {
        let sound = |name: &str| registry_id(registry::SOUND_EVENT, fields["type"][name].as_str().unwrap_or_default());
        PlateSounds { click_on: sound("pressurePlateClickOn"), click_off: sound("pressurePlateClickOff") }
    }
}

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> BlockState {
    if direction == Direction::Down && !super::of(state).can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
}

pub fn can_survive(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let below = pos.below();
    can_support_rigid_block(level, below) || can_support_center(level, below, Direction::Up)
}

pub fn tick(plate: &(impl BasePressurePlateBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let signal = plate.get_signal_for_state(state);
    if signal > 0 {
        check_pressed(plate, level, pos, state, signal);
    }
}

pub fn entity_inside(plate: &(impl BasePressurePlateBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let signal = plate.get_signal_for_state(state);
    if signal == 0 {
        check_pressed(plate, level, pos, state, signal);
    }
}

fn check_pressed(plate: &(impl BasePressurePlateBlock + ?Sized), level: &mut ServerLevel, pos: BlockPos, state: BlockState, old_signal: i32) {
    let signal = plate.get_signal_strength(level, pos);
    let was_pressed = old_signal > 0;
    let is_pressed = signal > 0;
    if old_signal != signal {
        let new_state = plate.set_signal_for_state(state, signal);
        level.set_block(pos, new_state, UPDATE_CLIENTS);
        update_neighbours(plate.block(), level, pos);
    }
    if !is_pressed && was_pressed {
        level.play_sound(None, pos, plate.sounds().click_off, SoundSource::Blocks, 1.0, 1.0);
    } else if is_pressed && !was_pressed {
        level.play_sound(None, pos, plate.sounds().click_on, SoundSource::Blocks, 1.0, 1.0);
    }
    if is_pressed {
        level.schedule_tick(pos, plate.block(), plate.get_pressed_time());
    }
}

pub fn affect_neighbors_after_removal(
    plate: &(impl BasePressurePlateBlock + ?Sized),
    state: BlockState,
    level: &mut ServerLevel,
    pos: BlockPos,
    moved_by_piston: bool,
) {
    if !moved_by_piston && plate.get_signal_for_state(state) > 0 {
        update_neighbours(plate.block(), level, pos);
    }
}

fn update_neighbours(block: Block, level: &mut ServerLevel, pos: BlockPos) {
    level.update_neighbors_at(pos, block);
    level.update_neighbors_at(pos.below(), block);
}

pub fn get_direct_signal(plate: &(impl BasePressurePlateBlock + ?Sized), state: BlockState, direction: Direction) -> i32 {
    if direction == Direction::Up { plate.get_signal_for_state(state) } else { 0 }
}

pub fn get_entity_count(level: &ServerLevel, entity_detection_box: &AABB, living_only: bool) -> i32 {
    level.non_spectators_touching(entity_detection_box, living_only).len() as i32
}

impl ServerLevel {
    pub fn non_spectators_touching(&self, area: &AABB, living_only: bool) -> Vec<EntityId> {
        let players = self
            .players
            .values()
            .filter(|p| p.game_mode.game_type != GameType::Spectator && !p.entity.is_removed() && p.entity.bounding_box().intersects(area))
            .map(|p| p.entity.id);
        let mut found: Vec<EntityId> = players.collect();
        if !living_only {
            found.extend(self.items_in(area, None));
        }
        found
    }
}
