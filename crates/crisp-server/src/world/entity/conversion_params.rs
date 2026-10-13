use super::EntityId;
use super::conversion_type::ConversionType;
use crate::server::level::ServerLevel;

pub struct ConversionParams {
    pub ty: ConversionType,
    pub keep_equipment: bool,
    pub preserve_can_pick_up_loot: bool,
    pub team: Option<String>,
}

impl ConversionParams {
    pub fn single(level: &ServerLevel, mob: EntityId, keep_equipment: bool, preserve_can_pick_up_loot: bool) -> ConversionParams {
        let team = level.team_of(mob).map(|team| team.name().to_string());
        ConversionParams { ty: ConversionType::Single, keep_equipment, preserve_can_pick_up_loot, team }
    }
}
