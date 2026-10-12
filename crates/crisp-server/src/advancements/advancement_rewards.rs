use crisp_core::random::RandomSource;
use crisp_data::registry;
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::codec::{self, DataResult};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;

#[derive(Default)]
pub struct AdvancementRewards {
    pub experience: i32,
    pub loot: Vec<Holder<LootTable>>,
    pub recipes: Vec<String>,
    pub function: Option<String>,
}

impl AdvancementRewards {
    pub fn decode(tag: &Nbt) -> DataResult<AdvancementRewards> {
        codec::map(tag)?;
        Ok(AdvancementRewards {
            experience: codec::decode_or(tag, "experience", 0, codec::int)?,
            loot: codec::decode_or(tag, "loot", Vec::new(), LootTable::decode_list)?,
            recipes: codec::decode_or(tag, "recipes", Vec::new(), |t| codec::list_of(t, codec::identifier))?,
            function: codec::decode_optional(tag, "function", codec::identifier)?,
        })
    }

    pub fn grant(&self, level: &mut ServerLevel, player: EntityId) {
        level.give_experience_points(player, self.experience);
        let mut changes = false;
        for loot_table in &self.loot {
            let origin = level.player(player).entity.position();
            let mut params = LootParams::new(level);
            params.this_entity = Some(player);
            params.origin = Some(origin);
            for mut item_stack in loot_table.value().get_random_items(params) {
                let p = level.player_mut(player);
                let infinite_materials = p.abilities.instabuild;
                if p.inventory.add_stack(&mut item_stack, infinite_materials).0 {
                    let p = level.player_mut(player);
                    let (x, y, z) = (p.entity.x(), p.entity.y(), p.entity.z());
                    let pitch = ((p.entity.random.next_float() - p.entity.random.next_float()) * 0.7 + 1.0) * 2.0;
                    let sound = crisp_data::registry_id(registry::SOUND_EVENT, "minecraft:entity.item.pickup");
                    level.play_sound_at(None, x, y, z, sound, SoundSource::Players, 0.2, pitch);
                    changes = true;
                } else if !item_stack.is_empty() {
                    level.swing(player, crate::world::InteractionHand::MainHand, crate::world::item::component::swing_animation::SwingAnimation::DEFAULT, true);
                    let mut drop = level.create_item_stack_to_drop(player, item_stack, false, false);
                    drop.set_no_pickup_delay();
                    drop.target = Some(level.player(player).profile.uuid);
                    level.add_fresh_entity(LevelEntity::Item(drop));
                }
            }
        }
        if changes {
            level.broadcast_container_menu_changes(player);
        }
        if !self.recipes.is_empty() {
            let recipe_manager = RecipeManager::get();
            let recipes: Vec<&'static RecipeHolder> = self.recipes.iter().filter_map(|id| recipe_manager.by_key(id)).collect();
            level.award_recipes(player, &recipes);
        }
        if let Some(function) = &self.function {
            level.pending_function_executions.push((function.clone(), player));
        }
    }
}
