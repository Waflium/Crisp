use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::java_string_hash;
use serde_json::{Map, Value};

#[derive(Clone, Debug)]
pub enum Schema {
    Ref(&'static str),
    Types(&'static [&'static str]),
    Enum(&'static [&'static str]),
    Array(Box<Schema>),
    Record(Vec<(&'static str, Schema)>),
}

pub const BOOL_SCHEMA: Schema = Schema::Types(&["boolean"]);
pub const INT_SCHEMA: Schema = Schema::Types(&["integer"]);
pub const BOOL_OR_INT_SCHEMA: Schema = Schema::Types(&["boolean", "integer"]);
pub const NUMBER_SCHEMA: Schema = Schema::Types(&["number"]);
pub const STRING_SCHEMA: Schema = Schema::Types(&["string"]);
pub const UUID_SCHEMA: Schema = Schema::Types(&["string"]);
pub const DISCOVERY_SCHEMA: Schema = Schema::Types(&["string"]);
pub const PERMISSION_LEVEL_SCHEMA: Schema = Schema::Types(&["integer"]);
pub const RULE_TYPE_SCHEMA: Schema = Schema::Enum(&["integer", "boolean"]);

pub struct SchemaComponent {
    pub name: &'static str,
    pub reference: &'static str,
    pub schema: fn() -> Schema,
}

impl SchemaComponent {
    pub fn as_ref(&self) -> Schema {
        Schema::Ref(self.reference)
    }

    pub fn as_array(&self) -> Schema {
        Schema::Array(Box::new(self.as_ref()))
    }
}

impl Schema {
    pub fn as_array(self) -> Schema {
        Schema::Array(Box::new(self))
    }

    pub fn to_json(&self) -> Value {
        let mut object = Map::new();
        match self {
            Schema::Ref(reference) => {
                object.insert("$ref".into(), Value::String(reference.to_string()));
            }
            Schema::Types(types) => {
                object.insert("type".into(), compact_list(types));
            }
            Schema::Enum(values) => {
                object.insert("type".into(), Value::String("string".into()));
                object.insert("enum".into(), Value::Array(values.iter().map(|v| Value::String(v.to_string())).collect()));
            }
            Schema::Array(items) => {
                object.insert("type".into(), Value::String("array".into()));
                object.insert("items".into(), items.to_json());
            }
            Schema::Record(fields) => {
                object.insert("type".into(), Value::String("object".into()));
                if !fields.is_empty() {
                    let hashes: Vec<i32> = fields.iter().map(|(name, _)| java_string_hash(name)).collect();
                    let properties = java_hash_map_order(&hashes).into_iter().map(|i| (fields[i].0.to_string(), fields[i].1.to_json())).collect();
                    object.insert("properties".into(), Value::Object(properties));
                }
            }
        }
        Value::Object(object)
    }
}

fn compact_list(values: &[&str]) -> Value {
    match values {
        [single] => Value::String(single.to_string()),
        _ => Value::Array(values.iter().map(|v| Value::String(v.to_string())).collect()),
    }
}

macro_rules! component {
    ($name:ident, $id:literal, $schema:expr) => {
        pub static $name: SchemaComponent = SchemaComponent { name: $id, reference: concat!("#/components/schemas/", $id), schema: || $schema };
    };
}

component!(DIFFICULTY_SCHEMA, "difficulty", Schema::Enum(&["peaceful", "easy", "normal", "hard"]));
component!(GAME_TYPE_SCHEMA, "game_type", Schema::Enum(&["survival", "creative", "adventure", "spectator"]));
component!(PLAYER_SCHEMA, "player", Schema::Record(vec![("id", UUID_SCHEMA), ("name", STRING_SCHEMA)]));
component!(VERSION_SCHEMA, "version", Schema::Record(vec![("name", STRING_SCHEMA), ("protocol", INT_SCHEMA)]));
component!(
    SERVER_STATE_SCHEMA,
    "server_state",
    Schema::Record(vec![("started", BOOL_SCHEMA), ("players", PLAYER_SCHEMA.as_array()), ("version", VERSION_SCHEMA.as_ref())])
);
component!(
    TYPED_GAME_RULE_SCHEMA,
    "typed_game_rule",
    Schema::Record(vec![("key", STRING_SCHEMA), ("value", BOOL_OR_INT_SCHEMA), ("type", RULE_TYPE_SCHEMA)])
);
component!(UNTYPED_GAME_RULE_SCHEMA, "untyped_game_rule", Schema::Record(vec![("key", STRING_SCHEMA), ("value", BOOL_OR_INT_SCHEMA)]));
component!(
    MESSAGE_SCHEMA,
    "message",
    Schema::Record(vec![("literal", STRING_SCHEMA), ("translatable", STRING_SCHEMA), ("translatableParams", STRING_SCHEMA.as_array())])
);
component!(
    SYSTEM_MESSAGE_SCHEMA,
    "system_message",
    Schema::Record(vec![("message", MESSAGE_SCHEMA.as_ref()), ("overlay", BOOL_SCHEMA), ("receivingPlayers", PLAYER_SCHEMA.as_array())])
);
component!(KICK_PLAYER_SCHEMA, "kick_player", Schema::Record(vec![("message", MESSAGE_SCHEMA.as_ref()), ("player", PLAYER_SCHEMA.as_ref())]));
component!(
    OPERATOR_SCHEMA,
    "operator",
    Schema::Record(vec![("player", PLAYER_SCHEMA.as_ref()), ("bypassesPlayerLimit", BOOL_SCHEMA), ("permissionLevel", INT_SCHEMA)])
);
component!(
    INCOMING_IP_BAN_SCHEMA,
    "incoming_ip_ban",
    Schema::Record(vec![
        ("player", PLAYER_SCHEMA.as_ref()),
        ("ip", STRING_SCHEMA),
        ("reason", STRING_SCHEMA),
        ("source", STRING_SCHEMA),
        ("expires", STRING_SCHEMA),
    ])
);
component!(
    IP_BAN_SCHEMA,
    "ip_ban",
    Schema::Record(vec![("ip", STRING_SCHEMA), ("reason", STRING_SCHEMA), ("source", STRING_SCHEMA), ("expires", STRING_SCHEMA)])
);
component!(
    PLAYER_BAN_SCHEMA,
    "user_ban",
    Schema::Record(vec![("player", PLAYER_SCHEMA.as_ref()), ("reason", STRING_SCHEMA), ("source", STRING_SCHEMA), ("expires", STRING_SCHEMA)])
);

pub static SCHEMA_REGISTRY: [&SchemaComponent; 14] = [
    &DIFFICULTY_SCHEMA,
    &GAME_TYPE_SCHEMA,
    &PLAYER_SCHEMA,
    &VERSION_SCHEMA,
    &SERVER_STATE_SCHEMA,
    &TYPED_GAME_RULE_SCHEMA,
    &UNTYPED_GAME_RULE_SCHEMA,
    &MESSAGE_SCHEMA,
    &SYSTEM_MESSAGE_SCHEMA,
    &KICK_PLAYER_SCHEMA,
    &OPERATOR_SCHEMA,
    &INCOMING_IP_BAN_SCHEMA,
    &IP_BAN_SCHEMA,
    &PLAYER_BAN_SCHEMA,
];
