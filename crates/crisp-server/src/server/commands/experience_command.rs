use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_SET_POINTS_INVALID: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.experience.set.points.invalid");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Type {
    Points,
    Levels,
}

impl Type {
    fn name(self) -> &'static str {
        match self {
            Type::Points => "points",
            Type::Levels => "levels",
        }
    }

    fn add(self, server: &mut Server, player: EntityId, amount: i32) {
        match self {
            Type::Points => server.entity_level_mut(player).give_experience_points(player, amount),
            Type::Levels => server.entity_level_mut(player).give_experience_levels(player, amount),
        }
    }

    fn set(self, server: &mut Server, player: EntityId, amount: i32) -> bool {
        let p = server.player_mut(player);
        match self {
            Type::Points if amount >= p.experience.xp_needed_for_next_level() => false,
            Type::Points => {
                p.set_experience_points(amount);
                true
            }
            Type::Levels => {
                p.set_experience_levels(amount);
                true
            }
        }
    }

    fn query(self, server: &Server, player: EntityId) -> i32 {
        let xp = server.player(player).experience;
        match self {
            Type::Points => mth_floor(xp.progress * xp.xp_needed_for_next_level() as f32),
            Type::Levels => xp.level,
        }
    }
}

fn mth_floor(v: f32) -> i32 {
    crisp_core::mth::floor(v as f64)
}

pub fn executors() -> Vec<(String, CommandFn)> {
    let executors: [(&str, CommandFn); 8] = [
        ("experience add target amount", add_points),
        ("experience add target amount points", add_points),
        ("experience add target amount levels", add_levels),
        ("experience set target amount", set_points),
        ("experience set target amount points", set_points),
        ("experience set target amount levels", set_levels),
        ("experience query target points", query_points),
        ("experience query target levels", query_levels),
    ];
    executors.into_iter().map(|(path, f)| (path.to_string(), f)).collect()
}

fn add_points(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "target")?;
    add_experience(server, source, players, context.get_int("amount"), Type::Points)
}

fn add_levels(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "target")?;
    add_experience(server, source, players, context.get_int("amount"), Type::Levels)
}

fn set_points(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "target")?;
    set_experience(server, source, players, context.get_int("amount"), Type::Points)
}

fn set_levels(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "target")?;
    set_experience(server, source, players, context.get_int("amount"), Type::Levels)
}

fn query_points(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let player = context.get_player(server, source, "target")?;
    query_experience(server, source, player, Type::Points)
}

fn query_levels(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let player = context.get_player(server, source, "target")?;
    query_experience(server, source, player, Type::Levels)
}

fn query_experience(server: &mut Server, source: &CommandSourceStack, player: EntityId, ty: Type) -> Result<i32, CommandSyntaxException> {
    let result = ty.query(server, player);
    let key = format!("commands.experience.query.{}", ty.name());
    source.send_success(server, || Component::translatable_with(&key, vec![server.entity_display_name(player).into(), Arg::Int(result)]), false);
    Ok(result)
}

fn add_experience(server: &mut Server, source: &CommandSourceStack, players: Vec<EntityId>, amount: i32, ty: Type) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    for player in players {
        ty.add(server, player, amount);
        tracker.track(player);
    }
    let (single, multiple) = (format!("commands.experience.add.{}.success.single", ty.name()), format!("commands.experience.add.{}.success.multiple", ty.name()));
    tracker.send_feedback(
        server,
        source,
        true,
        |player, _| Component::translatable_with(&single, vec![Arg::Int(amount), server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with(&multiple, vec![Arg::Int(amount), Arg::Int(count)]),
    )
}

fn set_experience(server: &mut Server, source: &CommandSourceStack, players: Vec<EntityId>, amount: i32, ty: Type) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    for player in players {
        let set = ty.set(server, player, amount);
        tracker.track_value(player, set as i32);
    }
    let (single, multiple) = (format!("commands.experience.set.{}.success.single", ty.name()), format!("commands.experience.set.{}.success.multiple", ty.name()));
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_SET_POINTS_INVALID.create()),
        |player, _| Component::translatable_with(&single, vec![Arg::Int(amount), server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with(&multiple, vec![Arg::Int(amount), Arg::Int(count)]),
    )
}
