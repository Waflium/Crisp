use crisp_core::{BlockPos, ChunkPos, Vec3};
use crisp_protocol::nbt::Nbt;
use tokio::sync::oneshot;

use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::server::level::ticket::TicketType;

const SPAWN_RADIUS: i32 = 3;

pub(crate) struct PrepareSpawnTask {
    pub uuid: u128,
    pub position: Vec3,
    pub angle: (f32, f32),
    pub reply: Option<oneshot::Sender<()>>,
    chunk_load_requested: bool,
}

impl PrepareSpawnTask {
    fn spawn_chunk(&self) -> ChunkPos {
        BlockPos::containing(self.position.x, self.position.y, self.position.z).chunk()
    }
}

impl Server {
    pub(crate) fn load_player_data(&self, name: &str, uuid: u128) -> Option<Nbt> {
        let dir = self.world_dir.as_ref()?;
        match crate::world::level::storage::read_dat(&dir.player(uuid)) {
            Ok(nbt) => nbt,
            Err(_) => {
                warn!("Failed to load player data for {name}");
                None
            }
        }
    }

    pub(crate) fn spawn_position(&mut self, saved: Option<&Nbt>) -> (Vec3, f32, f32) {
        let saved_position = saved.and_then(|nbt| nbt.get("Pos")).map(Nbt::as_list).filter(|p| p.len() == 3).and_then(|p| {
            Some(Vec3::new(p[0].as_f64()?, p[1].as_f64()?, p[2].as_f64()?))
        });
        let saved_rotation = saved.and_then(|nbt| nbt.get("Rotation")).map(Nbt::as_list).filter(|r| r.len() == 2).and_then(|r| {
            Some((r[0].as_f64()? as f32, r[1].as_f64()? as f32))
        });
        let position = saved_position.unwrap_or_else(|| self.level.find_spawn(self.spawn, self.default_game_type == GameType::Adventure));
        let (y_rot, x_rot) = saved_rotation.unwrap_or((self.spawn_yaw, self.spawn_pitch));
        (position, y_rot, x_rot)
    }

    pub(crate) fn start_prepare_spawn(&mut self, uuid: u128, name: &str, reply: oneshot::Sender<()>) {
        let saved = self.load_player_data(name, uuid);
        let (position, y_rot, x_rot) = self.spawn_position(saved.as_ref());
        self.prepare_spawn_tasks.push(PrepareSpawnTask { uuid, position, angle: (y_rot, x_rot), reply: Some(reply), chunk_load_requested: false });
    }

    pub(crate) fn tick_prepare_spawns(&mut self) {
        let mut index = 0;
        while index < self.prepare_spawn_tasks.len() {
            let spawn_chunk = self.prepare_spawn_tasks[index].spawn_chunk();
            let task = &mut self.prepare_spawn_tasks[index];
            if task.reply.is_none() {
                self.level.chunks.add_ticket_with_radius(TicketType::PlayerSpawn, spawn_chunk, SPAWN_RADIUS);
                index += 1;
                continue;
            }
            if !task.chunk_load_requested {
                task.chunk_load_requested = true;
                self.level.chunks.add_ticket_with_radius(TicketType::PlayerSpawn, spawn_chunk, SPAWN_RADIUS);
            }
            let loaded = (-SPAWN_RADIUS..=SPAWN_RADIUS)
                .all(|dx| (-SPAWN_RADIUS..=SPAWN_RADIUS).all(|dz| self.level.chunks.has_chunk(ChunkPos::new(spawn_chunk.x + dx, spawn_chunk.z + dz))));
            let task = &mut self.prepare_spawn_tasks[index];
            if loaded && let Some(reply) = task.reply.take() {
                if reply.send(()).is_err() {
                    self.prepare_spawn_tasks.swap_remove(index);
                    continue;
                }
            }
            index += 1;
        }
    }
}
