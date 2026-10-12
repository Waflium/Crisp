use super::super::command_response_tracker::CommandResponseTracker;
use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::slot::slot_collection::SlotCollection;
use crate::world::item::slot::slot_source::SlotSource;

pub type SetterFunction<'f, Target> = dyn FnMut(&mut ServerLevel, Target, &SlotCollection) -> i32 + 'f;

pub trait ItemAccessor {
    type Target: Clone;

    fn set_items(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource, function: &mut SetterFunction<Self::Target>) -> Result<(), CommandSyntaxException>;

    fn get_slots(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource) -> Result<Vec<ItemStack>, CommandSyntaxException>;

    fn get_replace_success(
        &self,
        server: &Server,
        source: &CommandSourceStack,
        tracker: CommandResponseTracker<Self::Target>,
        known_item: Option<&ItemStack>,
    ) -> Result<i32, CommandSyntaxException>;

    fn get_modify_success(&self, server: &Server, source: &CommandSourceStack, tracker: CommandResponseTracker<Self::Target>) -> Result<i32, CommandSyntaxException>;
}
