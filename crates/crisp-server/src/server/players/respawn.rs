use crisp_core::{BlockPos, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;

use crate::network::connection::{Outbound, Packet};
use crate::server::level::game_mode::GameType;
use crate::server::level::server_player::{RespawnConfig, ServerPlayer};
use crate::server::{GAME_EVENT_LEVEL_CHUNKS_LOAD_START, OVERWORLD, Server, hashed_seed};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::{DataValue, Pose, ids};
use crate::world::entity::player::player::get_default_dimensions;
use crate::world::entity::vehicle::dismount_helper::DismountType;
use crate::world::entity::living_entity::update_mob_effect;
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::world::entity::{EntityId, RemovalReason};
use crisp_core::mth;
use crate::world::level::BlockGetter;
use crate::world::level::block::{abstract_bed_block, respawn_anchor_block};
use crate::world::level::update_flags::UPDATE_ALL;
use crisp_core::random::RandomSource;
use crisp_data::properties::{HORIZONTAL_FACING, RESPAWN_ANCHOR_CHARGES};
use crate::world::level::border::border_change_listener::write_initialize_border;

const PERFORM_RESPAWN: i32 = 0;
const REQUEST_STATS: i32 = 1;
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
    level: usize,
    position: Vec3,
    y_rot: f32,
    x_rot: f32,
    missing_respawn_block: bool,
}

impl Server {
    pub fn create_common_spawn_info(&self, p: &mut Writer, level_index: usize, player: &ServerPlayer) {
        let level = &self.levels[level_index];
        p.varint(level.dimension_type_id);
        p.string(level.dimension);
        p.i64(hashed_seed(self.seed));
        p.varint(player.game_mode.game_type.id());
        p.varint(player.game_mode.previous_game_type.map_or(0, |g| g.id() + 1));
        p.bool(false);
        p.bool(level.chunks.generator.is_flat());
        write_death_location(p, &player.last_death_location);
        p.varint(player.entity.portal_cooldown);
        p.varint(level.chunks.generator.sea_level());
    }

    pub fn send_level_info(&self, out: &Outbound, level_index: usize) {
        let level = &self.levels[level_index];
        out.send(clientbound::INITIALIZE_BORDER, |p| write_initialize_border(p, &level.world_border));
        out.send(clientbound::SET_TIME, |p| self.full_time_sync(p));
        out.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| self.write_spawn(p));
        level.write_weather_for_joining(out);
        out.send(clientbound::GAME_EVENT, |p| {
            p.u8(GAME_EVENT_LEVEL_CHUNKS_LOAD_START);
            p.f32(0.0);
        });
        self.update_joining_player_tick_rate(out);
    }

    pub fn send_player_permission_level_to(&self, level_index: usize, player: &ServerPlayer) {
        let permission = self.profile_permission_level(player.profile.uuid);
        player.connection.send(clientbound::ENTITY_EVENT, |p| {
            p.i32(player.entity.id);
            p.u8(crate::server::EVENT_PERMISSION_LEVEL_ALL + permission as u8);
        });
        player.connection.send(clientbound::COMMANDS, |p| {
            crate::commands::write_commands_packet(p, &crate::commands::source::CommandSourceStack::for_new_player(self, level_index, player))
        });
    }

    pub(in crate::server) fn handle_client_command(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        self.player_mut(id).reset_last_action_time();
        let action = packet.reader().varint()?;
        if action == REQUEST_STATS {
            let player = self.player_mut(id);
            player.stats.send_stats(&player.connection);
            return Ok(());
        }
        if action != PERFORM_RESPAWN {
            return Ok(());
        }
        if self.player(id).won_game {
            self.player_mut(id).won_game = false;
            self.respawn(id, true, RemovalReason::ChangedDimension);
            let p = self.player_mut(id);
            p.reset_position();
            p.movement.waiting_for_respawn = false;
            p.movement.client_loaded_timeout_timer = 60;
            crate::advancements::triggers::criteria_triggers::CHANGED_DIMENSION.trigger(self.entity_level_mut(id), id, crate::server::THE_END, crate::server::OVERWORLD);
            return Ok(());
        }
        if self.player(id).health > 0.0 {
            return Ok(());
        }
        self.respawn(id, false, RemovalReason::Killed);
        let p = self.player_mut(id);
        p.reset_position();
        p.movement.waiting_for_respawn = false;
        p.movement.client_loaded_timeout_timer = 60;
        if self.hardcore {
            self.entity_level_mut(id).set_game_mode(id, GameType::Spectator);
        }
        Ok(())
    }

    pub fn find_respawn_teleport_transition(&mut self, id: EntityId, consume_spawn_block: bool, post_teleport_transition: PostTeleportTransition) -> TeleportTransition {
        let respawn = self.find_respawn_position_and_use_spawn_block(id, consume_spawn_block);
        let mut transition = TeleportTransition::new(respawn.level, respawn.position, Vec3::ZERO, respawn.y_rot, respawn.x_rot, post_teleport_transition);
        transition.missing_respawn_block = respawn.missing_respawn_block;
        transition
    }

    fn find_respawn_position_and_use_spawn_block(&mut self, id: EntityId, consume_spawn_block: bool) -> RespawnTransition {
        let config = self.player(id).respawn_config.clone();
        let respawn_level = self.get_level(config.as_ref().map_or(OVERWORLD, |c| c.respawn_data.dimension.as_str()));
        if let (Some(respawn_level), Some(config)) = (respawn_level, config) {
            return match self.find_respawn_and_use_spawn_block(respawn_level, &config, consume_spawn_block) {
                Some((position, y_rot, x_rot)) => RespawnTransition { level: respawn_level, position, y_rot, x_rot, missing_respawn_block: false },
                None => self.default_respawn(true),
            };
        }
        self.default_respawn(false)
    }

    fn find_respawn_and_use_spawn_block(&mut self, level: usize, respawn_config: &RespawnConfig, consume_spawn_block: bool) -> Option<(Vec3, f32, f32)> {
        let level = &mut self.levels[level];
        let respawn_data = &respawn_config.respawn_data;
        let (pos, yaw, pitch, forced) = (respawn_data.pos, respawn_data.yaw, respawn_data.pitch, respawn_config.forced);
        let block_state = level.block_state(pos);
        let block = block_state.block();
        let player_type = DismountType { dimensions: get_default_dimensions(Pose::Standing), fire_immune: false, is_player: true };
        if block.is_instance_of("RespawnAnchorBlock") && (forced || block_state.get(RESPAWN_ANCHOR_CHARGES) > 0) && respawn_anchor_block::can_set_spawn(level, pos) {
            let stand_up_position = respawn_anchor_block::find_stand_up_position(player_type, level, pos);
            if !forced && consume_spawn_block && stand_up_position.is_some() {
                level.set_block(pos, block_state.set(RESPAWN_ANCHOR_CHARGES, block_state.get(RESPAWN_ANCHOR_CHARGES) - 1), UPDATE_ALL);
            }
            return stand_up_position.map(|p| (p, calculate_look_at_yaw(p, pos), 0.0));
        }
        if let Some(kind) = abstract_bed_block::kind(block)
            && abstract_bed_block::get_bed_rule(kind, level, pos).can_set_spawn(level)
        {
            return abstract_bed_block::find_stand_up_position(player_type, level, pos, block_state.get(HORIZONTAL_FACING), yaw)
                .map(|p| (p, calculate_look_at_yaw(p, pos), 0.0));
        }
        if !forced {
            return None;
        }
        let free_bottom = is_possible_to_respawn_in_this(block_state);
        let free_top = is_possible_to_respawn_in_this(level.block_state(pos.above()));
        (free_bottom && free_top).then(|| (Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.1, pos.z as f64 + 0.5), yaw, pitch))
    }

    fn default_respawn(&mut self, missing_respawn_block: bool) -> RespawnTransition {
        let level = self.find_respawn_dimension();
        let spawn = self.respawn_data().clone();
        let adjusted = self.levels[level].find_spawn(spawn.pos, self.default_game_type == GameType::Adventure);
        let position = Vec3::at_bottom_center_of(BlockPos::containing(adjusted.x, adjusted.y, adjusted.z));
        RespawnTransition { level, position, y_rot: spawn.yaw, x_rot: spawn.pitch, missing_respawn_block }
    }

    pub fn respawn(&mut self, id: EntityId, keep_all_player_data: bool, removal_reason: RemovalReason) {
        let transition = self.find_respawn_position_and_use_spawn_block(id, !keep_all_player_data);
        let old_level = self.level_of(id).expect("respawning player");
        let mut old = self.levels[old_level].remove_player_immediately(id, removal_reason);
        self.data_storage.borrow().player_removed(id);
        let level_index = transition.level;
        let old_score = old.score();
        let mut player = ServerPlayer::new(old.profile.clone(), old.client, old.connection.clone(), BlockPos::ZERO);
        player.stats = std::mem::take(&mut old.stats);
        player.advancements = old.advancements;
        player.entity.id = id;
        player.chunk_sender = old.chunk_sender;
        player.movement = old.movement;
        player.waypoints = old.waypoints;
        player.ack_block_changes_up_to = old.ack_block_changes_up_to;
        player.keep_alive_time = old.keep_alive_time;
        player.keep_alive_challenge = old.keep_alive_challenge;
        player.latency = old.latency;
        player.chat = old.chat;
        player.chat_session = old.chat_session.clone();
        player.disconnected = old.disconnected;
        player.set_game_mode_for_player(old.game_mode.game_type, old.game_mode.previous_game_type);
        player.connection.send(clientbound::PLAYER_ABILITIES, |p| player.abilities.write(p));
        player.living.attributes.assign_base_values(&old.living.attributes);
        if keep_all_player_data {
            player.living.attributes.assign_permanent_modifiers(&old.living.attributes);
            player.health = mth::clamp_f(old.health, 0.0, player.living.attributes.value(Attribute::MaxHealth) as f32);
            player.entity.data.set(ids::HEALTH, DataValue::Float(player.health));
            player.food_data = old.food_data;
            self.levels[level_index].players.insert(id, player);
            for effect in &old.living.active_effects {
                self.levels[level_index].add_effect(id, effect.clone(), None);
            }
            player = self.levels[level_index].players.shift_remove(&id).expect("restored player");
            transfer_inventory_xp_and_score(&mut player, &old.inventory, &old.experience, old_score);
            player.entity.portal_process = old.entity.portal_process;
        } else {
            player.health = player.living.attributes.value(Attribute::MaxHealth) as f32;
            player.entity.data.set(ids::HEALTH, DataValue::Float(player.health));
            if self.game_rules.get_bool(crisp_data::game_rule::KEEP_INVENTORY) || old.game_mode.game_type == GameType::Spectator {
                transfer_inventory_xp_and_score(&mut player, &old.inventory, &old.experience, old_score);
            }
        }
        player.experience.enchantment_seed = old.experience.enchantment_seed;
        player.ender_chest_inventory = old.ender_chest_inventory;
        player.seen_credits = old.seen_credits;
        player.entered_nether_position = old.entered_nether_position;
        player.recipe_book.copy_over_data(&old.recipe_book);
        player.last_sent_exp = -1;
        player.last_sent_health = -1.0;
        player.last_sent_food = -1;
        player.chunk_tracking_view = old.chunk_tracking_view;
        player.last_death_location = old.last_death_location.clone();
        if !transition.missing_respawn_block {
            player.respawn_config = old.respawn_config.clone();
        }
        for post_effect in &old.post_effects {
            player.add_post_effect(post_effect);
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
        out.send(clientbound::RESPAWN, |p| {
            self.create_common_spawn_info(p, level_index, &player);
            p.u8(keep_all_player_data as u8);
        });
        let (pos, y_rot, x_rot) = (player.entity.position(), player.entity.y_rot, player.entity.x_rot);
        player.teleport(pos, y_rot, x_rot);
        out.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| self.write_spawn(p));
        self.send_difficulty_update(&out);
        player.send_experience();
        for effect in &player.living.active_effects {
            out.send_raw(update_mob_effect(id, effect, false));
        }
        player.send_post_effects();
        self.send_level_info(&out, level_index);
        self.send_player_permission_level_to(level_index, &player);
        self.levels[level_index].add_player(player);
        self.player_list.retain(|&p| p != id);
        self.player_list.push(id);
        self.levels[level_index].init_inventory_menu(id);
        if !keep_all_player_data
            && let Some(respawn_config) = self.player(id).respawn_config.clone()
            && let Some(respawn_level) = self.get_level(&respawn_config.respawn_data.dimension)
        {
            let respawn_position = respawn_config.respawn_data.pos;
            if self.levels[respawn_level].block_state(respawn_position).is_block(crisp_data::blocks::RESPAWN_ANCHOR) {
                let seed = self.levels[level_index].random.next_long();
                let sound = crate::world::entity::sound_event("minecraft:block.respawn_anchor.deplete");
                self.player(id).connection.send(clientbound::SOUND, |p| {
                    p.varint(sound + 1);
                    p.varint(crate::server::level::SoundSource::Blocks as i32);
                    p.i32(respawn_position.x * 8);
                    p.i32(respawn_position.y * 8);
                    p.i32(respawn_position.z * 8);
                    p.f32(1.0);
                    p.f32(1.0);
                    p.i64(seed);
                });
            }
        }
    }
}

fn transfer_inventory_xp_and_score(
    player: &mut ServerPlayer,
    old_inventory: &crate::world::item::Inventory,
    old_experience: &crate::world::entity::player::experience::Experience,
    old_score: i32,
) {
    for slot in 5..crate::world::item::SLOTS {
        player.inventory.slots[slot] = old_inventory.slots[slot].clone();
    }
    player.inventory.selected = old_inventory.selected;
    player.experience.level = old_experience.level;
    player.experience.total = old_experience.total;
    player.experience.progress = old_experience.progress;
    player.set_score(old_score);
}

fn calculate_look_at_yaw(position: Vec3, look_at_block_pos: BlockPos) -> f32 {
    let look_direction = Vec3::at_bottom_center_of(look_at_block_pos).subtract(position).normalize();
    crisp_core::mth::wrap_degrees_d(crisp_core::mth::atan2(look_direction.z, look_direction.x) * (180.0f32 as f64) / std::f32::consts::PI as f64 - 90.0) as f32
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
        let id = testing::server_player(&mut server, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let (out, mut packets) = Outbound::capture();
        let p = server.levels[0].player_mut(id);
        p.connection = out;
        p.inventory.slots[36] = Some(ItemStack::new(items::STONE, 3));
        server.levels[0].handle_player_loaded(id);
        server.handle_client_command(id, &perform_respawn()).unwrap();
        assert_eq!(ids(&mut packets), Vec::<i32>::new());
        server.levels[0].kill(id);
        let died = ids(&mut packets);
        assert_eq!(died.first(), Some(&clientbound::DAMAGE_EVENT));
        assert_eq!(&died[1..], [clientbound::PLAYER_COMBAT_KILL, clientbound::SYSTEM_CHAT, clientbound::ENTITY_EVENT]);
        assert!(server.levels[0].player(id).movement.waiting_for_respawn);
        for _ in 0..20 {
            server.levels[0].tick();
            server.levels[0].tick_connection(id);
        }
        assert!(server.levels[0].entity(id).is_removed());
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
        let p = server.levels[0].player(id);
        assert_eq!((p.entity.id, p.health, p.entity.is_removed()), (id, 20.0, false));
        assert_eq!(p.inventory.slots[36], Some(ItemStack::new(items::STONE, 3)));
        assert_eq!(p.last_death_location.as_ref().map(|l| l.1), Some(crisp_core::BlockPos::new(0, -60, 0)));
        assert!(!p.movement.waiting_for_respawn);
        server.levels[0].tick();
        server.levels[0].tick_connection(id);
        let next = ids(&mut packets);
        let health = next.iter().position(|&i| i == clientbound::SET_HEALTH).unwrap();
        assert_eq!(next[health + 1], clientbound::SET_EXPERIENCE);
    }
}
