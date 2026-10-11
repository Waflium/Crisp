use crate::world::difficulty::Difficulty;

pub struct EntityType {
    allowed_in_peaceful: bool,
}

impl EntityType {
    pub const fn new(allowed_in_peaceful: bool) -> EntityType {
        EntityType { allowed_in_peaceful }
    }

    pub fn can_spawn(&self, difficulty: Difficulty) -> bool {
        self.is_allowed_in_peaceful() || difficulty != Difficulty::Peaceful
    }

    pub fn is_allowed_in_peaceful(&self) -> bool {
        self.allowed_in_peaceful
    }
}
