use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TriState {
    True,
    False,
    Default,
}

impl TriState {
    pub fn from(value: bool) -> TriState {
        if value { TriState::True } else { TriState::False }
    }

    pub fn to_boolean(self, default_value: bool) -> bool {
        match self {
            TriState::True => true,
            TriState::False => false,
            TriState::Default => default_value,
        }
    }

    pub fn parse(json: &Value) -> Result<TriState, String> {
        match json {
            Value::Bool(b) => Ok(TriState::from(*b)),
            Value::String(s) => match s.as_str() {
                "true" => Ok(TriState::True),
                "false" => Ok(TriState::False),
                "default" => Ok(TriState::Default),
                _ => Err(format!("unknown tri-state {s}")),
            },
            _ => Err(format!("not a tri-state: {json}")),
        }
    }
}
