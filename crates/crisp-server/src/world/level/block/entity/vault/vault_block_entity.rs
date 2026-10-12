use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;

use super::vault_config::{self, VaultConfig};
use super::vault_server_data::{self, VaultServerData};
use super::vault_shared_data::{self, VaultSharedData};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::BlockEntityKind;

#[derive(Clone, Debug, Default)]
pub struct VaultBlockEntity {
    server_data: VaultServerData,
    shared_data: VaultSharedData,
    config: VaultConfig,
}

impl VaultBlockEntity {
    pub fn get_update_tag(&self) -> Nbt {
        Nbt::Compound(vec![(vault_shared_data::TAG_NAME.into(), self.shared_data.encode())])
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push((vault_config::TAG_NAME.into(), self.config.encode()));
        output.push((vault_shared_data::TAG_NAME.into(), self.shared_data.encode()));
        output.push((vault_server_data::TAG_NAME.into(), self.server_data.encode()));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        if let Some(server_data) = input.get(vault_server_data::TAG_NAME).and_then(|tag| VaultServerData::decode(tag).ok()) {
            self.server_data.set(&server_data);
        }
        self.config = input.get(vault_config::TAG_NAME).and_then(|tag| VaultConfig::decode(tag).ok()).unwrap_or_default();
        if let Some(shared_data) = input.get(vault_shared_data::TAG_NAME).and_then(|tag| VaultSharedData::decode(tag).ok()) {
            self.shared_data.set(&shared_data);
        }
    }

    pub fn get_shared_data(&self) -> &VaultSharedData {
        &self.shared_data
    }

    pub fn get_config(&self) -> &VaultConfig {
        &self.config
    }
}

pub mod server {
    use crisp_core::random::RandomSource;
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::properties::{OMINOUS, VAULT_STATE, VaultState};
    use crisp_data::{BlockState, registry, registry_id};

    use super::super::vault_config::VaultConfig;
    use super::super::vault_server_data::VaultServerData;
    use super::super::vault_shared_data::VaultSharedData;
    use super::super::vault_state;
    use crate::server::level::{ServerLevel, SoundSource};
    use crate::world::entity::EntityId;
    use crate::world::entity::attributes::Attribute;
    use crate::world::item::ItemStack;
    use crate::world::level::storage::loot::loot_params::LootParams;
    use crate::world::level::storage::loot::loot_table::LootTable;
    use crate::world::level::update_flags::UPDATE_ALL;

    const UNLOCKING_DELAY_TICKS: i64 = 14;
    const DISPLAY_CYCLE_TICK_RATE: i64 = 20;
    const INSERT_FAIL_SOUND_BUFFER_TICKS: i64 = 15;

    pub fn tick(level: &mut ServerLevel, pos: BlockPos, block_state: BlockState, config: &VaultConfig, server_data: &mut VaultServerData, shared_data: &mut VaultSharedData) {
        let current_state = block_state.get(VAULT_STATE);
        if should_cycle_display_item(level.game_time, current_state) {
            cycle_display_item_from_loot_table(level, current_state, config, shared_data, pos);
        }
        let mut next_block_state = block_state;
        if level.game_time >= server_data.state_updating_resumes_at() {
            next_block_state = next_block_state.set(VAULT_STATE, vault_state::tick_and_get_next(current_state, level, pos, config, server_data, shared_data));
            if block_state != next_block_state {
                set_vault_state(level, pos, block_state, next_block_state, config, shared_data);
            }
        }
        if server_data.is_dirty || shared_data.is_dirty {
            level.block_entity_set_changed_static(pos, block_state);
            if shared_data.is_dirty {
                level.send_block_updated(pos, block_state, next_block_state);
            }
            server_data.is_dirty = false;
            shared_data.is_dirty = false;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_insert_key(
        level: &mut ServerLevel,
        pos: BlockPos,
        block_state: BlockState,
        config: &VaultConfig,
        server_data: &mut VaultServerData,
        shared_data: &mut VaultSharedData,
        player: EntityId,
        stack_to_insert: &mut ItemStack,
    ) {
        let vault_state = block_state.get(VAULT_STATE);
        if can_eject_reward(config, vault_state) {
            let uuid = level.player(player).entity.uuid;
            if !is_valid_to_insert(config, stack_to_insert) {
                play_insert_fail_sound(level, server_data, pos, "minecraft:block.vault.insert_item_fail");
            } else if server_data.has_rewarded_player(uuid) {
                play_insert_fail_sound(level, server_data, pos, "minecraft:block.vault.reject_rewarded_player");
            } else {
                let items_to_eject = resolve_items_to_eject(level, config, pos, player, stack_to_insert);
                if !items_to_eject.is_empty() {
                    level.award_stat(player, crate::stats::stats::ITEM_USED.get(stack_to_insert.get_item().0), 1);
                    stack_to_insert.consume(config.key_item.count, level, Some(player));
                    unlock(level, block_state, pos, config, server_data, shared_data, items_to_eject);
                    server_data.add_to_rewarded_players(uuid);
                    shared_data.update_connected_players_within_range(level, pos, server_data, config, config.deactivation_range);
                }
            }
        }
    }

    fn set_vault_state(level: &mut ServerLevel, pos: BlockPos, current_block_state: BlockState, new_block_state: BlockState, config: &VaultConfig, shared_data: &mut VaultSharedData) {
        let current_vault_state = current_block_state.get(VAULT_STATE);
        let new_vault_state = new_block_state.get(VAULT_STATE);
        level.set_block(pos, new_block_state, UPDATE_ALL);
        vault_state::on_transition(current_vault_state, level, pos, new_vault_state, config, shared_data, new_block_state.get(OMINOUS));
    }

    pub fn cycle_display_item_from_loot_table(level: &mut ServerLevel, vault_state: VaultState, config: &VaultConfig, shared_data: &mut VaultSharedData, pos: BlockPos) {
        if !can_eject_reward(config, vault_state) {
            shared_data.set_display_item(&ItemStack::empty());
        } else {
            let display_item = get_random_display_item_from_loot_table(level, pos, config.override_loot_table_to_display.as_deref().unwrap_or(&config.loot_table));
            shared_data.set_display_item(&display_item);
        }
    }

    fn get_random_display_item_from_loot_table(level: &mut ServerLevel, pos: BlockPos, loot_table_id: &str) -> ItemStack {
        let loot_table = LootTable::get(loot_table_id);
        let mut params = LootParams::new(level);
        params.origin = Some(Vec3::at_center_of(pos));
        let mut results = loot_table.get_random_items_with_level_random(params);
        if results.is_empty() {
            return ItemStack::empty();
        }
        let index = level.random.next_int_bounded(results.len() as i32) as usize;
        results.swap_remove(index)
    }

    fn unlock(
        level: &mut ServerLevel,
        block_state: BlockState,
        pos: BlockPos,
        config: &VaultConfig,
        server_data: &mut VaultServerData,
        shared_data: &mut VaultSharedData,
        items_to_eject: Vec<ItemStack>,
    ) {
        server_data.set_items_to_eject(items_to_eject);
        shared_data.set_display_item(&server_data.get_next_item_to_eject());
        server_data.pause_state_updating_until(level.game_time + UNLOCKING_DELAY_TICKS);
        set_vault_state(level, pos, block_state, block_state.set(VAULT_STATE, VaultState::Unlocking), config, shared_data);
    }

    fn resolve_items_to_eject(level: &mut ServerLevel, config: &VaultConfig, pos: BlockPos, player: EntityId, inserted_stack: &ItemStack) -> Vec<ItemStack> {
        let loot_table = LootTable::get(&config.loot_table);
        let luck = level.attribute_value(player, Attribute::Luck) as f32;
        let mut params = LootParams::new(level);
        params.origin = Some(Vec3::at_center_of(pos));
        params.luck = luck;
        params.this_entity = Some(player);
        params.tool = Some(inserted_stack);
        loot_table.get_random_items(params)
    }

    fn can_eject_reward(config: &VaultConfig, vault_state: VaultState) -> bool {
        !config.key_item.is_empty() && vault_state != VaultState::Inactive
    }

    fn is_valid_to_insert(config: &VaultConfig, stack_to_insert: &ItemStack) -> bool {
        ItemStack::is_same_item_same_components(stack_to_insert, &config.key_item) && stack_to_insert.count >= config.key_item.count
    }

    fn should_cycle_display_item(game_time: i64, vault_state: VaultState) -> bool {
        game_time % DISPLAY_CYCLE_TICK_RATE == 0 && vault_state == VaultState::Active
    }

    fn play_insert_fail_sound(level: &mut ServerLevel, server_data: &mut VaultServerData, pos: BlockPos, sound: &str) {
        if level.game_time >= server_data.get_last_insert_fail_timestamp() + INSERT_FAIL_SOUND_BUFFER_TICKS {
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, sound), SoundSource::Blocks, 1.0, 1.0);
            server_data.set_last_insert_fail_timestamp(level.game_time);
        }
    }
}

impl ServerLevel {
    fn vault(&mut self, pos: BlockPos) -> Option<&mut VaultBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Vault(vault) => Some(vault),
            _ => None,
        }
    }

    fn vault_with(&mut self, pos: BlockPos, f: impl FnOnce(&mut ServerLevel, &VaultConfig, &mut VaultServerData, &mut VaultSharedData)) {
        let Some(vault) = self.vault(pos) else { return };
        let mut taken = std::mem::take(vault);
        f(self, &taken.config, &mut taken.server_data, &mut taken.shared_data);
        if let Some(vault) = self.vault(pos) {
            *vault = taken;
        }
    }

    pub fn vault_server_tick(&mut self, pos: BlockPos, state: BlockState) {
        self.vault_with(pos, |level, config, server_data, shared_data| server::tick(level, pos, state, config, server_data, shared_data));
    }

    pub fn vault_try_insert_key(&mut self, pos: BlockPos, state: BlockState, player: EntityId, stack_to_insert: &mut ItemStack) {
        self.vault_with(pos, |level, config, server_data, shared_data| server::try_insert_key(level, pos, state, config, server_data, shared_data, player, stack_to_insert));
    }
}
