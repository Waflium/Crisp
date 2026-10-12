use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::VaultState;
use crisp_data::{registry, registry_id};

use super::vault_block_entity::server;
use super::vault_config::VaultConfig;
use super::vault_server_data::VaultServerData;
use super::vault_shared_data::VaultSharedData;
use crate::core::dispenser::default_dispense_item_behavior::spawn_item;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::ItemStack;

const UPDATE_CONNECTED_PLAYERS_TICK_RATE: i64 = 20;
const DELAY_BETWEEN_EJECTIONS_TICKS: i64 = 20;
const DELAY_AFTER_LAST_EJECTION_TICKS: i64 = 20;
const DELAY_BEFORE_FIRST_EJECTION_TICKS: i64 = 20;

fn play_sound(level: &mut ServerLevel, pos: BlockPos, sound: &str) {
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, sound), SoundSource::Blocks, 1.0, 1.0);
}

fn on_enter(state: VaultState, level: &mut ServerLevel, pos: BlockPos, config: &VaultConfig, shared_data: &mut VaultSharedData, is_ominous: bool) {
    match state {
        VaultState::Inactive => {
            shared_data.set_display_item(&ItemStack::empty());
            level.level_event(None, 3016, pos, if is_ominous { 1 } else { 0 });
        }
        VaultState::Active => {
            if !shared_data.has_display_item() {
                server::cycle_display_item_from_loot_table(level, state, config, shared_data, pos);
            }
            level.level_event(None, 3015, pos, if is_ominous { 1 } else { 0 });
        }
        VaultState::Unlocking => play_sound(level, pos, "minecraft:block.vault.insert_item"),
        VaultState::Ejecting => play_sound(level, pos, "minecraft:block.vault.open_shutter"),
    }
}

fn on_exit(state: VaultState, level: &mut ServerLevel, pos: BlockPos) {
    if state == VaultState::Ejecting {
        play_sound(level, pos, "minecraft:block.vault.close_shutter");
    }
}

pub fn tick_and_get_next(
    state: VaultState,
    level: &mut ServerLevel,
    pos: BlockPos,
    config: &VaultConfig,
    server_data: &mut VaultServerData,
    shared_data: &mut VaultSharedData,
) -> VaultState {
    match state {
        VaultState::Inactive => update_state_for_connected_players(level, pos, config, server_data, shared_data, config.activation_range),
        VaultState::Active => update_state_for_connected_players(level, pos, config, server_data, shared_data, config.deactivation_range),
        VaultState::Unlocking => {
            server_data.pause_state_updating_until(level.game_time + DELAY_BEFORE_FIRST_EJECTION_TICKS);
            VaultState::Ejecting
        }
        VaultState::Ejecting => {
            if server_data.get_items_to_eject().is_empty() {
                server_data.mark_ejection_finished();
                update_state_for_connected_players(level, pos, config, server_data, shared_data, config.deactivation_range)
            } else {
                let ejection_sound_progress = server_data.ejection_progress();
                let item_to_eject = server_data.pop_next_item_to_eject();
                eject_result_item(level, pos, item_to_eject, ejection_sound_progress);
                shared_data.set_display_item(&server_data.get_next_item_to_eject());
                let is_last_ejection = server_data.get_items_to_eject().is_empty();
                let ejection_delay = if is_last_ejection { DELAY_AFTER_LAST_EJECTION_TICKS } else { DELAY_BETWEEN_EJECTIONS_TICKS };
                server_data.pause_state_updating_until(level.game_time + ejection_delay);
                VaultState::Ejecting
            }
        }
    }
}

fn update_state_for_connected_players(
    level: &mut ServerLevel,
    pos: BlockPos,
    config: &VaultConfig,
    server_data: &mut VaultServerData,
    shared_data: &mut VaultSharedData,
    activation_range: f64,
) -> VaultState {
    shared_data.update_connected_players_within_range(level, pos, server_data, config, activation_range);
    server_data.pause_state_updating_until(level.game_time + UPDATE_CONNECTED_PLAYERS_TICK_RATE);
    if shared_data.has_connected_players() { VaultState::Active } else { VaultState::Inactive }
}

pub fn on_transition(state: VaultState, level: &mut ServerLevel, pos: BlockPos, to: VaultState, config: &VaultConfig, shared_data: &mut VaultSharedData, is_ominous: bool) {
    on_exit(state, level, pos);
    on_enter(to, level, pos, config, shared_data, is_ominous);
}

fn eject_result_item(level: &mut ServerLevel, pos: BlockPos, item_to_eject: ItemStack, ejection_sound_progress: f32) {
    spawn_item(level, item_to_eject, 2, Direction::Up, Vec3::at_bottom_center_of(pos).relative(Direction::Up, 1.2));
    level.level_event(None, 3017, pos, 0);
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.vault.eject_item"), SoundSource::Blocks, 1.0, 0.8 + 0.4 * ejection_sound_progress);
}
