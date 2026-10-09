use crisp_data::commands::CommandNode;
use crisp_data::{GameRule, GameRuleType};

use crate::commands::CommandFn;
use crate::commands::arguments::ArgumentValue;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::level::gamerules::GameRules;

static ERROR_GAME_RULE_NOT_SET: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|rule, value| tr("commands.gamerule.not_set", vec![rule, value]));

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for rule in GameRules::default().available() {
        let name = rule.info().name;
        for literal in [name.to_string(), format!("minecraft:{name}")] {
            out.push((format!("gamerule {literal}"), query_rule));
            out.push((format!("gamerule {literal} value"), set_rule));
        }
    }
    out
}

fn rule(context: &CommandContext) -> GameRule {
    GameRule::by_name(CommandNode::get(context.nodes[1].node).name).expect("a game rule literal")
}

fn set_rule(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let rule = rule(context);
    let value = match (rule.info().kind, context.argument("value")) {
        (GameRuleType::Bool, ArgumentValue::Bool(b)) => *b as i32,
        (GameRuleType::Int, ArgumentValue::Int(i)) => *i,
        (_, other) => panic!("{other:?} for {}", rule.info().name),
    };
    let serialized = GameRules::serialize(rule, value);
    if server.game_rules.get(rule) == value {
        return Err(ERROR_GAME_RULE_NOT_SET.create(rule.info().name, serialized));
    }
    server.set_game_rule(rule, value);
    source.send_success(server, || Component::translatable_with("commands.gamerule.set", vec![rule.info().name.into(), serialized.as_str().into()]), true);
    Ok(value)
}

fn query_rule(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let rule = rule(context);
    let value = server.game_rules.get(rule);
    let serialized = GameRules::serialize(rule, value);
    source.send_success(server, || Component::translatable_with("commands.gamerule.query", vec![rule.info().name.into(), serialized.as_str().into()]), false);
    Ok(value)
}
