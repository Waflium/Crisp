use crisp_data::EquipmentSlot;

use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::brigadier::{
    CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, tr,
};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper;

static ERROR_NOT_LIVING_ENTITY: DynamicCommandExceptionType = DynamicCommandExceptionType(|target| tr("commands.enchant.failed.entity", vec![target]));
static ERROR_NO_ITEM: DynamicCommandExceptionType = DynamicCommandExceptionType(|target| tr("commands.enchant.failed.itemless", vec![target]));
static ERROR_INCOMPATIBLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|item| tr("commands.enchant.failed.incompatible", vec![item]));
static ERROR_LEVEL_TOO_HIGH: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|level, max| tr("commands.enchant.failed.level", vec![level, max]));
static ERROR_NOTHING_HAPPENED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.enchant.failed");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn enchant_default(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    enchant(server, source, &targets, context.get_identifier("enchantment"), 1)
}

pub fn enchant_level(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    enchant(server, source, &targets, context.get_identifier("enchantment"), context.get_int("level"))
}

fn enchant(server: &mut Server, source: &CommandSourceStack, targets: &[EntityId], enchantment_holder: &str, level: i32) -> Result<i32> {
    let enchantment = Enchantment::by_id(enchantment_holder).expect("registered enchantment");
    if level > enchantment.max_level() {
        return Err(ERROR_LEVEL_TOO_HIGH.create(level, enchantment.max_level()));
    }
    let mut tracker = CommandResponseTracker::new();
    for &entity in targets {
        let entity_level = server.entity_level_mut(entity);
        if entity_level.is_living(entity) {
            match entity_level.item_by_slot(entity, EquipmentSlot::Mainhand) {
                Some(mut item) => {
                    let existing = enchantment_helper::get_enchantments_for_crafting(&item);
                    if enchantment.can_enchant(&item)
                        && enchantment_helper::is_enchantment_compatible(existing.enchantments.iter().map(|(id, _)| id.as_str()), enchantment_holder)
                    {
                        enchantment_helper::enchant(&mut item, enchantment_holder, level);
                        entity_level.set_item_slot(entity, EquipmentSlot::Mainhand, item);
                        tracker.track(entity);
                    } else if targets.len() == 1 {
                        return Err(ERROR_INCOMPATIBLE.create(item.hover_name().get_string()));
                    }
                }
                None if targets.len() == 1 => return Err(ERROR_NO_ITEM.create(server.entity_display_name(entity).get_string())),
                None => {}
            }
        } else if targets.len() == 1 {
            return Err(ERROR_NOT_LIVING_ENTITY.create(server.entity_display_name(entity).get_string()));
        }
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_NOTHING_HAPPENED.create()),
        |entity, _| {
            Component::translatable_with(
                "commands.enchant.success.single",
                vec![Enchantment::get_fullname(enchantment_holder, level).into(), server.entity_display_name(entity).into()],
            )
        },
        |count, _| Component::translatable_with("commands.enchant.success.multiple", vec![Enchantment::get_fullname(enchantment_holder, level).into(), count.into()]),
    )
}
