use std::sync::Arc;

use serde_json::Value;

use crate::server::level::ServerLevel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Always,
    WhenDark,
    Never,
}

impl Rule {
    fn parse(json: &Value) -> Result<Rule, String> {
        match json.as_str() {
            Some("always") => Ok(Rule::Always),
            Some("when_dark") => Ok(Rule::WhenDark),
            Some("never") => Ok(Rule::Never),
            _ => Err(format!("unknown bed rule {json}")),
        }
    }

    pub fn test(self, level: &ServerLevel) -> bool {
        match self {
            Rule::Always => true,
            Rule::WhenDark => level.is_dark_outside(),
            Rule::Never => false,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BedRule {
    pub can_sleep: Rule,
    pub can_set_spawn: Rule,
    pub destroy_on_use: bool,
    pub destroy_on_leave: bool,
    pub error_message: Option<Arc<Value>>,
}

fn no_sleep() -> Option<Arc<Value>> {
    Some(Arc::new(serde_json::json!({ "translate": "block.minecraft.bed.no_sleep" })))
}

impl BedRule {
    pub fn can_sleep_when_dark() -> BedRule {
        BedRule { can_sleep: Rule::WhenDark, can_set_spawn: Rule::Always, destroy_on_use: false, destroy_on_leave: false, error_message: no_sleep() }
    }

    pub fn destroy_on_leave() -> BedRule {
        BedRule { can_sleep: Rule::WhenDark, can_set_spawn: Rule::Never, destroy_on_use: false, destroy_on_leave: true, error_message: no_sleep() }
    }

    pub fn parse(json: &Value) -> Result<BedRule, String> {
        let flag = |key: &str| json.get(key).map_or(Ok(false), |v| v.as_bool().ok_or_else(|| format!("{key} must be a boolean")));
        Ok(BedRule {
            can_sleep: Rule::parse(json.get("can_sleep").ok_or("missing can_sleep")?)?,
            can_set_spawn: Rule::parse(json.get("can_set_spawn").ok_or("missing can_set_spawn")?)?,
            destroy_on_use: flag("destroy_on_use")?,
            destroy_on_leave: flag("destroy_on_leave")?,
            error_message: json.get("error_message").map(|m| Arc::new(m.clone())),
        })
    }

    pub fn can_sleep(&self, level: &ServerLevel) -> bool {
        self.can_sleep.test(level)
    }

    pub fn can_set_spawn(&self, level: &ServerLevel) -> bool {
        self.can_set_spawn.test(level)
    }
}
