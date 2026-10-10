use crisp_data::{GameRule, GameRuleType};
use serde_json::{Map, Value};

use super::{ClientInfo, decode_params, log};
use crate::server::Server;
use crate::server::jsonrpc::codec::{self, DecodeResult};
use crate::world::level::gamerules::GameRules;

pub fn typed_rule(rule: GameRule, value: i32) -> Value {
    let info = rule.info();
    let mut object = Map::new();
    let (kind, value) = match info.kind {
        GameRuleType::Bool => ("boolean", Value::Bool(value != 0)),
        GameRuleType::Int => ("integer", Value::from(value)),
    };
    object.insert("type".into(), Value::String(kind.into()));
    object.insert("value".into(), value);
    object.insert("key".into(), Value::String(format!("minecraft:{}", info.name)));
    Value::Object(object)
}

pub fn get(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Array(server.game_rules.available().map(|rule| typed_rule(rule, server.game_rules.get(rule))).collect())
}

fn decode_update(value: &Value) -> DecodeResult<(GameRule, i32)> {
    let object = codec::object(value)?;
    let Some(key) = object.get("key") else { return Err(vec![format!("Input does not contain a key [key]: MapLike[{value}]")]) };
    let name = codec::string(key)?;
    let id = crate::resources::identifier::read(&name).map_err(|e| vec![e])?;
    let rule = GameRule::by_name(&id)
        .ok_or_else(|| vec![format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:game_rule]: {id}")])?;
    let Some(raw) = object.get("value") else { return Err(vec![format!("No key value in MapLike[{value}]")]) };
    let info = rule.info();
    let value = match info.kind {
        GameRuleType::Bool => codec::boolean(raw)? as i32,
        GameRuleType::Int => {
            let v = codec::int(raw)?;
            if v < info.min || v > info.max {
                return Err(vec![format!("Value {v} outside of range [{}:{}]", info.min, info.max)]);
            }
            v
        }
    };
    Ok((rule, value))
}

pub fn update(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let (rule, new_value) = decode_params(params, decode_update)?;
    let old_value = server.game_rules.get(rule);
    server.set_game_rule(rule, new_value);
    log(
        client,
        format_args!(
            "Game rule '{}' updated from '{}' to '{}'",
            rule.info().name,
            GameRules::serialize(rule, old_value),
            GameRules::serialize(rule, new_value)
        ),
    );
    Ok(typed_rule(rule, new_value))
}
