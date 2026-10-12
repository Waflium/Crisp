use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Instant;

use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_data::{WORLD_VERSION, items, registry, registry_id};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::chunk_map::ChunkTrackingView;
use super::game_mode::ServerPlayerGameMode;
use crate::network::chat::remote_chat_session::RemoteChatSession;
use crate::network::configuration::ClientInfo;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::util::tick_throttler::TickThrottler;
use super::game_mode::GameType;
use crate::server::network::chat::ChatState;
use crate::server::network::chunk_sender::PlayerChunkSender;
use crate::server::network::movement::{Input, PlayerMovement};
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation};
use crate::world::entity::data::{DataValue, ids, player_defaults};
use super::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::living_entity::{LivingEntity, remove_mob_effect, update_mob_effect};
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_event, flags};
use crate::world::scores::criteria::ObjectiveCriteria;
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::scores::team::Visibility;
use crisp_data::{MobEffect, game_rule};
use crate::world::inventory::{Drag, Menu, SLOT_COUNT, Window};
use crate::world::item::{Inventory, ItemStack, OFFHAND, protocol_slot, storage_slot, write_slot};
use crate::world::entity::player::abilities::Abilities;
use crate::world::level::storage::RespawnData;
use crate::world::InteractionHand;

pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.6, height: 1.8, eye_height: 1.62 };

const CREATIVE_BLOCK_INTERACTION_RANGE_MODIFIER: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:creative_mode_block_range"), amount: 0.5, operation: Operation::AddValue };
const CREATIVE_ENTITY_INTERACTION_RANGE_MODIFIER: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:creative_mode_entity_range"), amount: 2.0, operation: Operation::AddValue };
const WAYPOINT_TRANSMIT_RANGE_CROUCH_MODIFIER: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:waypoint_transmit_range_crouch"), amount: -1.0, operation: Operation::AddMultipliedTotal };

#[derive(Clone, PartialEq, Debug)]
pub struct RespawnConfig {
    pub respawn_data: RespawnData,
    pub forced: bool,
}

pub struct LastRecorded {
    pub health_and_absorption: f32,
    pub food_level: i32,
    pub air_level: i32,
    pub armor: i32,
    pub level: i32,
    pub experience: i32,
}

impl Default for LastRecorded {
    fn default() -> LastRecorded {
        LastRecorded { health_and_absorption: f32::from_bits(1), food_level: i32::MIN, air_level: i32::MIN, armor: i32::MIN, level: i32::MIN, experience: i32::MIN }
    }
}

pub struct ServerPlayer {
    pub entity: Entity,
    pub profile: Profile,
    pub connection: Outbound,
    pub client: ClientInfo,
    pub requested_view_distance: i32,
    pub chunk_tracking_view: Option<ChunkTrackingView>,
    pub chunk_sender: PlayerChunkSender,
    pub last_section_pos: (i32, i32, i32),
    pub game_mode: ServerPlayerGameMode,
    pub abilities: Abilities,
    pub respawn_config: Option<RespawnConfig>,
    pub sleep_counter: i32,
    pub health: f32,
    pub food_data: crate::world::food::FoodData,
    pub experience: crate::world::entity::player::experience::Experience,
    pub last_sent_health: f32,
    pub last_sent_food: i32,
    pub last_food_saturation_zero: bool,
    pub last_sent_exp: i32,
    pub attack_strength_ticker: i32,
    pub item_swap_ticker: i32,
    pub last_item_in_main_hand: ItemStack,
    pub last_recorded: LastRecorded,
    pub last_death_location: Option<(String, BlockPos)>,
    pub inventory: Inventory,
    pub inventory_state: i32,
    pub dirty_slots: Vec<usize>,
    pub window: Window,
    pub container_menu: Option<crate::world::inventory::abstract_container_menu::AbstractContainerMenu>,
    pub carried: Option<ItemStack>,
    pub drag: Drag,
    pub last_window_id: i32,
    pub living: LivingEntity,
    pub movement: PlayerMovement,
    pub last_client_input: Input,
    pub known_movement: Vec3,
    pub last_action_time: Instant,
    pub starting_to_fall_position: Option<Vec3>,
    pub waypoints: HashMap<EntityId, BlockPos>,
    pub ack_block_changes_up_to: i32,
    pub drop_spam_throttler: TickThrottler,
    pub keep_alive_time: Instant,
    pub keep_alive_challenge: Option<i64>,
    pub address: Option<std::net::SocketAddr>,
    pub latency: i32,
    pub received_packets: i32,
    pub average_received_packets: f32,
    pub connection_tick_count: i32,
    pub chat: ChatState,
    pub chat_session: Option<RemoteChatSession>,
    pub disconnected: bool,
    pub disconnect_reason: Option<String>,
    pub command_suggestions: crate::server::network::server_command_suggestions_provider::ServerCommandSuggestionsProvider,
}

impl ServerPlayer {
    pub fn update_options(&mut self, info: ClientInfo) {
        self.requested_view_distance = info.view_distance as i32;
        self.entity.data.set(ids::PLAYER_MODE_CUSTOMISATION, DataValue::Byte(info.skin_parts as i8));
        self.entity.data.set(ids::PLAYER_MAIN_HAND, DataValue::HumanoidArm(info.main_hand));
        self.client = info;
    }

    pub fn new(profile: Profile, client: ClientInfo, connection: Outbound, spawn: BlockPos) -> ServerPlayer {
        let profile_id = profile.uuid;
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:player"), DIMENSIONS, player_defaults());
        entity.uuid = profile.uuid;
        entity.set_pos(Vec3::new(spawn.x as f64 + 0.5, spawn.y as f64, spawn.z as f64 + 0.5));
        entity.data.set(ids::HEALTH, DataValue::Float(20.0));
        let last_section_pos = (mth::floor(entity.x()) >> 4, mth::floor(entity.y()) >> 4, mth::floor(entity.z()) >> 4);
        let mut player = ServerPlayer {
            entity,
            profile,
            connection,
            requested_view_distance: 0,
            client: ClientInfo::default(),
            chunk_tracking_view: None,
            chunk_sender: PlayerChunkSender::default(),
            last_section_pos,
            game_mode: ServerPlayerGameMode::default(),
            abilities: Abilities::default(),
            respawn_config: None,
            sleep_counter: 0,
            health: 20.0,
            food_data: Default::default(),
            experience: Default::default(),
            last_sent_health: -1.0e8,
            last_sent_food: -99999999,
            last_food_saturation_zero: true,
            last_sent_exp: -99999999,
            attack_strength_ticker: 0,
            item_swap_ticker: 0,
            last_item_in_main_hand: ItemStack::empty(),
            last_recorded: LastRecorded::default(),
            last_death_location: None,
            inventory: Inventory::new(),
            inventory_state: 1,
            dirty_slots: Vec::new(),
            window: Window::inventory(),
            container_menu: None,
            carried: None,
            drag: Drag::default(),
            drop_spam_throttler: TickThrottler::new(20, 1480),
            last_window_id: 0,
            living: LivingEntity::player(),
            movement: PlayerMovement::default(),
            last_client_input: Input::default(),
            known_movement: Vec3::ZERO,
            last_action_time: Instant::now(),
            starting_to_fall_position: None,
            waypoints: HashMap::new(),
            ack_block_changes_up_to: -1,
            keep_alive_time: Instant::now(),
            keep_alive_challenge: None,
            address: None,
            latency: 0,
            received_packets: 0,
            average_received_packets: 0.0,
            connection_tick_count: 0,
            chat: ChatState::new(profile_id, &crate::ServerConfig::default()),
            chat_session: None,
            disconnected: false,
            disconnect_reason: None,
            command_suggestions: Default::default(),
        };
        player.update_options(client);
        player
    }

    pub fn ip_address(&self) -> String {
        match self.address {
            Some(address) => address.ip().to_canonical().to_string(),
            None => "<unknown>".to_string(),
        }
    }

    pub fn id(&self) -> EntityId {
        self.entity.id
    }

    pub fn is_secondary_use_active(&self) -> bool {
        self.entity.is_shift_key_down()
    }

    pub fn abilities_flying(&self) -> bool {
        self.abilities.flying
    }

    pub fn abilities_may_fly(&self) -> bool {
        self.abilities.mayfly
    }

    pub fn set_game_mode_for_player(&mut self, game_type: GameType, previous: Option<GameType>) {
        self.game_mode.previous_game_type = previous;
        self.game_mode.game_type = game_type;
        game_type.update_player_abilities(&mut self.abilities);
    }

    pub fn saved_game_types(nbt: &Nbt) -> (Option<GameType>, Option<GameType>) {
        let read = |key: &str| nbt.get(key).and_then(Nbt::as_i64).map(|id| GameType::by_id(id as i32));
        (read("playerGameType"), read("previousPlayerGameType"))
    }

    pub fn item_in_hand(&self, hand: InteractionHand) -> Option<ItemStack> {
        match hand {
            InteractionHand::MainHand => self.inventory.held().cloned(),
            InteractionHand::OffHand => self.inventory.slots[OFFHAND].clone(),
        }
    }

    pub fn hand_slot(&self, hand: InteractionHand) -> usize {
        match hand {
            InteractionHand::MainHand => self.inventory.selected_slot(),
            InteractionHand::OffHand => OFFHAND,
        }
    }

    pub fn set_item_in_hand(&mut self, hand: InteractionHand, stack: Option<ItemStack>) {
        let slot = self.hand_slot(hand);
        self.inventory.slots[slot] = stack.filter(|s| s.count > 0 && s.item != items::AIR);
        self.dirty_slots.push(slot);
    }

    pub fn update_item_in_hand(&mut self, hand: InteractionHand, item_stack: &ItemStack) {
        let changed = match &self.inventory.slots[self.hand_slot(hand)] {
            Some(current) => !ItemStack::matches(current, item_stack),
            None => !item_stack.is_empty(),
        };
        if changed {
            self.set_item_in_hand(hand, Some(item_stack.clone()));
        }
    }

    pub fn to_nbt(&self, dimension: &str) -> Nbt {
        let s = |v: &str| Nbt::String(v.into());
        let mut nbt = Vec::new();
        self.entity.save_without_id(&mut nbt);
        nbt.push(("Health".into(), Nbt::Float(self.health)));
        nbt.push(("HurtTime".into(), Nbt::Short(self.living.hurt_time as i16)));
        nbt.push(("DeathTime".into(), Nbt::Short(self.living.death_time as i16)));
        let absorption = match self.entity.data.get(ids::PLAYER_ABSORPTION) {
            DataValue::Float(v) => *v,
            _ => 0.0,
        };
        nbt.push(("AbsorptionAmount".into(), Nbt::Float(absorption)));
        nbt.push(("attributes".into(), self.living.attributes.to_nbt()));
        if !self.living.active_effects.is_empty() {
            nbt.push(("active_effects".into(), Nbt::List(self.living.active_effects.iter().map(MobEffectInstance::to_nbt).collect())));
        }
        if let DataValue::OptionalBlockPos(Some(pos)) = self.entity.data.get(ids::SLEEPING_POS) {
            nbt.push(("sleeping_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
        nbt.push(("DataVersion".into(), Nbt::Int(WORLD_VERSION)));
        let inventory = self
            .inventory
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| {
                let stack = stack.as_ref().filter(|s| !s.is_empty())?;
                let Nbt::Compound(mut fields) = stack.to_nbt() else { return None };
                fields.insert(0, ("Slot".into(), Nbt::Byte(storage_slot(slot)?)));
                Some(Nbt::Compound(fields))
            })
            .collect();
        nbt.push(("Inventory".into(), Nbt::List(inventory)));
        nbt.push(("SelectedItemSlot".into(), Nbt::Int(self.inventory.selected as i32)));
        self.experience.save(&mut nbt);
        nbt.push(("SleepTimer".into(), Nbt::Short(self.sleep_counter as i16)));
        nbt.push(("Score".into(), Nbt::Int(self.score())));
        self.food_data.save(&mut nbt);
        nbt.push(("abilities".into(), self.abilities.to_nbt()));
        if let Some((dimension, pos)) = &self.last_death_location {
            nbt.push((
                "LastDeathLocation".into(),
                Nbt::Compound(vec![("dimension".into(), s(dimension)), ("pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z]))]),
            ));
        }
        nbt.push(("playerGameType".into(), Nbt::Int(self.game_mode.game_type.id())));
        if let Some(previous) = self.game_mode.previous_game_type {
            nbt.push(("previousPlayerGameType".into(), Nbt::Int(previous.id())));
        }
        nbt.push(("Dimension".into(), s(dimension)));
        if let Some(respawn) = &self.respawn_config {
            let mut fields = respawn.respawn_data.to_nbt_fields();
            if respawn.forced {
                fields.push(("forced".into(), Nbt::Byte(1)));
            }
            nbt.push(("respawn".into(), Nbt::Compound(fields)));
        }
        Nbt::Compound(nbt)
    }

    pub fn saved_sleeping_pos(nbt: &Nbt) -> Option<BlockPos> {
        match nbt.get("sleeping_pos")? {
            Nbt::IntArray(p) if p.len() == 3 => Some(BlockPos::new(p[0], p[1], p[2])),
            _ => None,
        }
    }

    pub fn load_nbt(&mut self, nbt: &Nbt) {
        let _ = self.entity.load(nbt);
        if let Some(abilities) = nbt.get("abilities") {
            self.abilities = Abilities::load(abilities);
        }
        self.respawn_config = nbt.get("respawn").and_then(|r| {
            let forced = r.get("forced").and_then(Nbt::as_i64).is_some_and(|f| f != 0);
            Some(RespawnConfig { respawn_data: RespawnData::from_nbt(r)?, forced })
        });
        let number = |key: &str| nbt.get(key).and_then(Nbt::as_f64);
        if let Some(attributes) = nbt.get("attributes") {
            self.living.attributes.load(attributes);
        }
        self.living.active_effects.clear();
        for effect in nbt.get("active_effects").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(MobEffectInstance::from_nbt) {
            match self.living.active_effects.binary_search_by_key(&effect.effect, |e| e.effect) {
                Ok(index) => self.living.active_effects[index] = effect,
                Err(index) => self.living.active_effects.insert(index, effect),
            }
            self.living.effects_dirty = true;
        }
        let max_health = self.living.attributes.value(Attribute::MaxHealth) as f32;
        self.health = mth::clamp_f(number("Health").map_or(max_health, |v| v as f32), 0.0, max_health);
        self.entity.data.set(ids::HEALTH, DataValue::Float(self.health));
        self.food_data = crate::world::food::FoodData::load(nbt);
        let walking_speed = self.abilities.walking_speed as f64;
        self.living.attributes.instance(Attribute::MovementSpeed).expect("players have movement speed").set_base_value(walking_speed);
        self.living.hurt_time = number("HurtTime").map_or(0, |v| v as i32);
        self.living.death_time = number("DeathTime").map_or(0, |v| v as i32);
        self.entity.data.set(ids::PLAYER_ABSORPTION, DataValue::Float(number("AbsorptionAmount").map_or(0.0, |v| v as f32).max(0.0)));
        self.last_death_location = nbt.get("LastDeathLocation").and_then(|l| {
            let dimension = l.get("dimension").and_then(Nbt::as_str)?.to_owned();
            match l.get("pos")? {
                Nbt::IntArray(p) if p.len() == 3 => Some((dimension, BlockPos::new(p[0], p[1], p[2]))),
                _ => None,
            }
        });
        self.inventory.selected = number("SelectedItemSlot").map_or(0, |v| v as usize).min(8);
        for entry in nbt.get("Inventory").map(Nbt::as_list).unwrap_or(&[]) {
            let slot = entry.get("Slot").and_then(Nbt::as_i64).and_then(|s| protocol_slot(s as i8));
            if let (Some(slot), Ok(stack)) = (slot, ItemStack::from_nbt(entry)) {
                self.inventory.slots[slot] = Some(stack);
            }
        }
        self.experience.load(nbt, &mut self.entity.random);
        self.set_score(nbt.get("Score").and_then(crate::world::level::block::entity::int_value).unwrap_or(0));
        self.sleep_counter = number("SleepTimer").map_or(0, |v| v as i16 as i32);
        let p = self.entity.position();
        self.last_section_pos = (mth::floor(p.x) >> 4, mth::floor(p.y) >> 4, mth::floor(p.z) >> 4);
    }

    pub fn write_chat_session(&self, p: &mut Writer) {
        let session = self.get_chat_session();
        p.bool(session.is_some());
        if let Some(session) = session {
            session.write_data(p);
        }
    }

    pub fn write_info(&self, p: &mut Writer) {
        p.uuid(self.profile.uuid);
        p.string(&self.profile.name);
        self.profile.write_properties(p);
        self.write_chat_session(p);
        p.varint(self.game_mode.game_type.id());
        p.bool(true);
        p.varint(self.latency);
        p.bool(false);
        p.varint(0);
        p.bool(true);
    }

    pub fn update_player_attributes(&mut self) {
        let creative = self.game_mode.game_type == GameType::Creative;
        let crouching = self.entity.is_crouching();
        let attributes = &mut self.living.attributes;
        for (attribute, modifier, applies) in [
            (Attribute::BlockInteractionRange, CREATIVE_BLOCK_INTERACTION_RANGE_MODIFIER, creative),
            (Attribute::EntityInteractionRange, CREATIVE_ENTITY_INTERACTION_RANGE_MODIFIER, creative),
            (Attribute::WaypointTransmitRange, WAYPOINT_TRANSMIT_RANGE_CROUCH_MODIFIER, crouching),
        ] {
            if let Some(instance) = attributes.instance(attribute) {
                if applies {
                    instance.add_or_update_transient_modifier(modifier);
                } else {
                    instance.remove_modifier(&modifier.id);
                }
            }
        }
    }

    pub fn pick_up(&mut self, stack: ItemStack) -> i32 {
        let (changed, left) = self.inventory.add(stack);
        self.dirty_slots.extend(changed);
        left
    }

    pub fn sync_inventory(&mut self) {
        if self.container_menu.is_some() {
            self.dirty_slots.retain(|&slot| !(9..=44).contains(&slot));
            return;
        }
        for slot in std::mem::take(&mut self.dirty_slots) {
            if let Some(window_slot) = self.window.window_slot(slot) {
                self.send_slot(window_slot);
            }
        }
    }

    pub fn send_slot(&mut self, slot: usize) {
        self.inventory_state += 1;
        let stack = self.window.slot(&self.inventory, slot);
        self.connection.send(clientbound::CONTAINER_SET_SLOT, |p| {
            p.varint(self.window.id);
            p.varint(self.inventory_state);
            p.i16(slot as i16);
            write_slot(p, stack.as_ref());
        });
    }

    pub fn send_window_content(&self) {
        self.connection.send(clientbound::CONTAINER_SET_CONTENT, |p| {
            p.varint(self.window.id);
            p.varint(self.inventory_state);
            p.varint(SLOT_COUNT as i32);
            (0..SLOT_COUNT).for_each(|s| write_slot(p, self.window.slot(&self.inventory, s).as_ref()));
            write_slot(p, self.carried.as_ref());
        });
    }

    pub fn menu(&mut self) -> Menu<'_> {
        Menu { window: &mut self.window, inventory: &mut self.inventory, carried: &mut self.carried }
    }

    pub fn reset_last_action_time(&mut self) {
        self.last_action_time = Instant::now();
    }

    pub fn disconnect(&mut self, translation_key: &str) {
        self.disconnect_with(crate::network::chat::translate(translation_key));
    }

    pub fn disconnect_with(&mut self, reason: crate::network::chat::Component) {
        self.connection.send(clientbound::DISCONNECT, |p| reason.write(p));
        self.disconnected = true;
        self.disconnect_reason = Some(reason.get_string());
    }
}

impl ServerLevel {
    pub fn handle_extra_items_created_on_use(&mut self, id: EntityId, extra_items: ItemStack) {
        let left = self.player_mut(id).pick_up(extra_items.clone());
        if left > 0 {
            self.player_drop(id, extra_items.copy_with_count(left), false, crate::util::Prediction::Predicted);
        }
    }

    pub fn server_player_do_tick(&mut self, id: EntityId) {
        if !self.is_spectator(id) || !self.touching_unloaded_chunk(id) {
            self.player_tick(id);
            self.tick_food(id);
        }
        let p = self.player_mut(id);
        let food = p.food_data;
        let saturation_zero = food.saturation_level == 0.0;
        if p.health != p.last_sent_health || p.last_sent_food != food.food_level || saturation_zero != p.last_food_saturation_zero {
            let health = p.health;
            p.connection.send(clientbound::SET_HEALTH, |w| {
                w.f32(health);
                w.varint(food.food_level);
                w.f32(food.saturation_level);
            });
            p.last_sent_health = health;
            p.last_sent_food = food.food_level;
            p.last_food_saturation_zero = saturation_zero;
        }
        let health_and_absorption = self.player(id).health + self.absorption_amount(id);
        if health_and_absorption != self.player(id).last_recorded.health_and_absorption {
            self.player_mut(id).last_recorded.health_and_absorption = health_and_absorption;
            self.update_score_for_criteria(id, &ObjectiveCriteria::HEALTH, mth::ceil(health_and_absorption as f64));
        }
        let food = self.player(id).food_data.food_level;
        if food != self.player(id).last_recorded.food_level {
            self.player_mut(id).last_recorded.food_level = food;
            self.update_score_for_criteria(id, &ObjectiveCriteria::FOOD, food);
        }
        let air = self.player(id).entity.air_supply();
        if air != self.player(id).last_recorded.air_level {
            self.player_mut(id).last_recorded.air_level = air;
            self.update_score_for_criteria(id, &ObjectiveCriteria::AIR, air);
        }
        let armor = self.armor_value(id);
        if armor != self.player(id).last_recorded.armor {
            self.player_mut(id).last_recorded.armor = armor;
            self.update_score_for_criteria(id, &ObjectiveCriteria::ARMOR, armor);
        }
        let experience = self.player(id).experience;
        if experience.total != self.player(id).last_recorded.experience {
            self.player_mut(id).last_recorded.experience = experience.total;
            self.update_score_for_criteria(id, &ObjectiveCriteria::EXPERIENCE, experience.total);
        }
        if experience.level != self.player(id).last_recorded.level {
            self.player_mut(id).last_recorded.level = experience.level;
            self.update_score_for_criteria(id, &ObjectiveCriteria::LEVEL, experience.level);
        }
        let p = self.player_mut(id);
        if p.experience.total != p.last_sent_exp {
            p.last_sent_exp = p.experience.total;
            p.send_experience();
        }
    }

    pub fn server_player_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        if let Some(attacker) = source.causing_entity.filter(|&e| self.is_player(e))
            && !self.can_harm_player(id, attacker)
        {
            return false;
        }
        self.player_hurt_server(id, source, damage)
    }

    pub fn server_player_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        self.player_is_invulnerable_to(id, source) || !self.player(id).movement.has_client_loaded()
    }

    pub fn can_harm_player(&self, id: EntityId, target: EntityId) -> bool {
        self.game_rules.get_bool(game_rule::PVP) && self.do_teams_allow_damage(id, target)
    }

    pub fn server_player_die(&mut self, id: EntityId, source: DamageSource) {
        if self.game_rules.get_bool(game_rule::SHOW_DEATH_MESSAGES) {
            let message = self.death_message(id);
            self.player(id).connection.send(clientbound::PLAYER_COMBAT_KILL, |p| {
                p.varint(id);
                message.write(p);
            });
            match self.team_of(id).map(|t| t.death_message_visibility()) {
                None | Some(Visibility::Always) => self.broadcast_system_message(&message),
                Some(Visibility::HideForOtherTeams) => self.broadcast_system_to_team(id, &message),
                Some(Visibility::HideForOwnTeam) => self.broadcast_system_to_all_except_team(id, &message),
                Some(Visibility::Never) => {}
            }
        } else {
            self.player(id).connection.send(clientbound::PLAYER_COMBAT_KILL, |p| {
                p.varint(id);
                crate::network::chat::Component::empty().write(p);
            });
        }
        if !self.is_spectator(id) {
            self.drop_all_death_loot(id, &source);
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&ObjectiveCriteria::DEATH_COUNT, &holder, |level, score| {
            let _ = score.increment(level);
        });
        if let Some(killer) = self.kill_credit(id) {
            self.award_kill_score(killer, id);
        }
        self.broadcast_entity_event(id, entity_event::DEATH);
        self.clear_fire(id);
        self.entity_mut(id).set_ticks_frozen(0);
        self.entity_mut(id).set_shared_flag(flags::ON_FIRE, false);
        self.recheck_combat_status(id);
        let pos = self.entity(id).block_position();
        let dimension = self.dimension;
        let p = self.player_mut(id);
        p.last_death_location = Some((dimension.to_owned(), pos));
        p.movement.waiting_for_respawn = true;
    }

    pub fn server_player_award_kill_score(&mut self, id: EntityId, victim: EntityId) {
        if victim == id {
            return;
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&ObjectiveCriteria::KILL_COUNT_ALL, &holder, |level, score| {
            let _ = score.increment(level);
        });
        if self.is_player(victim) {
            self.for_all_objectives(&ObjectiveCriteria::KILL_COUNT_PLAYERS, &holder, |level, score| {
                let _ = score.increment(level);
            });
        }
    }

    pub fn get_enchanted_damage(&mut self, id: EntityId, entity: EntityId, damage: f32, damage_source: &DamageSource) -> f32 {
        let weapon = self.get_weapon_item(id);
        self.modify_damage(&weapon, entity, damage_source, damage)
    }

    pub fn crit(&self, id: EntityId, entity: EntityId) {
        let packet = crate::network::connection::encode(clientbound::ANIMATE, |w| {
            w.varint(entity);
            w.u8(1);
        });
        self.send_to_tracking_players_and_self(id, &packet);
    }

    pub fn magic_crit(&self, id: EntityId, entity: EntityId) {
        let packet = crate::network::connection::encode(clientbound::ANIMATE, |w| {
            w.varint(entity);
            w.u8(2);
        });
        self.send_to_tracking_players_and_self(id, &packet);
    }

    pub fn server_player_indicate_damage(&mut self, id: EntityId, xd: f64, zd: f64) {
        let p = self.player_mut(id);
        let hurt_dir = (mth::atan2(zd, xd) * 180.0f32 as f64 / std::f32::consts::PI as f64 - p.entity.y_rot as f64) as f32;
        p.living.hurt_dir = hurt_dir;
        p.connection.send(clientbound::HURT_ANIMATION, |w| {
            w.varint(id);
            w.f32(hurt_dir);
        });
    }

    pub fn server_player_jump_from_ground(&mut self, id: EntityId) {
        self.living_entity_jump_from_ground(id);
        let exhaustion = if self.entity(id).is_sprinting() { crate::world::food::constants::EXHAUSTION_SPRINT_JUMP } else { crate::world::food::constants::EXHAUSTION_JUMP };
        self.cause_food_exhaustion(id, exhaustion);
    }

    pub fn server_player_on_effect_added(&mut self, id: EntityId, effect: MobEffect, source: Option<EntityId>) {
        self.living_entity_on_effect_added(id, effect, source);
        let p = self.player(id);
        let instance = p.living.effect(effect).expect("added effect");
        p.connection.send_raw(update_mob_effect(id, instance, true));
    }

    pub fn server_player_on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, source: Option<EntityId>) {
        self.living_entity_on_effect_updated(id, effect, refresh_attributes, source);
        let p = self.player(id);
        let instance = p.living.effect(effect).expect("updated effect");
        p.connection.send_raw(update_mob_effect(id, instance, false));
    }

    pub fn server_player_on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        self.living_entity_on_effects_removed(id, effects);
        let p = self.player(id);
        for effect in effects {
            p.connection.send_raw(remove_mob_effect(id, effect.effect));
        }
    }

    pub fn send_active_player_effects(&self, id: EntityId) {
        let p = self.player(id);
        for effect in &p.living.active_effects {
            p.connection.send_raw(update_mob_effect(id, effect, false));
        }
    }

    pub fn send_system_message(&self, id: EntityId, message: &crate::network::chat::Component) {
        let p = self.player(id);
        if p.client.chat_visibility == crate::network::configuration::ChatVisibility::Hidden {
            return;
        }
        p.connection.send(clientbound::SYSTEM_CHAT, |w| {
            message.write(w);
            w.bool(false);
        });
    }
}
