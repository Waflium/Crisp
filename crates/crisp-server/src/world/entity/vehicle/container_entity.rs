use crisp_data::game_rule;
use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::damagesource::DamageSource;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::block::entity::base_container_block_entity::{load_all_items, save_all_items};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::{InteractionResult, containers};

pub struct ContainerEntity {
    item_stacks: Vec<ItemStack>,
    loot_table: Option<String>,
    loot_table_seed: i64,
}

impl ContainerEntity {
    pub fn new(size: usize) -> ContainerEntity {
        ContainerEntity { item_stacks: vec![ItemStack::empty(); size], loot_table: None, loot_table_seed: 0 }
    }

    pub fn get_item_stacks(&mut self) -> &mut Vec<ItemStack> {
        &mut self.item_stacks
    }

    pub fn clear_item_stacks(&mut self) {
        let size = self.item_stacks.len();
        self.item_stacks = vec![ItemStack::empty(); size];
    }

    pub fn add_chest_vehicle_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        match &self.loot_table {
            Some(loot_table) => {
                output.push(("LootTable".into(), Nbt::String(loot_table.clone())));
                if self.loot_table_seed != 0 {
                    output.push(("LootTableSeed".into(), Nbt::Long(self.loot_table_seed)));
                }
            }
            None => save_all_items(output, &self.item_stacks, true),
        }
    }

    pub fn read_chest_vehicle_save_data(&mut self, input: &Nbt) {
        self.clear_item_stacks();
        self.loot_table = input.get("LootTable").and_then(Nbt::as_str).map(str::to_string);
        self.loot_table_seed = input.get("LootTableSeed").and_then(Nbt::as_i64).unwrap_or(0);
        if self.loot_table.is_none() {
            load_all_items(input, &mut self.item_stacks);
        }
    }

    pub fn is_chest_vehicle_empty(&self) -> bool {
        self.item_stacks.iter().all(ItemStack::is_empty)
    }

    pub fn has_loot_table(&self) -> bool {
        self.loot_table.is_some()
    }
}

impl ServerLevel {
    pub fn container_entity(&mut self, id: EntityId) -> Option<&mut ContainerEntity> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Boat(boat)) => boat.container.as_mut(),
            _ => None,
        }
    }

    pub fn is_container_entity(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Boat(boat)) if boat.container.is_some())
    }

    pub fn unpack_chest_vehicle_loot_table(&mut self, id: EntityId, player: Option<EntityId>) {
        let Some(container) = self.container_entity(id) else { return };
        let Some(loot_table_key) = container.loot_table.take() else { return };
        let loot_table_seed = container.loot_table_seed;
        let loot_table = LootTable::get(&loot_table_key);
        if let Some(player) = player.filter(|&player| self.is_player(player)) {
            crate::advancements::triggers::criteria_triggers::GENERATE_LOOT.trigger(self, player, &loot_table_key);
        }
        let origin = self.entity(id).position();
        let luck = player.map(|player| self.attribute_value(player, Attribute::Luck) as f32);
        let mut params = LootParams::new(self);
        params.origin = Some(origin);
        if let Some(luck) = luck {
            params.luck = luck;
            params.this_entity = player;
        }
        loot_table.fill(ContainerRef::ChestVehicle(id), params, loot_table_seed);
    }

    pub fn chest_vehicle_destroyed(&mut self, id: EntityId, _source: &DamageSource) {
        if self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
            containers::drop_contents_for_entity(self, id, ContainerRef::ChestVehicle(id));
        }
    }

    pub fn interact_with_container_vehicle(&mut self, id: EntityId, player: EntityId) -> InteractionResult {
        self.open_menu(player, crate::world::inventory::menu_provider::MenuProvider::ChestVehicle(id));
        InteractionResult::SUCCESS
    }

    pub fn is_chest_vehicle_still_valid(&self, id: EntityId, player: EntityId) -> bool {
        self.entity_ref(id).is_some_and(|e| !e.is_removed()) && self.is_within_entity_interaction_range(player, id, 4.0)
    }
}
