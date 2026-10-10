use crisp_core::{BlockPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;

use crate::network::connection::Packet;
use crate::server::level::game_mode::GameType;
use crate::server::level::player::ServerPlayer;
use crate::server::{GAME_EVENT_LEVEL_CHUNKS_LOAD_START, OVERWORLD, Server, hashed_seed};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::{DataValue, ids};
use crate::world::entity::{EntityId, RemovalReason};
use crate::world::level::BlockGetter;
use crate::world::level::border::border_change_listener::write_initialize_border;

const PERFORM_RESPAWN: i32 = 0;
const GAME_EVENT_NO_RESPAWN_BLOCK_AVAILABLE: u8 = 0;

pub fn write_death_location(p: &mut Writer, location: &Option<(String, BlockPos)>) {
    match location {
        Some((dimension, pos)) => {
            p.bool(true);
            p.string(dimension);
            p.position(pos.x, pos.y, pos.z);
        }
        None => p.bool(false),
    }
}

struct RespawnTransition {
    position: Vec3,
    y_rot: f32,
    x_rot: f32,
    missing_respawn_block: bool,
}

impl Server {
    pub(in crate::server) fn handle_client_command(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        self.level.player_mut(id).reset_last_action_time();
        if packet.reader().varint()? != PERFORM_RESPAWN {
            return Ok(());
        }
        if self.level.player(id).health > 0.0 {
            return Ok(());
        }
        self.respawn(id);
        let p = self.level.player_mut(id);
        p.reset_position();
        p.movement.waiting_for_respawn = false;
        p.movement.client_loaded_timeout_timer = 60;
        if self.hardcore {
            self.level.set_game_mode(id, GameType::Spectator);
        }
        Ok(())
    }

    fn find_respawn_position(&mut self, id: EntityId) -> RespawnTransition {
        let config = self.level.player(id).respawn_config.clone();
        if let Some(config) = config.filter(|c| c.respawn_data.dimension == OVERWORLD) {
            let data = &config.respawn_data;
            let pos = data.pos;
            if config.forced {
                let bottom = self.level.block_state(pos);
                let top = self.level.block_state(pos.above());
                if is_possible_to_respawn_in_this(bottom) && is_possible_to_respawn_in_this(top) {
                    let position = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.1, pos.z as f64 + 0.5);
                    return RespawnTransition { position, y_rot: data.yaw, x_rot: data.pitch, missing_respawn_block: false };
                }
            }
            return self.default_respawn(true);
        }
        self.default_respawn(false)
    }

    fn default_respawn(&mut self, missing_respawn_block: bool) -> RespawnTransition {
        let spawn = self.respawn_data().clone();
        let adjusted = self.level.find_spawn(spawn.pos, self.default_game_type == GameType::Adventure);
        let position = Vec3::at_bottom_center_of(BlockPos::containing(adjusted.x, adjusted.y, adjusted.z));
        RespawnTransition { position, y_rot: spawn.yaw, x_rot: spawn.pitch, missing_respawn_block }
    }

    pub fn respawn(&mut self, id: EntityId) {
        let transition = self.find_respawn_position(id);
        self.level.remove_player_from_level(id, RemovalReason::Killed);
        let old = self.level.players.shift_remove(&id).expect("respawning player");
        let old_score = old.score();
        let mut player = ServerPlayer::new(old.profile.clone(), old.client, old.connection.clone(), BlockPos::ZERO);
        player.entity.id = id;
        player.chunk_sender = old.chunk_sender;
        player.movement = old.movement;
        player.waypoints = old.waypoints;
        player.ack_block_changes_up_to = old.ack_block_changes_up_to;
        player.keep_alive_time = old.keep_alive_time;
        player.keep_alive_challenge = old.keep_alive_challenge;
        player.latency = old.latency;
        player.chat = old.chat;
        player.chat_session = old.chat_session;
        player.disconnected = old.disconnected;
        player.set_game_mode_for_player(old.game_mode.game_type, old.game_mode.previous_game_type);
        player.connection.send(clientbound::PLAYER_ABILITIES, |p| player.abilities.write(p));
        player.living.attributes.assign_base_values(&old.living.attributes);
        player.health = player.living.attributes.value(Attribute::MaxHealth) as f32;
        player.entity.data.set(ids::HEALTH, DataValue::Float(player.health));
        if self.game_rules.get_bool(crisp_data::game_rule::KEEP_INVENTORY) || old.game_mode.game_type == GameType::Spectator {
            for slot in 5..crate::world::item::SLOTS {
                player.inventory.slots[slot] = old.inventory.slots[slot].clone();
            }
            player.inventory.selected = old.inventory.selected;
            player.experience.level = old.experience.level;
            player.experience.total = old.experience.total;
            player.experience.progress = old.experience.progress;
            player.set_score(old_score);
        }
        player.experience.enchantment_seed = old.experience.enchantment_seed;
        player.last_sent_exp = -1;
        player.last_sent_health = -1.0;
        player.last_sent_food = -1;
        player.chunk_tracking_view = old.chunk_tracking_view;
        player.last_death_location = old.last_death_location.clone();
        if !transition.missing_respawn_block {
            player.respawn_config = old.respawn_config.clone();
        }
        let e = &mut player.entity;
        e.set_pos_raw(transition.position);
        e.set_y_rot(transition.y_rot);
        e.set_x_rot(transition.x_rot);
        e.set_old_pos();
        e.reapply_position();
        e.y_head_rot = e.y_rot;
        let out = player.connection.clone();
        if transition.missing_respawn_block {
            out.send(clientbound::GAME_EVENT, |p| {
                p.u8(GAME_EVENT_NO_RESPAWN_BLOCK_AVAILABLE);
                p.f32(0.0);
            });
        }
        let level = &self.level;
        out.send(clientbound::RESPAWN, |p| {
            p.varint(level.dimension_type_id);
            p.string(level.dimension);
            p.i64(hashed_seed(self.seed));
            p.varint(player.game_mode.game_type.id());
            p.varint(player.game_mode.previous_game_type.map_or(0, |g| g.id() + 1));
            p.bool(false);
            p.bool(level.chunks.generator.is_flat());
            write_death_location(p, &player.last_death_location);
            p.varint(0);
            p.varint(level.chunks.generator.sea_level());
            p.u8(0);
        });
        let (pos, y_rot, x_rot) = (player.entity.position(), player.entity.y_rot, player.entity.x_rot);
        player.teleport(pos, y_rot, x_rot);
        out.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| self.write_spawn(p));
        self.send_difficulty_update(&out);
        player.send_experience();
        out.send(clientbound::POST_EFFECTS, |p| p.varint(0));
        out.send(clientbound::INITIALIZE_BORDER, |p| write_initialize_border(p, &self.level.world_border));
        out.send(clientbound::SET_TIME, |p| self.full_time_sync(p));
        out.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| self.write_spawn(p));
        self.level.write_weather_for_joining(&out);
        out.send(clientbound::GAME_EVENT, |p| {
            p.u8(GAME_EVENT_LEVEL_CHUNKS_LOAD_START);
            p.f32(0.0);
        });
        out.send(clientbound::TICKING_STATE, |p| {
            p.f32(20.0);
            p.bool(false);
        });
        out.send(clientbound::TICKING_STEP, |p| p.varint(0));
        let permission = self.profile_permission_level(player.profile.uuid);
        out.send(clientbound::ENTITY_EVENT, |p| {
            p.i32(id);
            p.u8(crate::server::EVENT_PERMISSION_LEVEL_ALL + permission as u8);
        });
        out.send(clientbound::COMMANDS, |p| crate::commands::write_commands_packet(p, &crate::commands::source::CommandSourceStack::for_new_player(self, &player)));
        self.level.add_player(player);
        self.level.player(id).send_window_content();
    }
}

fn is_possible_to_respawn_in_this(state: crisp_data::BlockState) -> bool {
    let block = state.block();
    if block.is_instance_of("BasePressurePlateBlock") || block.is_instance_of("AbstractBannerBlock") || block.is_instance_of("SignBlock") {
        return true;
    }
    !state.is_solid() && !state.liquid()
}

#[cfg(test)]
mod tests {
    use crisp_core::Vec3;
    use crisp_data::packets::play::{clientbound, serverbound};
    use crisp_data::{game_rule, items};
    use crisp_protocol::Reader;

    use crate::ServerConfig;
    use crate::network::connection::{Outbound, Packet};
    use crate::server::Server;
    use crate::server::level::testing;
    use crate::world::item::ItemStack;

    fn ids(packets: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<i32> {
        let mut out = Vec::new();
        while let Ok(packet) = packets.try_recv() {
            out.push(Reader::new(&packet).varint().unwrap());
        }
        out
    }

    fn perform_respawn() -> Packet {
        Packet { id: serverbound::CLIENT_COMMAND, data: vec![0] }
    }

    #[test]
    fn death_and_respawn_send_what_vanilla_sends() {
        let mut server = Server::new(ServerConfig::default()).unwrap();
        server.game_rules.set(game_rule::KEEP_INVENTORY, 1);
        let id = testing::player(&mut server.level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let (out, mut packets) = Outbound::capture();
        let p = server.level.player_mut(id);
        p.connection = out;
        p.inventory.slots[36] = Some(ItemStack::new(items::STONE, 3));
        server.level.handle_player_loaded(id);
        server.handle_client_command(id, &perform_respawn()).unwrap();
        assert_eq!(ids(&mut packets), Vec::<i32>::new());
        server.level.kill(id);
        let died = ids(&mut packets);
        assert_eq!(died.first(), Some(&clientbound::DAMAGE_EVENT));
        assert_eq!(&died[1..], [clientbound::PLAYER_COMBAT_KILL, clientbound::SYSTEM_CHAT, clientbound::ENTITY_EVENT]);
        assert!(server.level.player(id).movement.waiting_for_respawn);
        for _ in 0..20 {
            server.level.tick();
            server.level.tick_connection(id);
        }
        assert!(server.level.entity(id).is_removed());
        ids(&mut packets);
        server.handle_client_command(id, &perform_respawn()).unwrap();
        assert_eq!(
            ids(&mut packets),
            [
                clientbound::PLAYER_ABILITIES,
                clientbound::RESPAWN,
                clientbound::PLAYER_POSITION,
                clientbound::SET_DEFAULT_SPAWN_POSITION,
                clientbound::CHANGE_DIFFICULTY,
                clientbound::SET_EXPERIENCE,
                clientbound::POST_EFFECTS,
                clientbound::INITIALIZE_BORDER,
                clientbound::SET_TIME,
                clientbound::SET_DEFAULT_SPAWN_POSITION,
                clientbound::GAME_EVENT,
                clientbound::TICKING_STATE,
                clientbound::TICKING_STEP,
                clientbound::ENTITY_EVENT,
                clientbound::COMMANDS,
                clientbound::SET_CHUNK_CACHE_CENTER,
                clientbound::CONTAINER_SET_CONTENT,
            ]
        );
        let p = server.level.player(id);
        assert_eq!((p.entity.id, p.health, p.entity.is_removed()), (id, 20.0, false));
        assert_eq!(p.inventory.slots[36], Some(ItemStack::new(items::STONE, 3)));
        assert_eq!(p.last_death_location.as_ref().map(|l| l.1), Some(crisp_core::BlockPos::new(0, -60, 0)));
        assert!(!p.movement.waiting_for_respawn);
        server.level.tick();
        server.level.tick_connection(id);
        let next = ids(&mut packets);
        let health = next.iter().position(|&i| i == clientbound::SET_HEALTH).unwrap();
        assert_eq!(next[health + 1], clientbound::SET_EXPERIENCE);
    }
}
