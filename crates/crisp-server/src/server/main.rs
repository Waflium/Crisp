use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::dedicated::dedicated_server_settings::DedicatedServerSettings;
use super::dedicated::dedicated_server_properties::{DedicatedServerProperties, InvalidProperty};
use super::eula::Eula;
use super::packs::repository::PackRepository;
use super::packs::resources::MultiPackResourceManager;
use super::reloadable_server_resources::ReloadableServerResources;
use super::world_stem::WorldStem;
use super::configure_pack_repository;
use crate::world::flag::FeatureFlagSet;
use crate::world::level::data_pack_config::DataPackConfig;
use crate::world::level::world_data_configuration::WorldDataConfiguration;
use crate::world::level::chunk::ChunkGenerator;
use crate::world::level::levelgen::{NoiseBasedChunkGenerator, flat};
use crate::world::level::storage::{WorldData, WorldDir};
use crate::{Listen, ServerConfig, serve_world};

const HELP: &[&str] = &[
    "Option                 Description                                         ",
    "------                 -----------                                         ",
    "--bonusChest                                                               ",
    "--demo                                                                     ",
    "--eraseCache                                                               ",
    "--forceUpgrade                                                             ",
    "--help                                                                     ",
    "--initSettings         Initializes 'server.properties' and 'eula.txt', then",
    "                         quits                                             ",
    "--jfrProfile                                                               ",
    "--nogui                                                                    ",
    "--pidFile <Path>                                                           ",
    "--port <Integer>       (default: -1)                                       ",
    "--recreateRegionFiles                                                      ",
    "--safeMode             Loads level with vanilla datapack only              ",
    "--serverId <String>                                                        ",
    "--universe <String>    (default: .)                                        ",
    "--world <String>                                                           ",
];

const FLAGS: [&str; 10] = ["nogui", "initSettings", "demo", "bonusChest", "forceUpgrade", "eraseCache", "recreateRegionFiles", "safeMode", "help", "jfrProfile"];
const WITH_ARGUMENT: [&str; 5] = ["universe", "world", "port", "serverId", "pidFile"];

#[derive(Default)]
struct OptionSet {
    flags: Vec<&'static str>,
    values: Vec<(&'static str, String)>,
    non_options: Vec<String>,
}

impl OptionSet {
    fn has(&self, name: &str) -> bool {
        self.flags.contains(&name) || self.values.iter().any(|(n, _)| *n == name)
    }

    fn value_of(&self, name: &str) -> Option<&str> {
        self.values.iter().rev().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }

    fn parse(args: &[String]) -> Result<OptionSet, String> {
        let mut options = OptionSet::default();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            i += 1;
            if arg == "--" {
                options.non_options.extend(args[i..].iter().cloned());
                break;
            }
            let Some(body) = arg.strip_prefix("--").or_else(|| arg.strip_prefix('-').filter(|b| !b.is_empty())) else {
                options.non_options.push(arg.clone());
                continue;
            };
            let (name, inline) = match body.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (body, None),
            };
            let option = resolve(name)?;
            if FLAGS.contains(&option) {
                if inline.is_some() {
                    return Err(format!("joptsimple.UnrecognizedOptionException: {name} is not a recognized option"));
                }
                options.flags.push(option);
                continue;
            }
            let value = match inline {
                Some(value) => value,
                None if i < args.len() => {
                    i += 1;
                    args[i - 1].clone()
                }
                None => return Err(format!("joptsimple.OptionMissingRequiredArgumentException: Option {option} requires an argument")),
            };
            if option == "port" && value.parse::<i32>().is_err() {
                return Err(format!("joptsimple.OptionArgumentConversionException: Cannot parse argument '{value}' of option port"));
            }
            options.values.push((option, value));
        }
        Ok(options)
    }
}

fn resolve(name: &str) -> Result<&'static str, String> {
    let all = FLAGS.iter().chain(WITH_ARGUMENT.iter());
    if let Some(&exact) = all.clone().find(|&&o| o == name) {
        return Ok(exact);
    }
    let candidates: Vec<&'static str> = all.filter(|o| o.starts_with(name)).copied().collect();
    match candidates.as_slice() {
        [single] => Ok(single),
        [] => Err(format!("joptsimple.UnrecognizedOptionException: {name} is not a recognized option")),
        _ => Err(format!("joptsimple.UnrecognizedOptionException: {name} is not a recognized option")),
    }
}

pub fn main(args: Vec<String>) {
    std::panic::set_hook(Box::new(|info| {
        if std::thread::current().name() == Some("Server thread") {
            return;
        }
        let message = info.payload().downcast_ref::<String>().map(String::as_str).or_else(|| info.payload().downcast_ref::<&str>().copied()).unwrap_or("");
        error!("Caught previously unhandled exception :\n{message}");
    }));
    let options = match OptionSet::parse(&args) {
        Ok(options) => options,
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            return;
        }
    };
    if options.has("help") {
        let separator = if cfg!(windows) { "\r\n" } else { "\n" };
        eprint!("{}", HELP.iter().map(|l| format!("{l}{separator}")).collect::<String>());
        return;
    }
    if let Some(pid_file) = options.value_of("pidFile") {
        let _ = std::fs::write(pid_file, std::process::id().to_string());
    }
    let settings_file = Path::new("server.properties");
    let settings = match DedicatedServerSettings::new(settings_file) {
        Ok(settings) => settings,
        Err(InvalidProperty(e)) => {
            error!("Failed to start the minecraft server\n{e}");
            return;
        }
    };
    settings.force_save();
    crate::world::level::chunk::storage::region_file_version::RegionFileVersion::configure(&settings.properties().region_file_compression);
    let eula_file = Path::new("eula.txt");
    let eula = Eula::new(eula_file);
    if options.has("initSettings") {
        info!("Initialized '{}' and '{}'", absolute(settings_file).display(), absolute(eula_file).display());
        return;
    }
    if !eula.has_agreed_to_eula() {
        info!("You need to agree to the EULA in order to run the server. Go to eula.txt for more info.");
        return;
    }
    let universe = PathBuf::from(options.value_of("universe").unwrap_or("."));
    let next_io_thread = AtomicUsize::new(0);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name_fn(move || format!("Netty Server IO #{}", next_io_thread.fetch_add(1, Ordering::Relaxed)))
        .build()
        .expect("network runtime");
    let services = {
        let _context = runtime.enter();
        super::services::Services::create()
    };
    let properties = settings.properties();
    let (management_server, management_runtime) = match super::jsonrpc::create(properties) {
        Ok(Some((server, runtime))) => (Some(server), Some(runtime)),
        Ok(None) => (None, None),
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            runtime.shutdown_background();
            return;
        }
    };
    let level_name = options.value_of("world").map_or_else(|| properties.level_name.clone(), str::to_string);
    let _lock = match crate::util::directory_lock::DirectoryLock::create(&universe.join(&level_name)) {
        Ok(lock) => lock,
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            runtime.shutdown_background();
            return;
        }
    };
    let world_dir = WorldDir { root: universe.join(&level_name) };
    let level_data = match WorldData::load(&world_dir) {
        Ok(level_data) => level_data,
        Err(e) => {
            error!("Failed to load world data. World files may be corrupted. Shutting down.\n{e}");
            runtime.shutdown_background();
            return;
        }
    };
    let safe_mode = options.has("safeMode");
    if safe_mode {
        warn!("Safe mode active, only vanilla datapack will be loaded");
    }
    let mut pack_repository = PackRepository::create_pack_repository(world_dir.root.join("datapacks"));
    let (init_mode, initial_data_config) = load_or_create_config(properties, level_data.as_ref());
    let data_configuration = configure_pack_repository(&mut pack_repository, &initial_data_config, init_mode, safe_mode);
    let resource_manager = MultiPackResourceManager::new(pack_repository.open_all_selected());
    let worker_properties = properties.clone();
    let worker_data_configuration = data_configuration.clone();
    let mut world_data = std::thread::Builder::new()
        .name("Worker-Main-1".into())
        .spawn(move || {
            level_data.unwrap_or_else(|| {
                info!("No existing world data, creating new world");
                create_new_world_data(&worker_properties, worker_data_configuration)
            })
        })
        .expect("worker thread")
        .join()
        .expect("world data");
    let data_packs = ReloadableServerResources::load_resources(&resource_manager, properties.function_permission_level);
    world_data.data_configuration = data_configuration;
    if let Err(e) = world_data.save_level_data(&world_dir) {
        error!("Failed to save level {}\n{e}", world_dir.root.display());
    }
    let _ = std::fs::create_dir_all(world_dir.root.join("players"));
    let port = options.value_of("port").and_then(|p| p.parse::<i32>().ok()).filter(|&p| p >= 0).unwrap_or(properties.server_port);
    let listen = Listen::Bind { ip: properties.server_ip.clone(), port: port as u16 };
    let mut config = ServerConfig::from_properties(properties, &universe, level_name);
    if properties.code_of_conduct {
        match super::dedicated::read_code_of_conducts() {
            Ok(code_of_conducts) => config.code_of_conducts = code_of_conducts.into(),
            Err(e) => {
                error!("Failed to start the minecraft server\n{e}");
                runtime.shutdown_background();
                return;
            }
        }
    }
    let world_stem = WorldStem { data_packs, world_data };
    if let Err(e) = runtime.block_on(serve_world(listen, config, services, Some(settings), true, Some((pack_repository, world_stem)), management_server)) {
        error!("Encountered an unexpected exception\n{e:#}");
    }
    runtime.shutdown_background();
    if let Some(management_runtime) = management_runtime {
        management_runtime.shutdown_background();
    }
}

fn load_or_create_config(properties: &DedicatedServerProperties, level_data: Option<&WorldData>) -> (bool, WorldDataConfiguration) {
    match level_data {
        Some(level_data) => (false, level_data.data_configuration.clone()),
        None => {
            let initial_data_pack_configuration =
                DataPackConfig { enabled: properties.initial_enabled_packs.clone(), disabled: properties.initial_disabled_packs.clone() };
            (true, WorldDataConfiguration::new(initial_data_pack_configuration, FeatureFlagSet::DEFAULT_FLAGS))
        }
    }
}

fn create_new_world_data(properties: &DedicatedServerProperties, data_configuration: WorldDataConfiguration) -> WorldData {
    let seed = properties.seed.unwrap_or_else(rand::random);
    let generator = create_dimensions(properties, seed);
    WorldData::new_world(
        seed,
        generator,
        properties.generate_structures,
        properties.game_mode,
        properties.difficulty,
        properties.hardcore,
        data_configuration,
    )
}

fn create_dimensions(properties: &DedicatedServerProperties, seed: i64) -> ChunkGenerator {
    let presets = crisp_data::WORLD_PRESETS;
    let level_type = properties.level_type.as_str();
    let preset = crate::resources::identifier::parse(level_type)
        .ok()
        .or_else(|| match level_type {
            "default" => Some("minecraft:normal".to_string()),
            "largebiomes" => Some("minecraft:large_biomes".to_string()),
            _ => None,
        })
        .filter(|preset| presets.iter().any(|(name, _)| name == preset));
    let preset = preset.unwrap_or_else(|| {
        warn!("Failed to parse level-type {level_type}, defaulting to minecraft:normal");
        "minecraft:normal".to_string()
    });
    if preset == "minecraft:flat" {
        let settings: serde_json::Value = serde_json::from_str(if properties.generator_settings.is_empty() { "{}" } else { &properties.generator_settings })
            .unwrap_or_else(|_| serde_json::json!({}));
        let parsed = flat::parse_settings(&settings);
        if let Some(error) = parsed.error {
            error!("{error}");
        }
        return ChunkGenerator::Flat(parsed.value.unwrap_or_default());
    }
    ChunkGenerator::Noise(std::sync::Arc::new(NoiseBasedChunkGenerator::overworld(seed)))
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
