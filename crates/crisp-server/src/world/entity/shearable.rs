use super::EntityId;
use super::mob::MobKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::ItemStack;

impl ServerLevel {
    pub fn is_shearable(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Sheep(_) | MobKind::MushroomCow(_)))
    }

    pub fn shear(&mut self, id: EntityId, sound_source: SoundSource, tool: &ItemStack) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Sheep(_)) => self.sheep_shear(id, sound_source, tool),
            Some(MobKind::MushroomCow(_)) => self.mushroom_cow_shear(id, sound_source, tool),
            _ => {}
        }
    }

    pub fn ready_for_shearing(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Sheep(_)) => self.sheep_ready_for_shearing(id),
            Some(MobKind::MushroomCow(_)) => self.mushroom_cow_ready_for_shearing(id),
            _ => false,
        }
    }
}
