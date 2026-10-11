use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct Sensing {
    seen: Vec<EntityId>,
    unseen: Vec<EntityId>,
}

impl Sensing {
    pub fn tick(&mut self) {
        self.seen.clear();
        self.unseen.clear();
    }
}

impl ServerLevel {
    pub fn sensing_has_line_of_sight(&mut self, mob: EntityId, target: EntityId) -> bool {
        let sensing = &self.mob(mob).expect("a mob").ai.sensing;
        if sensing.seen.contains(&target) {
            return true;
        }
        if sensing.unseen.contains(&target) {
            return false;
        }
        let has_line_of_sight = self.has_line_of_sight(mob, target);
        let sensing = &mut self.mob_mut(mob).expect("a mob").ai.sensing;
        if has_line_of_sight {
            sensing.seen.push(target);
        } else {
            sensing.unseen.push(target);
        }
        has_line_of_sight
    }
}
