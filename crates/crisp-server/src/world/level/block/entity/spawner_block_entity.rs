use crisp_protocol::nbt::Nbt;

use crate::world::level::base_spawner::BaseSpawner;

#[derive(Clone, Default)]
pub struct SpawnerBlockEntity {
    pub spawner: BaseSpawner,
}

impl SpawnerBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.spawner.load(input);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.spawner.save(output);
    }
}
