use crate::network::chat::{Component, HoverEvent};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreHolder {
    pub name: String,
    pub entity: Option<EntityId>,
}

impl ScoreHolder {
    pub const WILDCARD_NAME: &str = "*";

    pub fn for_name_only(name: impl Into<String>) -> ScoreHolder {
        ScoreHolder { name: name.into(), entity: None }
    }

    pub fn for_entity(level: &ServerLevel, entity: EntityId) -> ScoreHolder {
        ScoreHolder { name: level.scoreboard_name(entity), entity: Some(entity) }
    }

    pub fn scoreboard_name(&self) -> &str {
        &self.name
    }

    pub fn is_wildcard(&self) -> bool {
        self.entity.is_none() && self.name == ScoreHolder::WILDCARD_NAME
    }

    pub fn display_name(&self, level: &ServerLevel) -> Option<Component> {
        self.entity.map(|e| level.display_name(e))
    }

    pub fn feedback_display_name(&self, level: &ServerLevel) -> Component {
        match self.display_name(level) {
            Some(display) => display.with_hover(HoverEvent::ShowText(Box::new(Component::literal(self.name.clone())))),
            None => Component::literal(self.name.clone()),
        }
    }
}
