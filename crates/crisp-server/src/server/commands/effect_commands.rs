use crisp_data::MobEffect;

use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::effect::MobEffectInstance;
use crate::world::effect::mob_effect_instance::INFINITE_DURATION;
use crate::world::entity::EntityId;

static ERROR_GIVE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.effect.give.failed");
static ERROR_CLEAR_EVERYTHING_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.effect.clear.everything.failed");
static ERROR_CLEAR_SPECIFIC_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.effect.clear.specific.failed");

pub fn executors() -> Vec<(String, CommandFn)> {
    let executors: [(&str, CommandFn); 10] = [
        ("effect clear", clear_self),
        ("effect clear targets", clear_targets),
        ("effect clear targets effect", clear_effect),
        ("effect give targets effect", give),
        ("effect give targets effect seconds", give_seconds),
        ("effect give targets effect seconds amplifier", give_seconds_amplifier),
        ("effect give targets effect seconds amplifier hideParticles", give_seconds_amplifier_particles),
        ("effect give targets effect infinite", give_infinite),
        ("effect give targets effect infinite amplifier", give_infinite_amplifier),
        ("effect give targets effect infinite amplifier hideParticles", give_infinite_amplifier_particles),
    ];
    executors.into_iter().map(|(path, f)| (path.to_string(), f)).collect()
}

fn get_mob_effect(context: &CommandContext, name: &str) -> MobEffect {
    MobEffect::by_name(context.get_identifier(name)).expect("parsed mob effect")
}

fn display_name(effect: MobEffect) -> Component {
    Component::translatable(effect.info().description_id)
}

fn give(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    give_effect(server, context, source, None, 0, true)
}

fn give_seconds(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    give_effect(server, context, source, Some(context.get_int("seconds")), 0, true)
}

fn give_seconds_amplifier(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    give_effect(server, context, source, Some(context.get_int("seconds")), context.get_int("amplifier"), true)
}

fn give_seconds_amplifier_particles(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let particles = !context.get_bool("hideParticles");
    give_effect(server, context, source, Some(context.get_int("seconds")), context.get_int("amplifier"), particles)
}

fn give_infinite(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    give_effect(server, context, source, Some(INFINITE_DURATION), 0, true)
}

fn give_infinite_amplifier(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    give_effect(server, context, source, Some(INFINITE_DURATION), context.get_int("amplifier"), true)
}

fn give_infinite_amplifier_particles(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let particles = !context.get_bool("hideParticles");
    give_effect(server, context, source, Some(INFINITE_DURATION), context.get_int("amplifier"), particles)
}

fn give_effect(
    server: &mut Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    seconds: Option<i32>,
    amplifier: i32,
    particles: bool,
) -> Result<i32, CommandSyntaxException> {
    let effect = get_mob_effect(context, "effect");
    let duration = compute_duration_in_ticks(seconds, effect);
    let mut tracker = CommandResponseTracker::new();
    for entity in context.get_entities(server, source, "targets")? {
        if server.entity_level(entity).is_living(entity) {
            let instance = MobEffectInstance::with(effect, duration, amplifier, false, particles, particles);
            let added = server.entity_level_mut(entity).add_effect(entity, instance, source.entity);
            tracker.track_value(entity, added as i32);
        }
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_GIVE_FAILED.create()),
        |entity, _| {
            Component::translatable_with(
                "commands.effect.give.success.single",
                vec![display_name(effect).into(), server.entity_display_name(entity).into(), (duration / 20).into()],
            )
        },
        |count, _| Component::translatable_with("commands.effect.give.success.multiple", vec![display_name(effect).into(), count.into(), (duration / 20).into()]),
    )
}

fn compute_duration_in_ticks(seconds: Option<i32>, effect: MobEffect) -> i32 {
    let instantaneous = effect.info().instantaneous;
    match seconds {
        Some(seconds) if instantaneous => seconds,
        Some(INFINITE_DURATION) => INFINITE_DURATION,
        Some(seconds) => seconds * 20,
        None if instantaneous => 1,
        None => 600,
    }
}

fn clear_self(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity = source.entity_or_exception()?;
    clear_effects(server, source, &[entity])
}

fn clear_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_entities(server, source, "targets")?;
    clear_effects(server, source, &targets)
}

fn clear_effects(server: &mut Server, source: &CommandSourceStack, entities: &[EntityId]) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    for &entity in entities {
        if server.entity_level(entity).is_living(entity) {
            let removed = server.entity_level_mut(entity).remove_all_effects(entity);
            tracker.track_value(entity, removed as i32);
        }
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_CLEAR_EVERYTHING_FAILED.create()),
        |entity, _| Component::translatable_with("commands.effect.clear.everything.success.single", vec![server.entity_display_name(entity).into()]),
        |count, _| Component::translatable_with("commands.effect.clear.everything.success.multiple", vec![count.into()]),
    )
}

fn clear_effect(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let effect = get_mob_effect(context, "effect");
    let mut tracker = CommandResponseTracker::new();
    for entity in context.get_entities(server, source, "targets")? {
        if server.entity_level(entity).is_living(entity) {
            let removed = server.entity_level_mut(entity).remove_effect(entity, effect);
            tracker.track_value(entity, removed as i32);
        }
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_CLEAR_SPECIFIC_FAILED.create()),
        |entity, _| {
            Component::translatable_with("commands.effect.clear.specific.success.single", vec![display_name(effect).into(), server.entity_display_name(entity).into()])
        },
        |count, _| Component::translatable_with("commands.effect.clear.specific.success.multiple", vec![display_name(effect).into(), count.into()]),
    )
}
