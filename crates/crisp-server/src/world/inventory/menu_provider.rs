use crisp_core::{BlockPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_data::{block_entity_types as types, registry, registry_id};

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind, menu_type_id};
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::crafting::CookingRecipeType;
use crate::world::level::block::chest_block;
use crate::world::level::block::entity::BlockEntity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuProvider {
    BlockEntity(BlockPos),
    DoubleChest { first: BlockPos, second: BlockPos },
}

fn menu_kind(entity: &BlockEntity) -> Option<MenuKind> {
    Some(match entity.ty() {
        types::CHEST | types::TRAPPED_CHEST | types::BARREL => MenuKind::Chest { rows: 3 },
        types::SHULKER_BOX => MenuKind::ShulkerBox,
        types::FURNACE => MenuKind::Furnace(CookingRecipeType::Smelting),
        types::BLAST_FURNACE => MenuKind::Furnace(CookingRecipeType::Blasting),
        types::SMOKER => MenuKind::Furnace(CookingRecipeType::Smoking),
        types::HOPPER => MenuKind::Hopper,
        types::DISPENSER | types::DROPPER => MenuKind::Dispenser,
        types::BREWING_STAND => MenuKind::BrewingStand,
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
                let kind = menu_kind(entity)?;
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
                Some((AbstractContainerMenu::new(container_id, kind, container), name))
            }
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
                Some((AbstractContainerMenu::new(container_id, MenuKind::Chest { rows: 6 }, container), name))
            }
        }
    }

    pub fn open_menu(&mut self, player: EntityId, provider: MenuProvider) -> bool {
        let p = self.player(player);
        if p.container_menu.is_some() || p.window.id != 0 {
            self.close_container(player, true);
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

    pub fn close_container_menu(&mut self, player: EntityId) -> Option<i32> {
        let mut menu = self.player_mut(player).container_menu.take()?;
        self.menu_removed(&mut menu, player);
        Some(menu.container_id)
    }

    pub fn tick_container_menu(&mut self, player: EntityId) {
        let Some(mut menu) = self.player_mut(player).container_menu.take() else { return };
        self.menu_broadcast_changes(&mut menu, player);
        let valid = self.menu_still_valid(&menu, player);
        self.player_mut(player).container_menu = Some(menu);
        if !valid {
            self.close_container(player, true);
        }
    }

    pub fn handle_container_menu_click(&mut self, player: EntityId, state_id: i32, slot: i32, button: i32, input: i32, claims: Vec<(i32, crate::network::hashed_stack::HashedStack)>, carried: crate::network::hashed_stack::HashedStack) -> Result<(), ()> {
        let Some(mut menu) = self.player_mut(player).container_menu.take() else { return Ok(()) };
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
        if self.players.contains_key(&player) && self.player(player).container_menu.is_none() {
            self.player_mut(player).container_menu = Some(menu);
        }
        result
    }
}
