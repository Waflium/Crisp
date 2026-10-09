use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::item::component::swing_animation::SwingAnimation;

static ERROR_NO_LIVING_ENTITY: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.swing.failed.notliving");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn swing_self(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let entity = source.entity_or_exception()?;
    swing(server, source, &[entity], InteractionHand::MainHand, SwingAnimation::DEFAULT)
}

pub fn swing_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    swing(server, source, &targets, InteractionHand::MainHand, SwingAnimation::DEFAULT)
}

pub fn swing_hand(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    let hand = if context.nodes.iter().any(|n| crisp_data::commands::CommandNode::get(n.node).name == "offhand") {
        InteractionHand::OffHand
    } else {
        InteractionHand::MainHand
    };
    let animation = if context.has_argument("animation") {
        let duration = if context.has_argument("duration") { context.get_int("duration") } else { SwingAnimation::DEFAULT.duration };
        SwingAnimation { ty: context.get_swing_animation_type("animation"), duration }
    } else {
        SwingAnimation::DEFAULT
    };
    swing(server, source, &targets, hand, animation)
}

fn swing(server: &mut Server, source: &CommandSourceStack, targets: &[EntityId], hand: InteractionHand, animation: SwingAnimation) -> Result<i32> {
    let mut tracker = CommandResponseTracker::new();
    for &entity in targets {
        if server.level.is_player(entity) {
            server.level.swing(entity, hand, animation, true);
            tracker.track(entity);
        }
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_NO_LIVING_ENTITY.create()),
        |entity, _| Component::translatable_with("commands.swing.success.single", vec![server.entity_display_name(entity).into()]),
        |count, _| Component::translatable_with("commands.swing.success.multiple", vec![count.into()]),
    )
}
