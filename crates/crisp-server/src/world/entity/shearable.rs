use super::EntityId;
use super::mob::MobKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::ItemStack;

impl ServerLevel {
    pub fn is_shearable(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::MushroomCow(_)))
    }

    pub fn shear(&mut self, id: EntityId, sound_source: SoundSource, tool: &ItemStack) {
        if let Some(MobKind::MushroomCow(_)) = self.mob(id).map(|mob| &mob.kind) {
            self.mushroom_cow_shear(id, sound_source, tool);
        }
    }

    pub fn ready_for_shearing(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::MushroomCow(_)) => self.mushroom_cow_ready_for_shearing(id),
            _ => false,
        }
    }
}
