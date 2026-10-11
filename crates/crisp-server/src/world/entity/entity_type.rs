use crate::world::difficulty::Difficulty;

pub struct EntityType {
    pub id: &'static str,
    allowed_in_peaceful: bool,
}

impl EntityType {
    pub const fn new(id: &'static str, allowed_in_peaceful: bool) -> EntityType {
        EntityType { id, allowed_in_peaceful }
    }

    pub fn can_spawn(&self, difficulty: Difficulty) -> bool {
        self.is_allowed_in_peaceful() || difficulty != Difficulty::Peaceful
    }

    pub fn is_allowed_in_peaceful(&self) -> bool {
        self.allowed_in_peaceful
    }
}
