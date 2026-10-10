pub mod schema;

use serde_json::{Map, Value};

use super::codec::{self, DecodeResult, Fields};
use crate::server::players::NameAndId;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlayerDto {
    pub id: Option<u128>,
    pub name: Option<String>,
}

impl PlayerDto {
    pub fn decode(value: &Value) -> DecodeResult<PlayerDto> {
        let mut fields = Fields::of(value)?;
        let id = fields.optional("id", codec::uuid);
        let name = fields.optional("name", codec::string);
        fields.finish(|| PlayerDto { id, name })
    }

    pub fn encode(&self) -> Value {
        let mut object = Map::new();
        if let Some(id) = self.id {
            object.insert("id".into(), codec::encode_uuid(id));
        }
        if let Some(name) = &self.name {
            object.insert("name".into(), Value::String(name.clone()));
        }
        Value::Object(object)
    }

    pub fn from(user: &NameAndId) -> PlayerDto {
        PlayerDto { id: Some(user.id), name: Some(user.name.clone()) }
    }
}
