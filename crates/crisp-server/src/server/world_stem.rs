use super::reloadable_server_resources::ReloadableServerResources;
use crate::world::level::storage::WorldData;

pub struct WorldStem {
    pub data_packs: ReloadableServerResources,
    pub world_data: WorldData,
}
