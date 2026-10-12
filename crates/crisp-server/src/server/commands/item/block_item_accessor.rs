use crisp_core::BlockPos;

use super::super::command_response_tracker::{CommandResponseTracker, ElementType};
use super::item_accessor::{ItemAccessor, SetterFunction};
use super::item_commands::{self, ERROR_SOURCE_NOT_A_CONTAINER, ERROR_TARGET_NO_CHANGES, ERROR_TARGET_NO_CHANGES_KNOWN_ITEM, ERROR_TARGET_NOT_A_CONTAINER};
use crate::commands::brigadier::{CommandSyntaxException, Dynamic3CommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::item::ItemStack;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::storage::loot::loot_params::SlotProvider;

pub struct BlockItemAccessor {
    pub pos: BlockPos,
}

fn replace_success(pos: BlockPos, slot_count: i32) -> Component {
    Component::translatable_with("commands.item.block.replace.success", vec![Arg::Int(slot_count), Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)])
}

impl BlockItemAccessor {
    fn get_block_slots<R>(
        &self,
        server: &mut Server,
        source: &CommandSourceStack,
        slot_source: &SlotSource,
        exception_type: &Dynamic3CommandExceptionType,
        output: impl FnOnce(&mut ServerLevel, &crate::world::item::slot::slot_collection::SlotCollection) -> R,
    ) -> Result<R, CommandSyntaxException> {
        let level = &mut server.levels[source.level];
        let container = get_container(level, self.pos, exception_type)?;
        let slots = item_commands::get_slots_from_provider(level, source, SlotProvider::Container(container), slot_source);
        Ok(output(level, &slots))
    }
}

impl ItemAccessor for BlockItemAccessor {
    type Target = BlockPos;

    fn get_slots(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource) -> Result<Vec<ItemStack>, CommandSyntaxException> {
        self.get_block_slots(server, source, slot_source, &ERROR_SOURCE_NOT_A_CONTAINER, |level, slots| slots.item_copies(level))
    }

    fn set_items(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource, function: &mut SetterFunction<BlockPos>) -> Result<(), CommandSyntaxException> {
        self.get_block_slots(server, source, slot_source, &ERROR_TARGET_NOT_A_CONTAINER, |level, target_slots| {
            function(level, self.pos, target_slots);
        })
    }

    fn get_replace_success(
        &self,
        server: &Server,
        source: &CommandSourceStack,
        tracker: CommandResponseTracker<BlockPos>,
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
                    |pos, slot_count| {
                        Component::translatable_with(
                            "commands.item.block.replace.success.known_item",
                            vec![Arg::Int(slot_count), Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z), item_stack.display_name().into()],
                        )
                    },
                    |_, _| Component::empty(),
                )
            }
            None => tracker.send_feedback_of(server, source, true, ElementType::NonZero, Some(|| ERROR_TARGET_NO_CHANGES.create()), replace_success, |_, _| Component::empty()),
        }
    }

    fn get_modify_success(&self, server: &Server, source: &CommandSourceStack, tracker: CommandResponseTracker<BlockPos>) -> Result<i32, CommandSyntaxException> {
        tracker.send_feedback_of(
            server,
            source,
            true,
            ElementType::NonZero,
            Some(|| ERROR_TARGET_NO_CHANGES.create()),
            |pos, slot_count| {
                Component::translatable_with("commands.item.block.modify.success", vec![Arg::Int(slot_count), Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)])
            },
            |_, _| Component::empty(),
        )
    }
}

pub fn get_container(level: &mut ServerLevel, pos: BlockPos, exception_type: &Dynamic3CommandExceptionType) -> Result<ContainerRef, CommandSyntaxException> {
    let container = match level.get_block_entity(pos) {
        Some(entity) => match &entity.kind {
            BlockEntityKind::Jukebox(_) => Some(ContainerRef::Jukebox(pos)),
            BlockEntityKind::ChiseledBookShelf(_) => Some(ContainerRef::ChiseledBookShelf(pos)),
            BlockEntityKind::Shelf(_) => Some(ContainerRef::Shelf(pos)),
            BlockEntityKind::DecoratedPot(_) => Some(ContainerRef::DecoratedPot(pos)),
            _ if entity.container_base().is_some() => Some(ContainerRef::Block { pos, id: entity.id() }),
            _ => None,
        },
        None => None,
    };
    container.ok_or_else(|| exception_type.create(pos.x, pos.y, pos.z))
}
