use std::sync::Arc;

use crisp_data::registry;

use super::team_color::TeamColor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderType {
    Integer,
    Hearts,
}

impl RenderType {
    pub const VALUES: [RenderType; 2] = [RenderType::Integer, RenderType::Hearts];

    pub fn id(self) -> &'static str {
        match self {
            RenderType::Integer => "integer",
            RenderType::Hearts => "hearts",
        }
    }

    pub fn by_name(name: &str) -> Option<RenderType> {
        RenderType::VALUES.into_iter().find(|r| r.id() == name)
    }
}

static CUSTOM: [(&str, bool, RenderType); 43] = {
    use RenderType::{Hearts, Integer};
    [
        ("dummy", false, Integer),
        ("trigger", false, Integer),
        ("deathCount", false, Integer),
        ("playerKillCount", false, Integer),
        ("totalKillCount", false, Integer),
        ("health", true, Hearts),
        ("food", true, Integer),
        ("air", true, Integer),
        ("armor", true, Integer),
        ("xp", true, Integer),
        ("level", true, Integer),
        ("teamkill.black", false, Integer),
        ("teamkill.dark_blue", false, Integer),
        ("teamkill.dark_green", false, Integer),
        ("teamkill.dark_aqua", false, Integer),
        ("teamkill.dark_red", false, Integer),
        ("teamkill.dark_purple", false, Integer),
        ("teamkill.gold", false, Integer),
        ("teamkill.gray", false, Integer),
        ("teamkill.dark_gray", false, Integer),
        ("teamkill.blue", false, Integer),
        ("teamkill.green", false, Integer),
        ("teamkill.aqua", false, Integer),
        ("teamkill.red", false, Integer),
        ("teamkill.light_purple", false, Integer),
        ("teamkill.yellow", false, Integer),
        ("teamkill.white", false, Integer),
        ("killedByTeam.black", false, Integer),
        ("killedByTeam.dark_blue", false, Integer),
        ("killedByTeam.dark_green", false, Integer),
        ("killedByTeam.dark_aqua", false, Integer),
        ("killedByTeam.dark_red", false, Integer),
        ("killedByTeam.dark_purple", false, Integer),
        ("killedByTeam.gold", false, Integer),
        ("killedByTeam.gray", false, Integer),
        ("killedByTeam.dark_gray", false, Integer),
        ("killedByTeam.blue", false, Integer),
        ("killedByTeam.green", false, Integer),
        ("killedByTeam.aqua", false, Integer),
        ("killedByTeam.red", false, Integer),
        ("killedByTeam.light_purple", false, Integer),
        ("killedByTeam.yellow", false, Integer),
        ("killedByTeam.white", false, Integer),
    ]
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ObjectiveCriteria {
    Custom(u8),
    Stat(Arc<str>),
}

impl ObjectiveCriteria {
    pub const DUMMY: ObjectiveCriteria = ObjectiveCriteria::Custom(0);
    pub const TRIGGER: ObjectiveCriteria = ObjectiveCriteria::Custom(1);
    pub const DEATH_COUNT: ObjectiveCriteria = ObjectiveCriteria::Custom(2);
    pub const KILL_COUNT_PLAYERS: ObjectiveCriteria = ObjectiveCriteria::Custom(3);
    pub const KILL_COUNT_ALL: ObjectiveCriteria = ObjectiveCriteria::Custom(4);
    pub const HEALTH: ObjectiveCriteria = ObjectiveCriteria::Custom(5);
    pub const FOOD: ObjectiveCriteria = ObjectiveCriteria::Custom(6);
    pub const AIR: ObjectiveCriteria = ObjectiveCriteria::Custom(7);
    pub const ARMOR: ObjectiveCriteria = ObjectiveCriteria::Custom(8);
    pub const EXPERIENCE: ObjectiveCriteria = ObjectiveCriteria::Custom(9);
    pub const LEVEL: ObjectiveCriteria = ObjectiveCriteria::Custom(10);

    pub fn team_kill(color: TeamColor) -> ObjectiveCriteria {
        ObjectiveCriteria::Custom(11 + color.id() as u8)
    }

    pub fn killed_by_team(color: TeamColor) -> ObjectiveCriteria {
        ObjectiveCriteria::Custom(27 + color.id() as u8)
    }

    pub fn name(&self) -> &str {
        match self {
            ObjectiveCriteria::Custom(index) => CUSTOM[*index as usize].0,
            ObjectiveCriteria::Stat(name) => name,
        }
    }

    pub fn is_read_only(&self) -> bool {
        match self {
            ObjectiveCriteria::Custom(index) => CUSTOM[*index as usize].1,
            ObjectiveCriteria::Stat(_) => false,
        }
    }

    pub fn default_render_type(&self) -> RenderType {
        match self {
            ObjectiveCriteria::Custom(index) => CUSTOM[*index as usize].2,
            ObjectiveCriteria::Stat(_) => RenderType::Integer,
        }
    }

    pub fn by_name(name: &str) -> Result<Option<ObjectiveCriteria>, String> {
        if let Some(index) = CUSTOM.iter().position(|c| c.0 == name) {
            return Ok(Some(ObjectiveCriteria::Custom(index as u8)));
        }
        let Some(colon) = name.find(':') else { return Ok(None) };
        let stat_type = crate::resources::identifier::by_separator(&name[..colon], '.')?;
        let Some(values) = stat_type_registry(&stat_type) else { return Ok(None) };
        let value = crate::resources::identifier::by_separator(&name[colon + 1..], '.')?;
        if !values.contains(&value.as_str()) {
            return Ok(None);
        }
        Ok(Some(ObjectiveCriteria::Stat(format!("{}:{}", stat_type.replace(':', "."), value.replace(':', ".")).into())))
    }

    pub fn from_codec(name: &str) -> Result<ObjectiveCriteria, String> {
        match ObjectiveCriteria::by_name(name) {
            Ok(Some(criteria)) => Ok(criteria),
            _ => Err(format!("No scoreboard criteria with name: {name}")),
        }
    }
}


fn stat_type_registry(stat_type: &str) -> Option<&'static [&'static str]> {
    Some(match stat_type {
        "minecraft:mined" => registry::BLOCK,
        "minecraft:crafted" | "minecraft:used" | "minecraft:broken" | "minecraft:picked_up" | "minecraft:dropped" => registry::ITEM,
        "minecraft:killed" | "minecraft:killed_by" => registry::ENTITY_TYPE,
        "minecraft:custom" => registry::CUSTOM_STAT,
        _ => return None,
    })
}

pub fn all_names() -> Vec<String> {
    let mut names: Vec<String> = CUSTOM.iter().map(|c| c.0.to_string()).collect();
    for stat_type in registry::STAT_TYPE {
        if let Some(values) = stat_type_registry(stat_type) {
            let type_key = stat_type.replace(':', ".");
            names.extend(values.iter().map(|value| format!("{type_key}:{}", value.replace(':', "."))));
        }
    }
    names
}
