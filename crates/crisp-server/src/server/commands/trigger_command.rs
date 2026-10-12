use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::scores::criteria::ObjectiveCriteria;
use crate::world::scores::objective::ObjectiveId;
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::scores::scoreboard::ScoreAccess;

static ERROR_NOT_PRIMED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.trigger.failed.unprimed");
static ERROR_INVALID_OBJECTIVE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.trigger.failed.invalid");

type Result = std::result::Result<i32, CommandSyntaxException>;

fn player_and_objective(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> std::result::Result<(ScoreHolder, ObjectiveId), CommandSyntaxException> {
    let player = source.player_or_exception(server)?;
    let objective = context.get_objective(server, "objective")?;
    Ok((ScoreHolder::for_entity(&server.level, player), objective))
}

fn formatted(server: &Server, objective: ObjectiveId) -> Component {
    server.scoreboard.borrow().objective(objective).formatted_display_name()
}

pub fn add_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let (player, objective) = player_and_objective(server, context, source)?;
    let amount = context.get_int("value");
    let mut score = get_score(server, &player, objective)?;
    let new_value = score.add(&mut server.level, amount).expect("triggers are writable");
    source.send_success(server, || Component::translatable_with("commands.trigger.add.success", vec![formatted(server, objective).into(), Arg::Int(amount)]), true);
    Ok(new_value)
}

pub fn set_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let (player, objective) = player_and_objective(server, context, source)?;
    let amount = context.get_int("value");
    let mut score = get_score(server, &player, objective)?;
    score.set(&mut server.level, amount).expect("triggers are writable");
    source.send_success(server, || Component::translatable_with("commands.trigger.set.success", vec![formatted(server, objective).into(), Arg::Int(amount)]), true);
    Ok(amount)
}

pub fn simple_trigger(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let (player, objective) = player_and_objective(server, context, source)?;
    let mut score = get_score(server, &player, objective)?;
    let new_value = score.add(&mut server.level, 1).expect("triggers are writable");
    source.send_success(server, || Component::translatable_with("commands.trigger.simple.success", vec![formatted(server, objective).into()]), true);
    Ok(new_value)
}

fn get_score(server: &mut Server, holder: &ScoreHolder, objective: ObjectiveId) -> std::result::Result<ScoreAccess, CommandSyntaxException> {
    let mut score = {
        let mut scoreboard = server.scoreboard.borrow_mut();
        if *scoreboard.objective(objective).criteria() != ObjectiveCriteria::TRIGGER {
            return Err(ERROR_INVALID_OBJECTIVE.create());
        }
        if !scoreboard.get_player_score_info(holder.scoreboard_name(), objective).is_some_and(|score| !score.locked) {
            return Err(ERROR_NOT_PRIMED.create());
        }
        scoreboard.get_or_create_player_score(holder, objective, false)
    };
    score.lock(&mut server.level);
    Ok(score)
}

pub fn suggest_objectives(
    server: &Server,
    _context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let mut result = Vec::new();
    if let Some(entity) = source.entity {
        let holder = ScoreHolder::for_entity(&server.level, entity);
        let scoreboard = server.scoreboard.borrow();
        for objective in scoreboard.objectives() {
            if *objective.criteria() == ObjectiveCriteria::TRIGGER
                && scoreboard.get_player_score_info(holder.scoreboard_name(), objective.id()).is_some_and(|score| !score.locked)
            {
                result.push(objective.name().to_string());
            }
        }
    }
    crate::commands::shared_suggestion_provider::suggest(result, &mut builder);
    Ok(builder.build())
}
