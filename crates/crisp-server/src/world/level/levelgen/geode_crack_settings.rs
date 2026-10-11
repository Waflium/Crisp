use serde_json::Value;

pub struct GeodeCrackSettings {
    pub generate_crack_chance: f64,
    pub base_crack_size: f64,
    pub crack_point_offset: i32,
}

impl GeodeCrackSettings {
    pub fn parse(json: &Value) -> GeodeCrackSettings {
        GeodeCrackSettings {
            generate_crack_chance: json.get("generate_crack_chance").and_then(Value::as_f64).unwrap_or(1.0),
            base_crack_size: json.get("base_crack_size").and_then(Value::as_f64).unwrap_or(2.0),
            crack_point_offset: json.get("crack_point_offset").and_then(Value::as_i64).map_or(2, |v| v as i32),
        }
    }
}
