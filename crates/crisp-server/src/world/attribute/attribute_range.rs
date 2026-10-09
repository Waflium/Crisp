use super::Value;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AttributeRange {
    Any,
    Float(f32, f32),
}

impl AttributeRange {
    pub const UNIT_FLOAT: AttributeRange = AttributeRange::Float(0.0, 1.0);
    pub const UNIT_FLOAT_EPSILON: AttributeRange = AttributeRange::Float(0.0, 0.9999999);
    pub const NON_NEGATIVE_FLOAT: AttributeRange = AttributeRange::Float(0.0, f32::INFINITY);

    pub fn validate(self, value: &Value) -> Result<(), String> {
        match (self, value) {
            (AttributeRange::Float(min, max), Value::Float(v)) if !(*v >= min && *v <= max) => Err(format!("{v} is not in range [{min}; {max}]")),
            _ => Ok(()),
        }
    }

    pub fn sanitize(self, value: Value) -> Value {
        match (self, value) {
            (AttributeRange::Float(min, max), Value::Float(v)) if !(v >= min && v <= max) => {
                Value::Float(if v < min { min } else { crate::util::java_min_f(v, max) })
            }
            (_, value) => value,
        }
    }
}
