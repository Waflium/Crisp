use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::java_string_hash;
use serde_json::{Map, Value};

use crate::server::jsonrpc::api::schema::SCHEMA_REGISTRY;
use crate::server::jsonrpc::incoming_rpc_methods::INCOMING_RPC_METHODS;
use crate::server::jsonrpc::outgoing_rpc_methods::OUTGOING_RPC_METHODS;

pub fn discover() -> Value {
    let mut methods: Vec<Value> = INCOMING_RPC_METHODS.iter().filter(|m| m.discoverable).map(|m| m.info()).collect();
    methods.extend(OUTGOING_RPC_METHODS.iter().map(|m| m.info()));
    let hashes: Vec<i32> = SCHEMA_REGISTRY.iter().map(|c| java_string_hash(c.name)).collect();
    let schemas: Map<String, Value> =
        java_hash_map_order(&hashes).into_iter().map(|i| (SCHEMA_REGISTRY[i].name.to_string(), (SCHEMA_REGISTRY[i].schema)().to_json())).collect();
    let mut info = Map::new();
    info.insert("title".into(), Value::String("Minecraft Server JSON-RPC".into()));
    info.insert("version".into(), Value::String("3.1.0".into()));
    let mut components = Map::new();
    components.insert("schemas".into(), Value::Object(schemas));
    let mut response = Map::new();
    response.insert("openrpc".into(), Value::String("1.3.2".into()));
    response.insert("info".into(), Value::Object(info));
    response.insert("methods".into(), Value::Array(methods));
    response.insert("components".into(), Value::Object(components));
    Value::Object(response)
}
