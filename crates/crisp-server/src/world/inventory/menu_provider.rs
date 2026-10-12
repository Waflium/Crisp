use crisp_core::{BlockPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_data::{block_entity_types as types, registry, registry_id};

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind, menu_type_id};
use super::inventory_menu::CONTAINER_ID;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::crafting::recipe_type::RecipeType;
use crate::world::level::block::chest_block;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuProvider {
    BlockEntity(BlockPos),
    DoubleChest { first: BlockPos, second: BlockPos },
    EnderChest,
    Crafting(BlockPos),
    Stonecutter(BlockPos),
    Smithing(BlockPos),
    Anvil(BlockPos),
}

fn menu_kind(entity: &BlockEntity) -> Option<MenuKind> {
    Some(match entity.ty() {
        types::CHEST | types::TRAPPED_CHEST | types::BARREL => MenuKind::Chest { rows: 3 },
        types::SHULKER_BOX => MenuKind::ShulkerBox,
        types::FURNACE => MenuKind::Furnace(RecipeType::Smelting),
        types::BLAST_FURNACE => MenuKind::Furnace(RecipeType::Blasting),
        types::SMOKER => MenuKind::Furnace(RecipeType::Smoking),
        types::HOPPER => MenuKind::Hopper,
        types::DISPENSER | types::DROPPER => MenuKind::Dispenser,
        types::BREWING_STAND => MenuKind::BrewingStand,
        types::LECTERN => MenuKind::Lectern,
        types::CRAFTER => MenuKind::Crafter,
        _ => return None,
    })
}

impl ServerLevel {
    fn can_open(&mut self, pos: BlockPos, player: EntityId) -> bool {
        let spectator = self.is_spectator(player);
        let held = self.player(player).inventory.held().cloned().unwrap_or_else(crate::world::item::ItemStack::empty);
        let Some(entity) = self.get_block_entity(pos) else { return false };
        let randomizable = entity.is_randomizable();
        let Some(base) = entity.container_base() else { return false };
        let loot_ok = !randomizable || base.loot_table.is_none() || !spectator;
        let unlocks = match &base.lock_key {
            None => true,
            Some(lock) => spectator || ItemPredicate::decode(lock).map_or(true, |predicate| predicate.test(&held)),
        };
        loot_ok && unlocks
    }

    fn send_chest_locked_notifications(&mut self, at: Vec3, player: EntityId, display_name: Component) {
        let message = Component::translatable_with("container.isLocked", vec![Arg::Component(display_name)]);
        self.player(player).connection.send(clientbound::SYSTEM_CHAT, |p| {
            message.write(p);
            p.bool(true);
        });
        self.play_sound_at(None, at.x, at.y, at.z, registry_id(registry::SOUND_EVENT, "minecraft:block.chest.locked"), SoundSource::Blocks, 1.0, 1.0);
    }

    fn double_chest_name(&mut self, first: BlockPos, second: BlockPos) -> Component {
        for pos in [first, second] {
            if let Some(entity) = self.get_block_entity(pos)
                && entity.has_custom_name()
            {
                return entity.name();
            }
        }
        Component::translatable("container.chestDouble")
    }

    fn create_menu(&mut self, provider: MenuProvider, container_id: i32, player: EntityId) -> Option<(AbstractContainerMenu, Component)> {
        let spectator = self.is_spectator(player);
        match provider {
            MenuProvider::BlockEntity(pos) => {
                let entity = self.get_block_entity(pos)?;
                if let BlockEntityKind::Beacon(beacon) = &entity.kind {
                    let lock_key = beacon.get_lock_key().cloned();
                    let display_name = entity.name();
                    let held = self.player(player).inventory.held().cloned().unwrap_or_else(crate::world::item::ItemStack::empty);
                    let can_unlock = spectator || lock_key.as_ref().is_none_or(|lock| ItemPredicate::decode(lock).map_or(true, |predicate| predicate.test(&held)));
                    if can_unlock {
                        return Some((AbstractContainerMenu::beacon(container_id, pos), display_name));
                    }
                    self.send_chest_locked_notifications(Vec3::at_center_of(pos), player, display_name);
                    return None;
                }
                if let BlockEntityKind::EnchantingTable(_) = &entity.kind {
                    let title = entity.name();
                    let enchantment_seed = self.player(player).experience.enchantment_seed;
                    return Some((AbstractContainerMenu::enchantment(container_id, pos, enchantment_seed), title));
                }
                let kind = menu_kind(entity)?;
                if kind == MenuKind::Lectern {
                    let container = ContainerRef::lectern_book_access(self, pos);
                    return Some((AbstractContainerMenu::new(container_id, kind, Some(container)), Component::translatable("container.lectern")));
                }
                let (name, randomizable) = (entity.name(), entity.is_randomizable());
                if !self.can_open(pos, player) {
                    if !(randomizable && spectator) {
                        self.send_chest_locked_notifications(Vec3::at_center_of(pos), player, name);
                    }
                    return None;
                }
                self.unpack_loot_table(pos, Some(player));
                let container = ContainerRef::block(self, pos);
                self.container_start_open(container, player);
                let mut menu = AbstractContainerMenu::new(container_id, kind, Some(container));
                if kind == MenuKind::Crafter {
                    self.crafter_menu_refresh_recipe_result(&mut menu);
                }
                Some((menu, name))
            }
            MenuProvider::EnderChest => {
                let container = ContainerRef::PlayerEnderChest(player);
                self.container_start_open(container, player);
                Some((AbstractContainerMenu::new(container_id, MenuKind::Chest { rows: 3 }, Some(container)), Component::translatable("container.enderchest")))
            }
            MenuProvider::Crafting(pos) => Some((AbstractContainerMenu::crafting(container_id, pos), Component::translatable("container.crafting"))),
            MenuProvider::Stonecutter(pos) => Some((AbstractContainerMenu::stonecutter(container_id, pos), Component::translatable("container.stonecutter"))),
            MenuProvider::Smithing(pos) => Some((AbstractContainerMenu::smithing(container_id, pos), Component::translatable("container.upgrade"))),
            MenuProvider::Anvil(pos) => Some((AbstractContainerMenu::anvil(container_id, pos), Component::translatable("container.repair"))),
            MenuProvider::DoubleChest { first, second } => {
                let name = self.double_chest_name(first, second);
                if !(self.can_open(first, player) && self.can_open(second, player)) {
                    if !spectator {
                        let state = crate::world::level::BlockGetter::block_state(self, first);
                        let direction = chest_block::get_connected_direction(state);
                        let center = Vec3::at_center_of(first).add(direction.step_x() as f64 / 2.0, 0.0, direction.step_z() as f64 / 2.0);
                        self.send_chest_locked_notifications(center, player, name);
                    }
                    return None;
                }
                self.unpack_loot_table(first, Some(player));
                self.unpack_loot_table(second, Some(player));
                let (a, b) = (ContainerRef::block(self, first), ContainerRef::block(self, second));
                let (ContainerRef::Block { pos: p1, id: i1 }, ContainerRef::Block { pos: p2, id: i2 }) = (a, b) else { unreachable!() };
                let container = ContainerRef::Compound { first: (p1, i1), second: (p2, i2) };
                self.container_start_open(container, player);
                Some((AbstractContainerMenu::new(container_id, MenuKind::Chest { rows: 6 }, Some(container)), name))
            }
        }
    }

    pub fn take_container_menu(&mut self, player: EntityId) -> Option<AbstractContainerMenu> {
        let p = self.player_mut(player);
        p.container_menu.take().or_else(|| p.inventory_menu.take())
    }

    pub fn restore_container_menu(&mut self, player: EntityId, menu: AbstractContainerMenu) {
        if !self.players.contains_key(&player) {
            return;
        }
        let p = self.player_mut(player);
        if menu.container_id == CONTAINER_ID {
            if p.inventory_menu.is_none() {
                p.inventory_menu = Some(menu);
            }
        } else if p.container_menu.is_none() {
            p.container_menu = Some(menu);
        }
    }

    pub fn container_menu_id(&self, player: EntityId) -> i32 {
        self.player(player).container_menu.as_ref().map_or(CONTAINER_ID, |menu| menu.container_id)
    }

    pub fn init_inventory_menu(&mut self, player: EntityId) {
        let Some(mut menu) = self.player_mut(player).inventory_menu.take() else { return };
        self.menu_set_synchronizer(&mut menu, player);
        self.restore_container_menu(player, menu);
    }

    pub fn broadcast_container_menu_changes(&mut self, player: EntityId) {
        let Some(mut menu) = self.take_container_menu(player) else { return };
        self.menu_broadcast_changes(&mut menu, player);
        self.restore_container_menu(player, menu);
    }

    pub fn broadcast_inventory_menu_changes(&mut self, player: EntityId) {
        let Some(mut menu) = self.player_mut(player).inventory_menu.take() else { return };
        self.menu_broadcast_changes(&mut menu, player);
        self.restore_container_menu(player, menu);
    }

    pub fn close_container(&mut self, player: EntityId) {
        let container_id = self.container_menu_id(player);
        self.player(player).connection.send(clientbound::CONTAINER_CLOSE, |w| w.varint(container_id));
        self.do_close_container(player);
    }

    pub fn do_close_container(&mut self, player: EntityId) {
        let Some(mut menu) = self.take_container_menu(player) else { return };
        self.menu_removed(&mut menu, player);
        if menu.container_id == CONTAINER_ID {
            self.restore_container_menu(player, menu);
        } else if let Some(inventory_menu) = self.player_mut(player).inventory_menu.as_mut() {
            inventory_menu.transfer_state(&menu);
        }
    }

    pub fn remove_player_menus(&mut self, player: EntityId) {
        if let Some(mut menu) = self.player_mut(player).inventory_menu.take() {
            self.menu_removed(&mut menu, player);
            self.restore_container_menu(player, menu);
        }
        if self.player(player).container_menu.is_some() {
            self.do_close_container(player);
        }
    }

    pub fn open_menu(&mut self, player: EntityId, provider: MenuProvider) -> bool {
        if self.player(player).container_menu.is_some() {
            self.close_container(player);
        }
        let p = self.player_mut(player);
        p.last_window_id = p.last_window_id % 100 + 1;
        let container_id = p.last_window_id;
        let Some((mut menu, title)) = self.create_menu(provider, container_id, player) else {
            if self.is_spectator(player) {
                let message = Component::translatable("container.spectatorCantOpen").with_color(ChatFormatting::Red);
                self.player(player).connection.send(clientbound::SYSTEM_CHAT, |p| {
                    message.write(p);
                    p.bool(true);
                });
            }
            return false;
        };
        let menu_type = menu_type_id(menu.kind);
        self.player(player).connection.send(clientbound::OPEN_SCREEN, |w| {
            w.varint(container_id);
            w.varint(menu_type);
            title.write(w);
        });
        self.menu_broadcast_changes(&mut menu, player);
        self.menu_set_synchronizer(&mut menu, player);
        self.player_mut(player).container_menu = Some(menu);
        true
    }

    pub fn tick_container_menu(&mut self, player: EntityId) {
        let Some(mut menu) = self.take_container_menu(player) else { return };
        self.menu_broadcast_changes(&mut menu, player);
        let valid = self.menu_still_valid(&menu, player);
        self.restore_container_menu(player, menu);
        if !valid {
            self.close_container(player);
        }
    }

    pub fn handle_container_button_click(&mut self, player: EntityId, container_id: i32, button_id: i32) {
        self.player_mut(player).reset_last_action_time();
        if self.container_menu_id(player) != container_id || self.is_spectator(player) {
            return;
        }
        let Some(mut menu) = self.take_container_menu(player) else { return };
        if self.menu_still_valid(&menu, player) && self.menu_click_menu_button(&mut menu, player, button_id) {
            self.menu_broadcast_changes(&mut menu, player);
        }
        self.restore_container_menu(player, menu);
    }

    pub fn handle_container_menu_click(&mut self, player: EntityId, state_id: i32, slot: i32, button: i32, input: i32, claims: Vec<(i32, crate::network::hashed_stack::HashedStack)>, carried: crate::network::hashed_stack::HashedStack) -> Result<(), ()> {
        let Some(mut menu) = self.take_container_menu(player) else { return Ok(()) };
        let result = (|| {
            if self.is_spectator(player) || self.is_dead_or_dying(player) {
                self.menu_send_all_data_to_remote(&mut menu, player);
                return Ok(());
            }
            if !self.menu_still_valid(&menu, player) || !menu.is_valid_slot_index(slot) {
                return Ok(());
            }
            let full_resync_needed = state_id != menu.state_id();
            menu.set_suppress_remote_updates(true);
            let clicked = self.menu_clicked(&mut menu, player, slot, button, input);
            for (slot, stack) in claims {
                menu.set_remote_slot_unsafe(slot, stack);
            }
            menu.set_remote_carried(carried);
            menu.set_suppress_remote_updates(false);
            clicked?;
            if full_resync_needed {
                self.menu_broadcast_full_state(&mut menu, player);
            } else {
                self.menu_broadcast_changes(&mut menu, player);
            }
            Ok(())
        })();
        self.restore_container_menu(player, menu);
        result
    }
}
