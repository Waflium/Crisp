use super::super::command_response_tracker::{CommandResponseTracker, ElementType};
use super::item_accessor::{ItemAccessor, SetterFunction};
use super::item_commands::{self, ERROR_TARGET_NO_CHANGES, ERROR_TARGET_NO_CHANGES_KNOWN_ITEM};
use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::loot_params::SlotProvider;

pub struct EntityItemAccessor {
    pub entities: Vec<EntityId>,
}

impl ItemAccessor for EntityItemAccessor {
    type Target = EntityId;

    fn get_slots(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource) -> Result<Vec<ItemStack>, CommandSyntaxException> {
        let mut slot_collections = Vec::new();
        for &entity in &self.entities {
            let level = server.entity_level_mut(entity);
            let slots = item_commands::get_slots_from_provider(level, source, SlotProvider::Entity(entity), slot_source);
            slot_collections.extend(slots.item_copies(level));
        }
        Ok(slot_collections)
    }

    fn set_items(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource, function: &mut SetterFunction<EntityId>) -> Result<(), CommandSyntaxException> {
        for &entity in &self.entities {
            let level = server.entity_level_mut(entity);
            let target_slots = item_commands::get_slots_from_provider(level, source, SlotProvider::Entity(entity), slot_source);
            let updated_slots = function(level, entity, &target_slots);
            if updated_slots > 0 && level.is_player(entity) {
                level.broadcast_container_menu_changes(entity);
            }
        }
        Ok(())
    }

    fn get_replace_success(
        &self,
        server: &Server,
        source: &CommandSourceStack,
        tracker: CommandResponseTracker<EntityId>,
        known_item: Option<&ItemStack>,
    ) -> Result<i32, CommandSyntaxException> {
        match known_item {
            Some(item_stack) => {
                if tracker.element_count(ElementType::NonZero) == 0 {
                    return Err(ERROR_TARGET_NO_CHANGES_KNOWN_ITEM.create(item_commands::item_stack_to_string(item_stack)));
                }
                tracker.send_feedback(
                    server,
                    source,
                    true,
                    |entity, slot_count| {
                        Component::translatable_with(
                            "commands.item.entity.replace.success.single.known_item",
                            vec![Arg::Int(slot_count), server.entity_display_name(entity).into(), item_stack.display_name().into()],
                        )
                    },
                    |entity_count, _| {
                        Component::translatable_with("commands.item.entity.replace.success.multiple.known_item", vec![Arg::Int(entity_count), item_stack.display_name().into()])
                    },
                )
            }
            None => tracker.send_feedback_of(
                server,
                source,
                true,
                ElementType::NonZero,
                Some(|| ERROR_TARGET_NO_CHANGES.create()),
                |entity, slot_count| {
                    Component::translatable_with("commands.item.entity.replace.success.single", vec![Arg::Int(slot_count), server.entity_display_name(entity).into()])
                },
                |entity_count, _| Component::translatable_with("commands.item.entity.replace.success.multiple", vec![Arg::Int(entity_count)]),
            ),
        }
    }

    fn get_modify_success(&self, server: &Server, source: &CommandSourceStack, tracker: CommandResponseTracker<EntityId>) -> Result<i32, CommandSyntaxException> {
        tracker.send_feedback_of(
            server,
            source,
            true,
            ElementType::NonZero,
            Some(|| ERROR_TARGET_NO_CHANGES.create()),
            |entity, slot_count| {
                Component::translatable_with("commands.item.entity.modify.success.single", vec![Arg::Int(slot_count), server.entity_display_name(entity).into()])
            },
            |entity_count, _| Component::translatable_with("commands.item.entity.modify.success.multiple", vec![Arg::Int(entity_count)]),
        )
    }
}
