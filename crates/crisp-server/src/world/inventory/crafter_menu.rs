use super::abstract_container_menu::AbstractContainerMenu;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::item::ItemStack;
use crate::world::level::block::crafter_block::get_potential_results;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::crafter_block_entity::SLOT_DISABLED;

pub const SLOT_COUNT: usize = 9;
pub const INV_SLOT_START: usize = 9;
pub const USE_ROW_SLOT_END: usize = 45;
const RESULT_CONTAINER: usize = 0;

impl ServerLevel {
    fn crafter_menu_data(&self, menu: &AbstractContainerMenu, data_id: usize) -> i32 {
        match menu.container.and_then(ContainerRef::single_block).and_then(|pos| self.chunks.block_entity_ref(pos)).map(|entity| &entity.kind) {
            Some(BlockEntityKind::Crafter(crafter_block_entity)) => crafter_block_entity.container_data.get(data_id),
            _ => 0,
        }
    }

    pub fn crafter_menu_is_slot_disabled(&self, menu: &AbstractContainerMenu, slot_id: usize) -> bool {
        slot_id < SLOT_COUNT && self.crafter_menu_data(menu, slot_id) == SLOT_DISABLED
    }

    pub fn crafter_menu_refresh_recipe_result(&mut self, menu: &mut AbstractContainerMenu) {
        let Some(pos) = menu.container.and_then(ContainerRef::single_block) else { return };
        let Some(crafter_block_entity) = self.crafter(pos) else { return };
        let mut craft_input = crafter_block_entity.as_craft_input();
        let result = get_potential_results(self, &mut craft_input).map_or_else(ItemStack::empty, |recipe| recipe.crafting().expect("a crafting recipe").assemble(&craft_input));
        *menu.containers[RESULT_CONTAINER].get_item_mut(0) = result;
    }

    pub fn crafter_menu_slot_changed(&mut self, menu: &mut AbstractContainerMenu) {
        self.crafter_menu_refresh_recipe_result(menu);
    }
}
