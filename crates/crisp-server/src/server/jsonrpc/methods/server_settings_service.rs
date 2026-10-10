use serde_json::Value;

use super::{ClientInfo, decode_params, log};
use crate::server::Server;
use crate::server::jsonrpc::codec;
use crate::server::level::game_mode::GameType;
use crate::world::difficulty::Difficulty;

const PERMISSION_LEVEL_NAMES: [&str; 5] = ["ALL", "MODERATORS", "GAMEMASTERS", "ADMINS", "OWNERS"];

fn decode_bool(params: &Value) -> Result<bool, String> {
    decode_params(params, codec::boolean)
}

fn decode_int(params: &Value) -> Result<i32, String> {
    decode_params(params, codec::int)
}

pub fn autosave(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.is_auto_save())
}

pub fn set_autosave(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let enabled = decode_bool(params)?;
    log(client, format_args!("Update autosave from {} to {enabled}", server.is_auto_save()));
    server.set_auto_save(enabled);
    Ok(Value::Bool(server.is_auto_save()))
}

pub fn difficulty(server: &mut Server, _client: ClientInfo) -> Value {
    Value::String(server.difficulty.key().into())
}

pub fn set_difficulty(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let difficulty = decode_params(params, |v| codec::enum_by_name(v, Difficulty::by_name))?;
    log(client, format_args!("Update difficulty from '{}' to '{}'", server.difficulty.key().to_uppercase(), difficulty.key().to_uppercase()));
    server.set_dedicated_difficulty(difficulty);
    Ok(Value::String(server.difficulty.key().into()))
}

pub fn enforce_allowlist(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.config.enforce_whitelist)
}

pub fn set_enforce_allowlist(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let enforce = decode_bool(params)?;
    log(client, format_args!("Update enforce allowlist from {} to {enforce}", server.config.enforce_whitelist));
    server.set_enforce_whitelist(enforce);
    server.kick_unlisted_players();
    Ok(Value::Bool(server.config.enforce_whitelist))
}

pub fn using_allowlist(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.config.white_list)
}

pub fn set_using_allowlist(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let used = decode_bool(params)?;
    log(client, format_args!("Update using allowlist from {} to {used}", server.config.white_list));
    server.set_using_whitelist(used);
    server.kick_unlisted_players();
    Ok(Value::Bool(server.config.white_list))
}

pub fn max_players(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.max_players as i32)
}

pub fn set_max_players(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let max = decode_int(params)?;
    log(client, format_args!("Update max players from {} to {max}", server.config.max_players as i32));
    server.set_max_players(max);
    Ok(Value::from(server.config.max_players as i32))
}

pub fn pause_when_empty(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.pause_when_empty_seconds)
}

pub fn set_pause_when_empty(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let seconds = decode_int(params)?;
    log(client, format_args!("Update pause when empty from {} seconds to {seconds} seconds", server.config.pause_when_empty_seconds));
    server.set_pause_when_empty_seconds(seconds);
    Ok(Value::from(server.config.pause_when_empty_seconds))
}

pub fn player_idle_timeout(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.player_idle_timeout)
}

pub fn set_player_idle_timeout(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let minutes = decode_int(params)?;
    log(client, format_args!("Update player idle timeout from {} minutes to {minutes} minutes", server.config.player_idle_timeout));
    server.set_player_idle_timeout(minutes);
    Ok(Value::from(server.config.player_idle_timeout))
}

pub fn allow_flight(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.config.allow_flight)
}

pub fn set_allow_flight(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let allow = decode_bool(params)?;
    log(client, format_args!("Update allow flight from {} to {allow}", server.config.allow_flight));
    server.set_allow_flight(allow);
    Ok(Value::Bool(server.config.allow_flight))
}

pub fn spawn_protection(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.spawn_protection)
}

pub fn set_spawn_protection(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let radius = decode_int(params)?;
    log(client, format_args!("Update spawn protection radius from {} to {radius}", server.config.spawn_protection));
    server.set_spawn_protection_radius(radius);
    Ok(Value::from(server.config.spawn_protection))
}

pub fn motd(server: &mut Server, _client: ClientInfo) -> Value {
    Value::String(server.config.motd.clone())
}

pub fn set_motd(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let motd = decode_params(params, codec::string)?;
    log(client, format_args!("Update MOTD from '{}' to '{motd}'", server.config.motd));
    server.set_motd(motd);
    Ok(Value::String(server.config.motd.clone()))
}

pub fn force_game_mode(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.config.force_game_mode)
}

pub fn set_force_game_mode(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let force = decode_bool(params)?;
    log(client, format_args!("Update force game mode from {} to {force}", server.config.force_game_mode));
    server.set_force_game_mode(force);
    Ok(Value::Bool(server.config.force_game_mode))
}

pub fn game_mode(server: &mut Server, _client: ClientInfo) -> Value {
    Value::String(server.config.game_mode.name().into())
}

pub fn set_game_mode(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let mode = decode_params(params, |v| codec::enum_by_name(v, GameType::by_name))?;
    log(client, format_args!("Update game mode from '{}' to '{}'", server.config.game_mode.name().to_uppercase(), mode.name().to_uppercase()));
    server.set_game_mode(mode);
    Ok(Value::String(server.config.game_mode.name().into()))
}

pub fn view_distance(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.view_distance)
}

pub fn set_view_distance(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let distance = decode_int(params)?;
    log(client, format_args!("Update view distance from {} to {distance}", server.config.view_distance));
    server.set_view_distance(distance);
    Ok(Value::from(server.config.view_distance))
}

pub fn simulation_distance(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.simulation_distance)
}

pub fn set_simulation_distance(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let distance = decode_int(params)?;
    log(client, format_args!("Update simulation distance from {} to {distance}", server.config.simulation_distance));
    server.set_simulation_distance(distance);
    Ok(Value::from(server.config.simulation_distance))
}

pub fn accept_transfers(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.accepts_transfers())
}

pub fn set_accept_transfers(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let accept = decode_bool(params)?;
    log(client, format_args!("Update accepts transfers from {} to {accept}", server.accepts_transfers()));
    server.set_accepts_transfers(accept);
    Ok(Value::Bool(server.accepts_transfers()))
}

pub fn status_heartbeat_interval(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.status_heartbeat_interval())
}

pub fn set_status_heartbeat_interval(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let seconds = decode_int(params)?;
    if server.set_status_heartbeat_interval(seconds) {
        log(client, format_args!("Updated status heartbeat interval from {} to {seconds}", server.status_heartbeat_interval()));
    } else {
        log(
            client,
            format_args!(
                "The heartbeat interval was not updated to the new value of {seconds} seconds as the json rpc server is null or a pre-existing heartbeat failed to get canceled"
            ),
        );
    }
    Ok(Value::from(server.status_heartbeat_interval()))
}

pub fn operator_user_permission_level(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.op_permission_level)
}

pub fn set_operator_user_permission_level(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let level = decode_int(params)?.clamp(0, 4);
    log(
        client,
        format_args!(
            "Update operator user permission level from permission level: {} to {}",
            PERMISSION_LEVEL_NAMES[server.config.op_permission_level.clamp(0, 4) as usize],
            PERMISSION_LEVEL_NAMES[level as usize]
        ),
    );
    server.set_operator_user_permission_level(level);
    Ok(Value::from(server.config.op_permission_level))
}

pub fn hides_online_players(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.config.hides_online_players)
}

pub fn set_hides_online_players(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let hide = decode_bool(params)?;
    log(client, format_args!("Update hides online players from {} to {hide}", server.config.hides_online_players));
    server.set_hides_online_players(hide);
    Ok(Value::Bool(server.config.hides_online_players))
}

pub fn replies_to_status(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Bool(server.replies_to_status())
}

pub fn set_replies_to_status(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let enable = decode_bool(params)?;
    log(client, format_args!("Update replies to status from {} to {enable}", server.replies_to_status()));
    server.set_replies_to_status(enable);
    Ok(Value::Bool(server.replies_to_status()))
}

pub fn entity_broadcast_range_percentage(server: &mut Server, _client: ClientInfo) -> Value {
    Value::from(server.config.entity_broadcast_range_percentage)
}

pub fn set_entity_broadcast_range_percentage(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let percentage = decode_int(params)?;
    log(client, format_args!("Update entity broadcast range percentage from {}% to {percentage}%", server.config.entity_broadcast_range_percentage));
    server.set_entity_broadcast_range_percentage(percentage);
    Ok(Value::from(server.config.entity_broadcast_range_percentage))
}
