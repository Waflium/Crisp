use serde_json::Value;

pub struct FeatureSize {
    pub min_clipped_height: Option<i32>,
    pub kind: FeatureSizeType,
}

pub enum FeatureSizeType {
    TwoLayers { limit: i32, lower_size: i32, upper_size: i32 },
    ThreeLayers { limit: i32, upper_limit: i32, lower_size: i32, middle_size: i32, upper_size: i32 },
}

impl FeatureSize {
    pub fn parse(json: &Value) -> Result<FeatureSize, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("feature size without type")?;
        let int = |key: &str, default: i32| json.get(key).and_then(Value::as_i64).map_or(default, |v| v as i32);
        let kind = match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "two_layers_feature_size" => FeatureSizeType::TwoLayers { limit: int("limit", 1), lower_size: int("lower_size", 0), upper_size: int("upper_size", 1) },
            "three_layers_feature_size" => FeatureSizeType::ThreeLayers {
                limit: int("limit", 1),
                upper_limit: int("upper_limit", 1),
                lower_size: int("lower_size", 0),
                middle_size: int("middle_size", 1),
                upper_size: int("upper_size", 1),
            },
            other => return Err(format!("unknown feature size type {other}")),
        };
        Ok(FeatureSize { min_clipped_height: json.get("min_clipped_height").and_then(Value::as_i64).map(|v| v as i32), kind })
    }

    pub fn get_size_at_height(&self, tree_height: i32, yo: i32) -> i32 {
        match self.kind {
            FeatureSizeType::TwoLayers { limit, lower_size, upper_size } => {
                if yo < limit {
                    lower_size
                } else {
                    upper_size
                }
            }
            FeatureSizeType::ThreeLayers { limit, upper_limit, lower_size, middle_size, upper_size } => {
                if yo < limit {
                    lower_size
                } else if yo >= tree_height - upper_limit {
                    upper_size
                } else {
                    middle_size
                }
            }
        }
    }

    pub fn min_clipped_height(&self) -> Option<i32> {
        self.min_clipped_height
    }
}
