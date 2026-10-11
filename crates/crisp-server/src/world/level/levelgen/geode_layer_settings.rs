use serde_json::Value;

pub struct GeodeLayerSettings {
    pub filling: f64,
    pub inner_layer: f64,
    pub middle_layer: f64,
    pub outer_layer: f64,
}

impl GeodeLayerSettings {
    pub fn parse(json: &Value) -> GeodeLayerSettings {
        let layer = |key: &str, default: f64| json.get(key).and_then(Value::as_f64).unwrap_or(default);
        GeodeLayerSettings { filling: layer("filling", 1.7), inner_layer: layer("inner_layer", 2.2), middle_layer: layer("middle_layer", 3.2), outer_layer: layer("outer_layer", 4.2) }
    }
}
