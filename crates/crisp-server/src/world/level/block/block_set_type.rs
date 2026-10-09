use crisp_data::{Block, registry, registry_id};
use serde_json::Value;

fn sound(value: &Value) -> i32 {
    registry_id(registry::SOUND_EVENT, value.as_str().expect("sound event name"))
}

pub fn fields(block: Block) -> Value {
    serde_json::from_str(block.info().fields).expect("block fields are JSON")
}

pub struct BlockSetType {
    pub can_open_by_hand: bool,
    pub door_close: i32,
    pub door_open: i32,
    pub trapdoor_close: i32,
    pub trapdoor_open: i32,
}

impl BlockSetType {
    pub fn from_json(json: &Value) -> BlockSetType {
        BlockSetType {
            can_open_by_hand: json["canOpenByHand"].as_bool().expect("canOpenByHand"),
            door_close: sound(&json["doorClose"]),
            door_open: sound(&json["doorOpen"]),
            trapdoor_close: sound(&json["trapdoorClose"]),
            trapdoor_open: sound(&json["trapdoorOpen"]),
        }
    }
}

pub struct WoodType {
    pub fence_gate_close: i32,
    pub fence_gate_open: i32,
}

impl WoodType {
    pub fn from_json(json: &Value) -> WoodType {
        WoodType { fence_gate_close: sound(&json["fenceGateClose"]), fence_gate_open: sound(&json["fenceGateOpen"]) }
    }
}
