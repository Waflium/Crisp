use std::collections::VecDeque;
use std::time::Duration;

use crisp_data::packets::configuration::{clientbound, serverbound};
use crisp_data::{SYNCED_REGISTRIES, VERSION_NAME};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::ServerConfig;
use crate::network::chat::{Arg, Component};
use crate::network::connection::{PacketReader, PacketWriter, disconnection_reason};
use crate::network::login::Profile;
use crate::server::Event;
use crate::server::dedicated::dedicated_server_properties::ServerResourcePackInfo;
use crate::world::level::storage::format_uuid;

pub struct ClientInfo {
    pub language: String,
    pub view_distance: i8,
    pub chat_visibility: ChatVisibility,
    pub chat_colors: bool,
    pub skin_parts: u8,
    pub main_hand: i32,
    pub text_filtering: bool,
    pub allows_listing: bool,
    pub particle_status: i32,
}

impl Default for ClientInfo {
    fn default() -> ClientInfo {
        ClientInfo {
            language: "en_us".into(),
            view_distance: 2,
            chat_visibility: ChatVisibility::Full,
            chat_colors: true,
            skin_parts: 0,
            main_hand: 1,
            text_filtering: false,
            allows_listing: false,
            particle_status: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatVisibility {
    #[default]
    Full,
    System,
    Hidden,
}

pub struct CommonListenerCookie {
    pub latency: i32,
    pub client_information: ClientInfo,
    pub transferred: bool,
}

impl CommonListenerCookie {
    pub fn create_initial(transferred: bool) -> CommonListenerCookie {
        CommonListenerCookie { latency: 0, client_information: ClientInfo::default(), transferred }
    }
}

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SERVER_THREAD: &str = "Server thread";
const RESOURCE_PACK_DECLINED: i32 = 1;
const RESOURCE_PACK_ACCEPTED: i32 = 3;
const RESOURCE_PACK_DOWNLOADED: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigurationTask {
    SynchronizeRegistries,
    ServerCodeOfConduct,
    ServerResourcePack,
    PrepareSpawn,
    JoinWorld,
}

impl ConfigurationTask {
    fn id(self) -> &'static str {
        match self {
            ConfigurationTask::SynchronizeRegistries => "synchronize_registries",
            ConfigurationTask::ServerCodeOfConduct => "server_code_of_conduct",
            ConfigurationTask::ServerResourcePack => "server_resource_pack",
            ConfigurationTask::PrepareSpawn => "prepare_spawn",
            ConfigurationTask::JoinWorld => "join_world",
        }
    }
}

struct ServerConfigurationPacketListener<'a> {
    config: &'a ServerConfig,
    profile: &'a Profile,
    events: &'a mpsc::UnboundedSender<Event>,
    prepare_spawn: Option<oneshot::Receiver<()>>,
    spawn_requested: bool,
    configuration_tasks: VecDeque<ConfigurationTask>,
    current_task: Option<ConfigurationTask>,
    client_information: ClientInfo,
    keep_alive_time: Instant,
    keep_alive_pending: bool,
    keep_alive_challenge: i64,
    latency: i32,
}

pub async fn configure(
    r: &mut PacketReader,
    w: &mut PacketWriter,
    config: &ServerConfig,
    profile: &Profile,
    events: &mpsc::UnboundedSender<Event>,
    cookie: CommonListenerCookie,
) -> Option<CommonListenerCookie> {
    let mut listener = ServerConfigurationPacketListener {
        config,
        profile,
        events,
        prepare_spawn: None,
        spawn_requested: false,
        configuration_tasks: VecDeque::new(),
        current_task: None,
        client_information: cookie.client_information,
        keep_alive_time: Instant::now(),
        keep_alive_pending: false,
        keep_alive_challenge: 0,
        latency: cookie.latency,
    };
    match listener.run(r, w).await {
        Ok(()) => Some(CommonListenerCookie { latency: listener.latency, client_information: listener.client_information, transferred: cookie.transferred }),
        Err(reason) => {
            info!(thread: SERVER_THREAD, "{} ({}) lost connection: {}", profile.name, format_uuid(profile.uuid), reason.get_string());
            if listener.spawn_requested {
                let _ = events.send(Event::PrepareSpawnClosed { uuid: profile.uuid });
            }
            None
        }
    }
}

impl ServerConfigurationPacketListener<'_> {
    async fn run(&mut self, r: &mut PacketReader, w: &mut PacketWriter) -> Result<(), Component> {
        self.start_configuration(w);
        w.flush().await.map_err(|e| disconnection_reason(&e.into()))?;
        let keep_alive = tokio::time::sleep_until(self.keep_alive_time + KEEP_ALIVE_INTERVAL);
        tokio::pin!(keep_alive);
        loop {
            let packet = tokio::select! {
                packet = r.recv() => packet.map_err(|e| disconnection_reason(&e))?,
                () = wait_for_spawn(&mut self.prepare_spawn) => {
                    self.prepare_spawn = None;
                    self.finish_current_task(w, ConfigurationTask::PrepareSpawn)?;
                    w.flush().await.map_err(|e| disconnection_reason(&e.into()))?;
                    continue;
                }
                _ = &mut keep_alive => {
                    if self.keep_alive_pending {
                        return Err(self.disconnect(w, Component::translatable("disconnect.timeout")).await);
                    }
                    self.keep_alive_pending = true;
                    self.keep_alive_time = Instant::now();
                    self.keep_alive_challenge = crate::util::get_millis();
                    let challenge = self.keep_alive_challenge;
                    w.queue(clientbound::KEEP_ALIVE, |p| p.i64(challenge));
                    keep_alive.as_mut().reset(self.keep_alive_time + KEEP_ALIVE_INTERVAL);
                    w.flush().await.map_err(|e| disconnection_reason(&e.into()))?;
                    continue;
                }
            };
            let joined = self.handle_packet(w, packet.id, &packet.data).await?;
            w.flush().await.map_err(|e| disconnection_reason(&e.into()))?;
            if joined {
                return Ok(());
            }
        }
    }

    fn start_configuration(&mut self, w: &mut PacketWriter) {
        w.queue(clientbound::CUSTOM_PAYLOAD, |p| {
            p.string("minecraft:brand");
            p.string("vanilla");
        });
        if !self.config.server_links.is_empty() {
            w.queue(clientbound::SERVER_LINKS, |p| crate::server::server_links::write_untrusted(&self.config.server_links, p));
        }
        w.queue(clientbound::UPDATE_ENABLED_FEATURES, |p| {
            p.varint(1);
            p.string("minecraft:vanilla");
        });
        self.configuration_tasks.push_back(ConfigurationTask::SynchronizeRegistries);
        self.add_optional_tasks();
        self.configuration_tasks.push_back(ConfigurationTask::PrepareSpawn);
        self.configuration_tasks.push_back(ConfigurationTask::JoinWorld);
        self.start_next_task(w);
    }

    fn add_optional_tasks(&mut self) {
        if !self.config.code_of_conducts.is_empty() {
            self.configuration_tasks.push_back(ConfigurationTask::ServerCodeOfConduct);
        }
        if self.config.server_resource_pack.is_some() {
            self.configuration_tasks.push_back(ConfigurationTask::ServerResourcePack);
        }
    }

    fn start_next_task(&mut self, w: &mut PacketWriter) {
        let Some(task) = self.configuration_tasks.pop_front() else { return };
        self.current_task = Some(task);
        match task {
            ConfigurationTask::SynchronizeRegistries => w.queue(clientbound::SELECT_KNOWN_PACKS, |p| {
                p.varint(1);
                p.string("minecraft");
                p.string("core");
                p.string(VERSION_NAME);
            }),
            ConfigurationTask::ServerCodeOfConduct => {
                let code_of_conduct = self.code_of_conduct();
                w.queue(clientbound::CODE_OF_CONDUCT, |p| p.string(code_of_conduct));
            }
            ConfigurationTask::ServerResourcePack => {
                let info = self.config.server_resource_pack.as_ref().expect("server resource pack");
                w.queue(clientbound::RESOURCE_PACK_PUSH, |p| write_resource_pack_push(info, p));
            }
            ConfigurationTask::PrepareSpawn => {
                let (reply, ready) = oneshot::channel();
                self.spawn_requested = self.events.send(Event::PrepareSpawn { uuid: self.profile.uuid, name: self.profile.name.clone(), reply }).is_ok();
                self.prepare_spawn = Some(ready);
            }
            ConfigurationTask::JoinWorld => w.queue(clientbound::FINISH_CONFIGURATION, |_| {}),
        }
    }

    fn code_of_conduct(&self) -> &str {
        let code_of_conducts = &self.config.code_of_conducts;
        let language = self.client_information.language.to_lowercase();
        let find = |language: &str| code_of_conducts.iter().find(|(l, _)| l == language).map(|(_, c)| c.as_str());
        find(&language).or_else(|| find("en_us")).unwrap_or(&code_of_conducts[0].1)
    }

    fn finish_current_task(&mut self, w: &mut PacketWriter, task: ConfigurationTask) -> Result<(), Component> {
        if self.current_task != Some(task) {
            let current = self.current_task.map_or("null", ConfigurationTask::id);
            return Err(internal_exception(&format!(
                "java.lang.IllegalStateException: Unexpected request for task finish, current task: {current}, requested: {}",
                task.id()
            )));
        }
        self.current_task = None;
        self.start_next_task(w);
        Ok(())
    }

    async fn handle_packet(&mut self, w: &mut PacketWriter, id: i32, data: &[u8]) -> Result<bool, Component> {
        let result = match id {
            serverbound::CLIENT_INFORMATION => {
                self.client_information = read_client_information(data).map_err(|_| decoder_exception())?;
                Ok(())
            }
            serverbound::SELECT_KNOWN_PACKS => self.handle_select_known_packs(w, data),
            serverbound::ACCEPT_CODE_OF_CONDUCT => self.finish_current_task(w, ConfigurationTask::ServerCodeOfConduct),
            serverbound::RESOURCE_PACK => self.handle_resource_pack_response(w, data),
            serverbound::KEEP_ALIVE => self.handle_keep_alive(data),
            serverbound::FINISH_CONFIGURATION => return self.finish_current_task(w, ConfigurationTask::JoinWorld).map(|()| true),
            serverbound::COOKIE_RESPONSE => Err(Component::translatable("multiplayer.disconnect.unexpected_query_response")),
            _ => Ok(()),
        };
        match result {
            Ok(()) => Ok(false),
            Err(reason) => Err(self.disconnect(w, reason).await),
        }
    }

    fn handle_select_known_packs(&mut self, w: &mut PacketWriter, data: &[u8]) -> Result<(), Component> {
        if self.current_task != Some(ConfigurationTask::SynchronizeRegistries) {
            return Err(internal_exception(
                "java.lang.IllegalStateException: Unexpected response from client: received pack selection, but no negotiation ongoing",
            ));
        }
        let mut r = crisp_protocol::Reader::new(data);
        let read = |r: &mut crisp_protocol::Reader| -> anyhow::Result<Vec<(String, String, String)>> {
            let count = r.varint()?;
            (0..count).map(|_| Ok((r.string(32767)?.to_owned(), r.string(32767)?.to_owned(), r.string(32767)?.to_owned()))).collect()
        };
        let accepted = read(&mut r).map_err(|_| decoder_exception())?;
        if accepted != [("minecraft".to_string(), "core".to_string(), VERSION_NAME.to_string())] {
            return Err(internal_exception("java.lang.IllegalStateException: client does not have the minecraft:core data pack"));
        }
        send_registries(w);
        self.finish_current_task(w, ConfigurationTask::SynchronizeRegistries)
    }

    fn handle_resource_pack_response(&mut self, w: &mut PacketWriter, data: &[u8]) -> Result<(), Component> {
        let mut r = crisp_protocol::Reader::new(data);
        let (pack, action) = (|| -> anyhow::Result<(u128, i32)> { Ok((r.uuid()?, r.varint()?)) })().map_err(|_| decoder_exception())?;
        if !(0..8).contains(&action) {
            return Err(decoder_exception());
        }
        if action == RESOURCE_PACK_DECLINED && self.config.server_resource_pack.as_ref().is_some_and(|i| i.is_required) {
            info!(thread: SERVER_THREAD, "Disconnecting {} due to resource pack {} rejection", self.profile.name, format_uuid(pack));
            return Err(Component::translatable("multiplayer.requiredTexturePrompt.disconnect"));
        }
        if action != RESOURCE_PACK_ACCEPTED && action != RESOURCE_PACK_DOWNLOADED {
            self.finish_current_task(w, ConfigurationTask::ServerResourcePack)?;
        }
        Ok(())
    }

    fn handle_keep_alive(&mut self, data: &[u8]) -> Result<(), Component> {
        let id = crisp_protocol::Reader::new(data).i64().map_err(|_| decoder_exception())?;
        if self.keep_alive_pending && id == self.keep_alive_challenge {
            let time = self.keep_alive_time.elapsed().as_millis() as i32;
            self.latency = (self.latency * 3 + time) / 4;
            self.keep_alive_pending = false;
            return Ok(());
        }
        Err(Component::translatable("disconnect.timeout"))
    }

    async fn disconnect(&self, w: &mut PacketWriter, reason: Component) -> Component {
        let _ = w.send(clientbound::DISCONNECT, |p| reason.write(p)).await;
        reason
    }
}

fn internal_exception(exception: &str) -> Component {
    Component::translatable_with("disconnect.genericReason", vec![Arg::String(format!("Internal Exception: {exception}"))])
}

fn decoder_exception() -> Component {
    internal_exception("io.netty.handler.codec.DecoderException: Failed to decode packet")
}

fn send_registries(w: &mut PacketWriter) {
    for registry in SYNCED_REGISTRIES {
        w.queue(clientbound::REGISTRY_DATA, |p| {
            p.string(registry.name);
            p.varint(registry.entries.len() as i32);
            for entry in registry.entries {
                p.string(entry);
                p.bool(false);
            }
        });
    }
    w.queue(clientbound::UPDATE_TAGS, write_tags);
}

const UNSYNCHRONIZED_DATA_REGISTRIES: &[&str] = &[
    "minecraft:advancement",
    "minecraft:worldgen/carver",
    "minecraft:context_float_provider",
    "minecraft:context_int_provider",
    "minecraft:worldgen/density_function",
    "minecraft:enchantment_provider",
    "minecraft:worldgen/feature",
    "minecraft:worldgen/flat_level_generator_preset",
    "minecraft:item_modifier",
    "minecraft:dimension",
    "minecraft:loot_table",
    "minecraft:worldgen/material_condition",
    "minecraft:worldgen/material_rule",
    "minecraft:worldgen/multi_noise_biome_source_parameter_list",
    "minecraft:worldgen/noise",
    "minecraft:worldgen/noise_settings",
    "minecraft:worldgen/placed_feature",
    "minecraft:predicate",
    "minecraft:worldgen/processor_list",
    "minecraft:recipe",
    "minecraft:slot_source",
    "minecraft:worldgen/structure",
    "minecraft:worldgen/structure_set",
    "minecraft:worldgen/template_pool",
    "minecraft:trade_set",
    "minecraft:trial_spawner",
    "minecraft:villager_trade",
    "minecraft:worldgen/world_preset",
];

fn is_network_safe(registry: &str) -> bool {
    !UNSYNCHRONIZED_DATA_REGISTRIES.contains(&registry)
}

pub fn write_tags(p: &mut crisp_protocol::Writer) {
    let registries: Vec<_> = crisp_data::tags().iter().filter(|registry| is_network_safe(registry.registry)).collect();
    p.varint(registries.len() as i32);
    for registry in registries {
        p.string(registry.registry);
        p.varint(registry.tags.len() as i32);
        for (name, ids) in registry.tags {
            p.string(name);
            p.varint(ids.len() as i32);
            ids.iter().for_each(|&id| p.varint(id));
        }
    }
}

pub fn write_resource_pack_push(info: &ServerResourcePackInfo, p: &mut crisp_protocol::Writer) {
    p.uuid(info.id);
    p.string(&info.url);
    p.string(&info.hash);
    p.bool(info.is_required);
    p.bool(info.prompt.is_some());
    if let Some(prompt) = &info.prompt {
        prompt.write(p);
    }
}

pub fn read_client_information(data: &[u8]) -> anyhow::Result<ClientInfo> {
    let mut r = crisp_protocol::Reader::new(data);
    Ok(ClientInfo {
        language: r.string(16)?.to_owned(),
        view_distance: r.u8()? as i8,
        chat_visibility: match r.varint()? {
            1 => ChatVisibility::System,
            2 => ChatVisibility::Hidden,
            _ => ChatVisibility::Full,
        },
        chat_colors: r.bool()?,
        skin_parts: r.u8()?,
        main_hand: r.varint()?,
        text_filtering: r.bool()?,
        allows_listing: r.bool()?,
        particle_status: r.varint()?,
    })
}

async fn wait_for_spawn(prepare_spawn: &mut Option<oneshot::Receiver<()>>) {
    if let Some(ready) = prepare_spawn {
        if ready.await.is_ok() {
            return;
        }
        *prepare_spawn = None;
    }
    std::future::pending().await
}
