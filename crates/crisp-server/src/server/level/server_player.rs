use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Instant;

use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_data::{WORLD_VERSION, items, registry, registry_id};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::chunk_map::ChunkTrackingView;
use crate::advancements::triggers::criteria_triggers;
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
use crate::world::inventory::abstract_container_menu::AbstractContainerMenu;
use crate::world::item::component as dc;
use crate::world::item::{Inventory, ItemStack, OFFHAND, inventory_slot, map_item, protocol_slot};
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::entity::player::abilities::Abilities;
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::world::level::storage::RespawnData;
use crate::world::InteractionHand;
use crate::stats::server_stats_counter::ServerStatsCounter;
use crate::stats::stat::Stat;
use crate::stats::stats;
use crate::stats::stats_counter::StatsCounter;
use crate::world::food::constants::{EXHAUSTION_SPRINT, EXHAUSTION_SWIM};

pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.6, height: 1.8, eye_height: 1.62 };
const GAME_EVENT_WIN_GAME: u8 = 4;

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
    pub is_changing_dimension: bool,
    pub seen_credits: bool,
    pub won_game: bool,
    pub entered_nether_position: Option<Vec3>,
    pub levitation_start_pos: Option<Vec3>,
    pub levitation_start_time: i32,
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
    pub ender_chest_inventory: crate::world::inventory::player_ender_chest_container::PlayerEnderChestContainer,
    pub inventory_menu: Option<AbstractContainerMenu>,
    pub container_menu: Option<AbstractContainerMenu>,
    pub recipe_book: crate::stats::server_recipe_book::ServerRecipeBook,
    pub stats: ServerStatsCounter,
    pub advancements: crate::server::player_advancements::PlayerAdvancements,
    pub carried: Option<ItemStack>,
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
    pub post_effects: Vec<String>,
    pub post_effects_dirty: bool,
    pub camera: Option<EntityId>,
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
            is_changing_dimension: false,
            seen_credits: false,
            won_game: false,
            entered_nether_position: None,
            levitation_start_pos: None,
            levitation_start_time: 0,
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
            ender_chest_inventory: Default::default(),
            inventory_menu: Some(AbstractContainerMenu::inventory()),
            container_menu: None,
            recipe_book: Default::default(),
            stats: ServerStatsCounter::default(),
            advancements: Default::default(),
            carried: None,
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
            post_effects: Vec::new(),
            post_effects_dirty: true,
            camera: None,
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
        let stack_at = |slot: usize| self.inventory.slots[slot].as_ref().filter(|s| !s.is_empty());
        let equipment: Vec<(String, Nbt)> = [("offhand", OFFHAND), ("feet", 8), ("legs", 7), ("chest", 6), ("head", 5)]
            .into_iter()
            .filter_map(|(name, slot)| Some((name.to_string(), stack_at(slot)?.to_nbt())))
            .collect();
        if !equipment.is_empty() || stack_at(self.inventory.selected_slot()).is_some() {
            nbt.push(("equipment".into(), Nbt::Compound(equipment)));
        }
        nbt.push(("DataVersion".into(), Nbt::Int(WORLD_VERSION)));
        let inventory = (0..36)
            .filter_map(|index| {
                let stack = self.inventory.slots[inventory_slot(index)].as_ref().filter(|s| !s.is_empty())?;
                let Nbt::Compound(mut fields) = stack.to_nbt() else { return None };
                fields.insert(0, ("Slot".into(), Nbt::Byte(index as i8)));
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
        nbt.push(("EnderItems".into(), self.ender_chest_inventory.store_as_slots()));
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
        nbt.push(("seenCredits".into(), Nbt::Byte(self.seen_credits as i8)));
        if let Some(pos) = self.entered_nether_position {
            nbt.push(("entered_nether_pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        }
        nbt.push((crate::stats::server_recipe_book::RECIPE_BOOK_TAG.into(), self.recipe_book.pack()));
        nbt.push(("Dimension".into(), s(dimension)));
        if let Some(respawn) = &self.respawn_config {
            let mut fields = respawn.respawn_data.to_nbt_fields();
            if respawn.forced {
                fields.push(("forced".into(), Nbt::Byte(1)));
            }
            nbt.push(("respawn".into(), Nbt::Compound(fields)));
        }
        nbt.push(("post_effects".into(), Nbt::List(self.post_effects.iter().map(|p| s(p)).collect())));
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
        self.entered_nether_position = nbt.get("entered_nether_pos").map(Nbt::as_list).filter(|p| p.len() == 3).and_then(|p| {
            Some(Vec3::new(p[0].as_f64()?, p[1].as_f64()?, p[2].as_f64()?))
        });
        self.seen_credits = number("seenCredits").is_some_and(|v| v != 0.0);
        if let Some(post_effects) = nbt.get("post_effects").and_then(|list| {
            list.as_list().iter().map(|p| p.as_str().and_then(crate::commands::arguments::parse_identifier)).collect::<Option<Vec<String>>>()
        }) {
            self.post_effects.extend(post_effects);
        }
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
        if let Some(equipment) = nbt.get("equipment") {
            for (name, slot) in [("offhand", OFFHAND), ("feet", 8), ("legs", 7), ("chest", 6), ("head", 5)] {
                if let Some(Ok(stack)) = equipment.get(name).map(ItemStack::from_nbt) {
                    self.inventory.slots[slot] = Some(stack);
                }
            }
        }
        self.ender_chest_inventory.from_slots(nbt.get("EnderItems").map(Nbt::as_list).unwrap_or(&[]));
        if let Some(recipe_book) = nbt.get(crate::stats::server_recipe_book::RECIPE_BOOK_TAG) {
            self.recipe_book.load_untrusted(recipe_book);
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

    pub fn send_post_effects(&mut self) {
        self.post_effects_dirty = false;
        let post_effects = &self.post_effects;
        self.connection.send(clientbound::POST_EFFECTS, |w| {
            w.varint(post_effects.len() as i32);
            for post_effect in post_effects {
                w.string(post_effect);
            }
        });
    }

    pub fn add_post_effect(&mut self, post_effect: &str) -> bool {
        if self.post_effects.iter().any(|p| p == post_effect) {
            return false;
        }
        self.post_effects.push(post_effect.to_string());
        self.post_effects_dirty = true;
        true
    }

    pub fn clear_post_effects(&mut self) -> bool {
        if self.post_effects.is_empty() {
            return false;
        }
        self.post_effects.clear();
        self.post_effects_dirty = true;
        true
    }

    pub fn get_post_effects(&self) -> &[String] {
        &self.post_effects
    }

    pub fn remove_post_effect(&mut self, post_effect: &str) -> bool {
        match self.post_effects.iter().position(|p| p == post_effect) {
            Some(index) => {
                self.post_effects.remove(index);
                self.post_effects_dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn pick_up(&mut self, stack: ItemStack) -> i32 {
        self.inventory.add(stack).1
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

fn did_not_move(dx: f64, dy: f64, dz: f64) -> bool {
    dx == 0.0 && dy == 0.0 && dz == 0.0
}

impl ServerLevel {
    pub fn handle_extra_items_created_on_use(&mut self, id: EntityId, extra_items: ItemStack) {
        let left = self.player_mut(id).pick_up(extra_items.clone());
        if left > 0 {
            self.player_drop(id, extra_items.copy_with_count(left), false, crate::util::Prediction::Predicted);
        }
    }

    fn synchronize_special_item_updates(&mut self, id: EntityId, map_id: Option<MapId>) {
        let Some(data) = map_item::get_saved_data_by_id(map_id, self) else { return };
        let Some(map_id) = map_id else { return };
        if let Some(packet) = data.borrow_mut().get_update_packet(map_id, id) {
            self.player(id).connection.send(clientbound::MAP_ITEM_DATA, |w| packet.write(w));
        }
    }

    pub fn server_player_do_tick(&mut self, id: EntityId) {
        if !self.is_spectator(id) || !self.touching_unloaded_chunk(id) {
            self.player_tick(id);
            self.tick_food(id);
            self.award_stat(id, stats::PLAY_TIME, 1);
            self.award_stat(id, stats::TOTAL_WORLD_TIME, 1);
            if self.is_alive(id) {
                self.award_stat(id, stats::TIME_SINCE_DEATH, 1);
            }
            if self.entity(id).is_discrete() {
                self.award_stat(id, stats::CROUCH_TIME, 1);
            }
            if !self.is_sleeping(id) {
                self.award_stat(id, stats::TIME_SINCE_REST, 1);
            }
        }
        for i in 0..41 {
            let slot = crate::world::item::inventory_slot(i);
            let map_id = self.player(id).inventory.slots[slot].as_ref().filter(|stack| !stack.is_empty()).and_then(|stack| stack.get(dc::MAP_ID)).and_then(MapId::from_nbt);
            if map_id.is_some() {
                self.synchronize_special_item_updates(id, map_id);
            }
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
        if self.entity(id).tick_count % 20 == 0 {
            criteria_triggers::LOCATION.trigger(self, id);
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
        self.player_is_invulnerable_to(id, source)
            || self.player(id).is_changing_dimension && !source.is_type(crisp_data::damage_types::ENDER_PEARL)
            || !self.player(id).movement.has_client_loaded()
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
            let killer_type = self.entity(killer).entity_type;
            self.award_stat(id, stats::ENTITY_KILLED_BY.get(killer_type as u16), 1);
            self.award_kill_score(killer, id, &source);
        }
        self.broadcast_entity_event(id, entity_event::DEATH);
        self.award_stat(id, stats::DEATHS, 1);
        self.reset_stat(id, stats::TIME_SINCE_DEATH);
        self.reset_stat(id, stats::TIME_SINCE_REST);
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

    pub fn server_player_award_kill_score(&mut self, id: EntityId, victim: EntityId, killing_blow: &DamageSource) {
        if victim == id {
            return;
        }
        self.entity_award_kill_score(id, victim, killing_blow);
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&ObjectiveCriteria::KILL_COUNT_ALL, &holder, |level, score| {
            let _ = score.increment(level);
        });
        if self.is_player(victim) {
            self.award_stat(id, stats::PLAYER_KILLS, 1);
            self.for_all_objectives(&ObjectiveCriteria::KILL_COUNT_PLAYERS, &holder, |level, score| {
                let _ = score.increment(level);
            });
        } else {
            self.award_stat(id, stats::MOB_KILLS, 1);
        }
        criteria_triggers::PLAYER_KILLED_ENTITY.trigger(self, id, victim, killing_blow);
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

    pub fn check_movement_statistics(&mut self, id: EntityId, dx: f64, dy: f64, dz: f64) {
        if did_not_move(dx, dy, dz) {
            return;
        }
        let distance = |d: f64| mth::java_round_f32((d.sqrt() as f32) * 100.0);
        let e = self.entity(id);
        if e.is_swimming() {
            let distance = distance(dx * dx + dy * dy + dz * dz);
            if distance > 0 {
                self.award_stat(id, stats::SWIM_ONE_CM, distance);
                self.cause_food_exhaustion(id, EXHAUSTION_SWIM * distance as f32 * 0.01);
            }
        } else if e.is_eye_in_fluid(&crisp_data::fluid_tags::WATER) {
            let distance = distance(dx * dx + dy * dy + dz * dz);
            if distance > 0 {
                self.award_stat(id, stats::WALK_UNDER_WATER_ONE_CM, distance);
                self.cause_food_exhaustion(id, EXHAUSTION_SWIM * distance as f32 * 0.01);
            }
        } else if e.is_in_water() {
            let horizontal_distance = distance(dx * dx + dz * dz);
            if horizontal_distance > 0 {
                self.award_stat(id, stats::WALK_ON_WATER_ONE_CM, horizontal_distance);
                self.cause_food_exhaustion(id, EXHAUSTION_SWIM * horizontal_distance as f32 * 0.01);
            }
        } else if self.on_climbable(id) {
            if dy > 0.0 {
                self.award_stat(id, stats::CLIMB_ONE_CM, mth::java_round(dy * 100.0) as i32);
            }
        } else if self.entity(id).on_ground {
            let horizontal_distance = distance(dx * dx + dz * dz);
            if horizontal_distance > 0 {
                let e = self.entity(id);
                if e.is_sprinting() {
                    self.award_stat(id, stats::SPRINT_ONE_CM, horizontal_distance);
                    self.cause_food_exhaustion(id, EXHAUSTION_SPRINT * horizontal_distance as f32 * 0.01);
                } else if e.is_crouching() {
                    self.award_stat(id, stats::CROUCH_ONE_CM, horizontal_distance);
                    self.cause_food_exhaustion(id, 0.0);
                } else {
                    self.award_stat(id, stats::WALK_ONE_CM, horizontal_distance);
                    self.cause_food_exhaustion(id, 0.0);
                }
            }
        } else if self.entity(id).is_fall_flying() {
            let distance = distance(dx * dx + dy * dy + dz * dz);
            self.award_stat(id, stats::AVIATE_ONE_CM, distance);
        } else {
            let horizontal_distance = distance(dx * dx + dz * dz);
            if horizontal_distance > 25 {
                self.award_stat(id, stats::FLY_ONE_CM, horizontal_distance);
            }
        }
    }

    pub fn award_stat(&mut self, id: EntityId, stat: Stat, count: i32) {
        let Some(player) = self.players.get_mut(&id) else { return };
        player.stats.increment(stat, count);
        let criteria = stat.criteria();
        if self.scoreboard.borrow().objectives_by_criteria(&criteria).is_empty() {
            return;
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&criteria, &holder, |level, score| {
            let _ = score.add(level, count);
        });
    }

    pub fn reset_stat(&mut self, id: EntityId, stat: Stat) {
        let Some(player) = self.players.get_mut(&id) else { return };
        player.stats.set_value(stat, 0);
        let criteria = stat.criteria();
        if self.scoreboard.borrow().objectives_by_criteria(&criteria).is_empty() {
            return;
        }
        let holder = ScoreHolder::for_entity(self, id);
        self.for_all_objectives(&criteria, &holder, |level, score| {
            let _ = score.reset(level);
        });
    }

    pub fn server_player_jump_from_ground(&mut self, id: EntityId) {
        self.living_entity_jump_from_ground(id);
        self.award_stat(id, stats::JUMP, 1);
        let exhaustion = if self.entity(id).is_sprinting() { crate::world::food::constants::EXHAUSTION_SPRINT_JUMP } else { crate::world::food::constants::EXHAUSTION_JUMP };
        self.cause_food_exhaustion(id, exhaustion);
    }

    pub fn server_player_on_effect_added(&mut self, id: EntityId, effect: MobEffect, source: Option<EntityId>) {
        self.living_entity_on_effect_added(id, effect, source);
        let p = self.player(id);
        let instance = p.living.effect(effect).expect("added effect");
        p.connection.send_raw(update_mob_effect(id, instance, true));
        if effect == crisp_data::mob_effects::LEVITATION {
            let tick_count = self.entity(id).tick_count;
            let p = self.player_mut(id);
            p.levitation_start_time = tick_count;
            p.levitation_start_pos = Some(p.entity.position());
        }
        criteria_triggers::EFFECTS_CHANGED.trigger(self, id, source);
    }

    pub fn server_player_on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, source: Option<EntityId>) {
        self.living_entity_on_effect_updated(id, effect, refresh_attributes, source);
        let p = self.player(id);
        let instance = p.living.effect(effect).expect("updated effect");
        p.connection.send_raw(update_mob_effect(id, instance, false));
        criteria_triggers::EFFECTS_CHANGED.trigger(self, id, source);
    }

    pub fn server_player_on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        self.living_entity_on_effects_removed(id, effects);
        let p = self.player_mut(id);
        for effect in effects {
            p.connection.send_raw(remove_mob_effect(id, effect.effect));
            if effect.effect == crisp_data::mob_effects::LEVITATION {
                p.levitation_start_pos = None;
            }
        }
        criteria_triggers::EFFECTS_CHANGED.trigger(self, id, None);
    }

    pub fn show_end_credits(&mut self, id: EntityId) {
        self.player_remove(id, crate::world::entity::RemovalReason::ChangedDimension);
        self.update_sleeping_player_list();
        let p = self.player_mut(id);
        if !p.won_game {
            p.won_game = true;
            p.connection.send(clientbound::GAME_EVENT, |w| {
                w.u8(GAME_EVENT_WIN_GAME);
                w.f32(0.0);
            });
            p.seen_credits = true;
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

    pub fn get_camera(&self, id: EntityId) -> EntityId {
        self.player(id).camera.unwrap_or(id)
    }

    pub fn set_camera(&mut self, id: EntityId, new_camera: Option<EntityId>) {
        let old_camera = self.get_camera(id);
        let camera = new_camera.unwrap_or(id);
        self.player_mut(id).camera = Some(camera);
        if old_camera != camera {
            if let Some(pos) = self.entity_ref(camera).map(Entity::position) {
                self.server_player_teleport_in_level(id, pos);
            }
            self.on_camera_changed(id, camera, new_camera.is_some());
        }
    }

    fn server_player_teleport_in_level(&mut self, id: EntityId, pos: Vec3) {
        if self.is_sleeping(id) {
            self.stop_sleep_in_bed(id, true, true);
        }
        let player = self.player_mut(id);
        let (y_rot, x_rot) = (player.entity.y_rot, player.entity.x_rot);
        let destination = crate::world::entity::relative::PositionMoveRotation { position: pos, delta_movement: Vec3::ZERO, y_rot, x_rot };
        player.teleport_relative(destination, 0);
        player.reset_position();
        player.entity.y_head_rot = y_rot;
        player.movement.above_ground_tick_count = 0;
    }

    pub(crate) fn on_camera_changed(&mut self, id: EntityId, camera: EntityId, moved: bool) {
        if moved {
            self.move_player(id);
        }
        let player = self.player_mut(id);
        player.connection.send(clientbound::SET_CAMERA, |w| w.varint(camera));
        player.reset_position();
    }

    pub fn tick_camera(&mut self, id: EntityId) {
        let camera = self.get_camera(id);
        if camera == id {
            return;
        }
        if self.entity_ref(camera).is_some() && self.is_alive(camera) {
            let e = self.entity(camera);
            let (pos, y_rot, x_rot) = (e.position(), e.y_rot, e.x_rot);
            let player = &mut self.player_mut(id).entity;
            player.abs_snap_to(pos.x, pos.y, pos.z);
            player.abs_snap_rotation_to(y_rot, x_rot);
            self.move_player(id);
            if self.player(id).entity.is_shift_key_down() {
                self.set_camera(id, Some(id));
            }
        } else {
            self.set_camera(id, Some(id));
        }
    }
}

impl crate::server::Server {
    pub fn set_camera(&mut self, id: EntityId, new_camera: Option<EntityId>) {
        let level = self.level_of(id).expect("player on this server");
        let camera = new_camera.unwrap_or(id);
        match self.level_of(camera) {
            Some(camera_level) if camera_level != level && self.levels[level].get_camera(id) != camera => {
                self.player_mut(id).camera = Some(camera);
                let pos = self.levels[camera_level].entity(camera).position();
                let (y_rot, x_rot) = (self.player(id).entity.y_rot, self.player(id).entity.x_rot);
                self.teleport_to(id, camera_level, pos, 0, y_rot, x_rot, false);
                let level = self.level_of(id).expect("player on this server");
                self.levels[level].on_camera_changed(id, camera, new_camera.is_some());
            }
            _ => self.levels[level].set_camera(id, new_camera),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn server_player_teleport_to(&mut self, id: EntityId, level: usize, pos: Vec3, relatives: u16, y_rot: f32, x_rot: f32, reset_camera: bool) -> bool {
        let current = self.level_of(id).expect("player on this server");
        if self.levels[current].is_sleeping(id) {
            self.levels[current].stop_sleep_in_bed(id, true, true);
        }
        if reset_camera {
            self.set_camera(id, Some(id));
        }
        let transition = TeleportTransition::with_relatives(level, pos, Vec3::ZERO, y_rot, x_rot, relatives, PostTeleportTransition::DO_NOTHING);
        let success = self.server_player_teleport(id, transition).is_some();
        if success {
            let p = self.player_mut(id);
            p.entity.y_head_rot = if relatives & crate::world::entity::relative::relative::Y_ROT != 0 { p.entity.y_head_rot + y_rot } else { y_rot };
            p.movement.above_ground_tick_count = 0;
        }
        success
    }

    pub fn server_player_teleport(&mut self, id: EntityId, transition: TeleportTransition) -> Option<EntityId> {
        let old_level = self.level_of(id)?;
        if self.levels[old_level].entity(id).is_removed() {
            return None;
        }
        let out = self.player(id).connection.clone();
        if transition.missing_respawn_block {
            out.send(clientbound::GAME_EVENT, |w| {
                w.u8(0);
                w.f32(0.0);
            });
        }
        let new_level = transition.new_level;
        if new_level == old_level {
            let level = &mut self.levels[old_level];
            let player = level.player_mut(id);
            player.teleport_relative(transition.position_move_rotation(), transition.relatives);
            player.reset_position();
            transition.post_teleport_transition.on_transition(level, id);
            return Some(id);
        }
        self.player_mut(id).is_changing_dimension = true;
        out.send(clientbound::RESPAWN, |w| {
            self.create_common_spawn_info(w, new_level, self.player(id));
            w.u8(3);
        });
        out.send(clientbound::CHANGE_DIFFICULTY, |w| {
            w.varint(self.difficulty.id());
            w.bool(self.difficulty_locked);
        });
        self.send_player_permission_level(id);
        let mut player = self.levels[old_level].remove_player_immediately(id, crate::world::entity::RemovalReason::ChangedDimension);
        player.entity.removal = None;
        if self.levels[old_level].dimension == crate::server::OVERWORLD && self.levels[new_level].dimension == crate::server::THE_NETHER {
            player.entered_nether_position = Some(player.entity.position());
        }
        player.teleport_relative(transition.position_move_rotation(), transition.relatives);
        player.reset_position();
        self.levels[new_level].add_player(player);
        self.trigger_dimension_change_triggers(id, old_level);
        let level = &mut self.levels[new_level];
        level.stop_using_item(id);
        let player = level.player(id);
        player.connection.send(clientbound::PLAYER_ABILITIES, |w| player.abilities.write(w));
        self.send_level_info(&out, new_level);
        self.send_all_player_info(id);
        let level = &mut self.levels[new_level];
        level.send_active_player_effects(id);
        level.player_mut(id).send_post_effects();
        transition.post_teleport_transition.on_transition(level, id);
        let player = level.player_mut(id);
        player.last_sent_exp = -1;
        player.last_sent_health = -1.0;
        player.last_sent_food = -1;
        self.teleport_spectators(id, transition, old_level);
        Some(id)
    }

    pub fn teleport_spectators(&mut self, entity: EntityId, transition: TeleportTransition, old_level: usize) {
        let spectators: Vec<EntityId> = self.levels[old_level].players.values().filter(|p| p.camera == Some(entity)).map(|p| p.entity.id).collect();
        for spectator in spectators {
            self.teleport(spectator, transition);
            self.set_camera(spectator, None);
        }
    }

    fn trigger_dimension_change_triggers(&mut self, id: EntityId, old_level: usize) {
        let new_level = self.level_of(id).expect("player on this server");
        let old_key = self.levels[old_level].dimension;
        let new_key = self.levels[new_level].dimension;
        let level = &mut self.levels[new_level];
        criteria_triggers::CHANGED_DIMENSION.trigger(level, id, old_key, new_key);
        if old_key == crate::server::THE_NETHER
            && new_key == crate::server::OVERWORLD
            && let Some(entered_nether_position) = level.player(id).entered_nether_position
        {
            criteria_triggers::NETHER_TRAVEL.trigger(level, id, entered_nether_position);
        }
        if self.levels[new_level].dimension != crate::server::THE_NETHER {
            self.player_mut(id).entered_nether_position = None;
        }
    }

    pub fn send_all_player_info(&mut self, id: EntityId) {
        let level = self.entity_level_mut(id);
        if let Some(mut menu) = level.player_mut(id).inventory_menu.take() {
            level.menu_send_all_data_to_remote(&mut menu, id);
            level.player_mut(id).inventory_menu = Some(menu);
        }
        let player = level.player_mut(id);
        player.last_sent_health = -1.0e8;
        let selected = player.inventory.selected as i32;
        player.connection.send(clientbound::SET_HELD_SLOT, |w| w.varint(selected));
    }
}
