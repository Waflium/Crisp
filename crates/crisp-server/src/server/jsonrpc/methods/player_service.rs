use serde_json::Value;

use super::{ClientInfo, Message, decode_params, log};
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::jsonrpc::api::PlayerDto;
use crate::server::jsonrpc::codec::{self, DecodeResult, Fields};
use crate::server::players::NameAndId;

pub fn players(server: &Server) -> Vec<PlayerDto> {
    server.players().map(|p| PlayerDto::from(&NameAndId { id: p.profile.uuid, name: p.profile.name.clone() })).collect()
}

pub fn get(server: &mut Server, _client: ClientInfo) -> Value {
    Value::Array(players(server).iter().map(PlayerDto::encode).collect())
}

struct KickDto {
    player: PlayerDto,
    message: Option<Message>,
}

impl KickDto {
    fn decode(value: &Value) -> DecodeResult<KickDto> {
        let mut fields = Fields::of(value)?;
        let player = fields.required("player", PlayerDto::decode);
        let message = fields.optional("message", Message::decode);
        fields.finish(|| KickDto { player: player.unwrap(), message })
    }
}

pub fn kick(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let kick = decode_params(params, |v| codec::list(v, KickDto::decode))?;
    let mut kicked = Vec::new();
    for dto in kick {
        let found = match (dto.player.id, &dto.player.name) {
            (Some(id), _) => server.player_by_uuid(id),
            (None, Some(name)) => server.player_by_name(name),
            (None, None) => None,
        };
        let Some(player) = found else { continue };
        log(client, format_args!("Remove player '{}'", server.player(player).profile.name));
        let reason = dto.message.and_then(|m| m.as_component()).unwrap_or_else(|| Component::translatable("multiplayer.disconnect.kicked"));
        server.player_mut(player).disconnect_with(reason);
        kicked.push(dto.player.encode());
    }
    Ok(Value::Array(kicked))
}
