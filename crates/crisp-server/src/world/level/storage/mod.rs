pub mod command_storage;
pub mod loot;

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crisp_core::BlockPos;
use crisp_data::{Block, BlockState, VERSION_NAME, WORLD_VERSION};
use crisp_protocol::nbt::Nbt;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::server::level::game_mode::GameType;
use crate::world::difficulty::Difficulty;
use crate::world::level::chunk::storage::{decode_nbt, encode_nbt};
use crate::world::level::chunk::{Biome, ChunkGenerator, FlatLevelSource};
use crate::world::level::levelgen::NoiseBasedChunkGenerator;
use crate::world::level::levelgen::flat::StructureOverrides;
use crate::world::level::saveddata::weather_data::WeatherData;
use crate::world::level::validation::directory_validator::{DirectoryValidator, SymlinkTargetAllowList};
use crate::world::level::validation::path_allow_list::PathAllowList;
use crate::world::level::world_data_configuration::WorldDataConfiguration;

const LEVEL_NBT_VERSION: i32 = 19133;

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

pub fn read_dat(path: &Path) -> io::Result<Option<Nbt>> {
    let Ok(file) = File::open(path) else { return Ok(None) };
    let mut bytes = Vec::new();
    GzDecoder::new(file).read_to_end(&mut bytes)?;
    decode_nbt(&bytes).map(Some)
}

pub fn write_dat(path: &Path, nbt: &Nbt) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("dat_tmp");
    let mut gz = GzEncoder::new(File::create(&tmp)?, Compression::default());
    gz.write_all(&encode_nbt(nbt))?;
    gz.finish()?.sync_all()?;
    fs::rename(tmp, path)
}

fn save_level_dat(dir: &WorldDir, root: &Nbt) -> io::Result<()> {
    fs::create_dir_all(&dir.root)?;
    let data_file = dir.root.join(format!("level{}.dat", rand::random::<u64>()));
    let mut gz = GzEncoder::new(File::create(&data_file)?, Compression::default());
    gz.write_all(&encode_nbt(root))?;
    gz.finish()?;
    let (current, old) = (dir.level_dat(), dir.old_level_dat());
    if current.exists() {
        let _ = fs::remove_file(&old);
        fs::rename(&current, &old)?;
    }
    if let Err(e) = fs::rename(&data_file, &current) {
        let _ = fs::rename(&old, &current);
        return Err(e);
    }
    Ok(())
}

pub const ALLOWED_SYMLINKS_CONFIG_NAME: &str = "allowed_symlinks.txt";

pub fn parse_validator(config_path: &Path) -> DirectoryValidator {
    if config_path.exists() {
        let list = fs::read(config_path)
            .map_err(|e| crate::server::dedicated::settings::java_io_exception(&e, config_path))
            .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "java.io.UncheckedIOException: java.nio.charset.MalformedInputException: Input length = 1".to_string()))
            .and_then(|text| PathAllowList::read_plain(&text));
        match list {
            Ok(list) => return DirectoryValidator::new(SymlinkTargetAllowList::PathAllowList(list)),
            Err(e) => error!("Failed to parse {ALLOWED_SYMLINKS_CONFIG_NAME}, disallowing all symbolic links\n{e}"),
        }
    }
    DirectoryValidator::new(SymlinkTargetAllowList::Nothing)
}

pub struct WorldDir {
    pub root: PathBuf,
}

impl WorldDir {
    pub fn level_dat(&self) -> PathBuf {
        self.root.join("level.dat")
    }
    pub fn old_level_dat(&self) -> PathBuf {
        self.root.join("level.dat_old")
    }
    pub fn icon_file(&self) -> PathBuf {
        self.root.join("icon.png")
    }
    pub fn data(&self, name: &str) -> PathBuf {
        self.root.join("data/minecraft").join(format!("{name}.dat"))
    }
    pub fn data_folder(&self) -> PathBuf {
        self.root.join("data")
    }
    pub fn regions(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("region")
    }
    pub fn dimension_data_folder(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("data")
    }
    pub fn entities(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("entities")
    }
    pub fn poi(&self, dimension: &str) -> PathBuf {
        let (ns, path) = dimension.split_once(':').unwrap();
        self.root.join("dimensions").join(ns).join(path).join("poi")
    }
    pub fn player(&self, uuid: u128) -> PathBuf {
        self.root.join("players/data").join(format!("{}.dat", format_uuid(uuid)))
    }
}

pub fn format_uuid(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn uuid_ints(uuid: u128) -> Nbt {
    Nbt::IntArray((0..4).map(|i| (uuid >> (96 - 32 * i)) as u32 as i32).collect())
}

fn compound(fields: Vec<(&str, Nbt)>) -> Nbt {
    Nbt::Compound(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn string(s: &str) -> Nbt {
    Nbt::String(s.to_owned())
}

fn data_file(data: Nbt) -> Nbt {
    compound(vec![("data", data), ("DataVersion", Nbt::Int(WORLD_VERSION))])
}

#[derive(Clone, Debug)]
pub struct RespawnData {
    pub dimension: String,
    pub pos: BlockPos,
    pub yaw: f32,
    pub pitch: f32,
}

impl PartialEq for RespawnData {
    fn eq(&self, other: &RespawnData) -> bool {
        self.dimension == other.dimension && self.pos == other.pos && self.yaw.to_bits() == other.yaw.to_bits() && self.pitch.to_bits() == other.pitch.to_bits()
    }
}

impl RespawnData {
    pub fn of(dimension: &str, pos: BlockPos, yaw: f32, pitch: f32) -> RespawnData {
        RespawnData { dimension: dimension.to_string(), pos, yaw: crisp_core::mth::wrap_degrees(yaw), pitch: crisp_core::mth::clamp_f(pitch, -90.0, 90.0) }
    }

    pub fn to_nbt_fields(&self) -> Vec<(String, Nbt)> {
        vec![
            ("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z])),
            ("pitch".into(), Nbt::Float(self.pitch)),
            ("dimension".into(), string(&self.dimension)),
            ("yaw".into(), Nbt::Float(self.yaw)),
        ]
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<RespawnData> {
        let Some(Nbt::IntArray(p)) = nbt.get("pos") else { return None };
        let [x, y, z] = p[..] else { return None };
        let dimension = nbt.get("dimension").and_then(Nbt::as_str)?.to_string();
        let angle = |key: &str| nbt.get(key).and_then(Nbt::as_f64).map(|v| v as f32);
        Some(RespawnData { dimension, pos: BlockPos::new(x, y, z), yaw: angle("yaw")?, pitch: angle("pitch")? })
    }
}

pub struct WorldData {
    pub seed: i64,
    pub game_time: i64,
    pub spawn: RespawnData,
    pub game_type: GameType,
    pub difficulty: Difficulty,
    pub difficulty_locked: bool,
    pub hardcore: bool,
    pub clocks: Nbt,
    pub weather: WeatherData,
    pub generator: ChunkGenerator,
    pub generate_structures: bool,
    pub initialized: bool,
    pub server_brands: Vec<String>,
    pub was_modded: bool,
    pub game_rules: Nbt,
    pub data_configuration: WorldDataConfiguration,
}

impl WorldData {
    pub fn new_world(
        seed: i64,
        generator: ChunkGenerator,
        generate_structures: bool,
        game_type: GameType,
        difficulty: Difficulty,
        hardcore: bool,
        data_configuration: WorldDataConfiguration,
    ) -> WorldData {
        WorldData {
            seed,
            game_time: 0,
            spawn: RespawnData::of("minecraft:overworld", BlockPos::ZERO, 0.0, 0.0),
            game_type,
            difficulty,
            difficulty_locked: false,
            hardcore,
            clocks: Nbt::Compound(Vec::new()),
            weather: WeatherData::default(),
            generator,
            generate_structures,
            initialized: false,
            server_brands: Vec::new(),
            was_modded: false,
            game_rules: Nbt::Compound(Vec::new()),
            data_configuration,
        }
    }

    pub fn load(dir: &WorldDir) -> io::Result<Option<WorldData>> {
        let Some(level) = read_dat(&dir.level_dat())? else { return Ok(None) };
        let level = level.get("Data").ok_or_else(|| invalid("level.dat has no Data"))?;
        let data_version = level.get("DataVersion").and_then(Nbt::as_i64).unwrap_or(0);
        if data_version != WORLD_VERSION as i64 {
            return Err(invalid(&format!(
                "the world was saved by data version {data_version}; Crisp only reads {WORLD_VERSION} ({VERSION_NAME}) worlds"
            )));
        }
        let spawn = level.get("spawn").and_then(RespawnData::from_nbt).unwrap_or_else(|| RespawnData::of("minecraft:overworld", BlockPos::ZERO, 0.0, 0.0));
        let difficulty_settings = level.get("difficulty_settings");
        let setting = |key: &str| difficulty_settings.and_then(|d| d.get(key));
        let flag = |key: &str| setting(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        let settings_file = read_dat(&dir.data("world_gen_settings"))?.ok_or_else(|| invalid("world_gen_settings.dat is missing"))?;
        let worldgen = settings_file.get("data").ok_or_else(|| invalid("world_gen_settings.dat has no data"))?;
        let seed = worldgen.get("seed").and_then(Nbt::as_i64).unwrap_or(0);
        let overworld = worldgen.get("dimensions").and_then(|d| d.get("minecraft:overworld")).and_then(|d| d.get("generator"));
        let generator = match overworld.and_then(|g| g.get("type")).and_then(Nbt::as_str) {
            Some("minecraft:flat") => {
                let settings = overworld.and_then(|g| g.get("settings")).ok_or_else(|| invalid("flat settings missing"))?;
                let mut flat_layers = Vec::new();
                for layer in settings.get("layers").map(Nbt::as_list).unwrap_or(&[]) {
                    let name = layer.get("block").and_then(Nbt::as_str).unwrap_or("minecraft:air");
                    let state = Block::by_name(name).map(Block::default_state).ok_or_else(|| invalid(&format!("unknown block {name}")))?;
                    flat_layers.push((state, layer.get("height").and_then(Nbt::as_i64).unwrap_or(1) as u32));
                }
                ChunkGenerator::Flat(FlatLevelSource {
                    layers: flat_layers,
                    biome: settings.get("biome").and_then(Nbt::as_str).and_then(Biome::by_name).unwrap_or(FlatLevelSource::default().biome),
                    structure_overrides: settings.get("structure_overrides").and_then(StructureOverrides::from_nbt),
                    lakes: settings.get("lakes").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
                    features: settings.get("features").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
                    generation: Default::default(),
                })
            }
            Some("minecraft:noise") => {
                let generator = overworld.unwrap();
                let settings = generator.get("settings").and_then(Nbt::as_str);
                let source = generator.get("biome_source");
                let preset = source.and_then(|s| s.get("preset")).and_then(Nbt::as_str);
                let source_type = source.and_then(|s| s.get("type")).and_then(Nbt::as_str);
                if settings != Some("minecraft:overworld") || source_type != Some("minecraft:multi_noise") || preset != Some("minecraft:overworld") {
                    return Err(invalid("only the default overworld noise generator is supported"));
                }
                ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::overworld(seed)))
            }
            _ => return Err(invalid("only superflat and normal overworlds can be loaded")),
        };
        let clocks = read_dat(&dir.data("world_clocks"))?.and_then(|c| c.get("data").cloned()).unwrap_or(Nbt::Compound(Vec::new()));
        let weather = read_dat(&dir.data("weather"))?.and_then(|w| w.get("data").and_then(WeatherData::from_nbt)).unwrap_or_default();
        Ok(Some(WorldData {
            seed,
            game_time: level.get("Time").and_then(Nbt::as_i64).unwrap_or(0),
            spawn,
            game_type: GameType::by_id(level.get("GameType").and_then(Nbt::as_i64).unwrap_or(0) as i32),
            difficulty: setting("difficulty").and_then(Nbt::as_str).and_then(Difficulty::by_name).unwrap_or(Difficulty::Normal),
            difficulty_locked: flag("locked"),
            hardcore: flag("hardcore"),
            clocks,
            weather,
            generator,
            generate_structures: worldgen.get("generate_structures").and_then(Nbt::as_i64).is_none_or(|v| v != 0),
            initialized: level.get("initialized").and_then(Nbt::as_i64).is_none_or(|v| v != 0),
            server_brands: level.get("ServerBrands").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(Nbt::as_str).fold(Vec::new(), |mut brands, brand| {
                if !brands.iter().any(|b| b == brand) {
                    brands.push(brand.to_string());
                }
                brands
            }),
            was_modded: level.get("WasModded").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
            game_rules: read_dat(&dir.data("game_rules"))?.and_then(|r| r.get("data").cloned()).unwrap_or(Nbt::Compound(Vec::new())),
            data_configuration: WorldDataConfiguration::parse(level),
        }))
    }

    pub fn save(&self, dir: &WorldDir) -> io::Result<()> {
        self.save_level_data(dir)?;
        self.save_saved_data(dir)
    }

    pub fn save_level_data(&self, dir: &WorldDir) -> io::Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        let mut data = vec![
            (
                "difficulty_settings",
                compound(vec![
                    ("difficulty", string(self.difficulty.key())),
                    ("hardcore", Nbt::Byte(self.hardcore as i8)),
                    ("locked", Nbt::Byte(self.difficulty_locked as i8)),
                ]),
            ),
            ("Time", Nbt::Long(self.game_time)),
            ("version_history", Nbt::List(vec![Nbt::Int(WORLD_VERSION)])),
            ("GameType", Nbt::Int(self.game_type.id())),
            ("ServerBrands", Nbt::List(self.server_brands.iter().map(|b| string(b)).collect())),
            ("version", Nbt::Int(LEVEL_NBT_VERSION)),
            ("LastPlayed", Nbt::Long(now)),
            ("spawn", Nbt::Compound(self.spawn.to_nbt_fields())),
            (
                "Version",
                compound(vec![
                    ("Snapshot", Nbt::Byte(0)),
                    ("Series", string("main")),
                    ("Id", Nbt::Int(WORLD_VERSION)),
                    ("Name", string(VERSION_NAME)),
                ]),
            ),
            ("LevelName", string(&dir.root.file_name().unwrap_or_default().to_string_lossy())),
            ("initialized", Nbt::Byte(self.initialized as i8)),
            ("WasModded", Nbt::Byte(self.was_modded as i8)),
            ("DataVersion", Nbt::Int(WORLD_VERSION)),
            ("allowCommands", Nbt::Byte(0)),
            ("DataPacks", self.data_configuration.data_packs.to_nbt()),
        ];
        if let Some(enabled_features) = self.data_configuration.encoded_enabled_features() {
            let after_version = data.iter().position(|(key, _)| *key == "version").unwrap() + 1;
            data.insert(after_version, ("enabled_features", enabled_features));
        }
        let level = compound(vec![("Data", compound(data))]);
        save_level_dat(dir, &level)
    }

    pub fn save_saved_data(&self, dir: &WorldDir) -> io::Result<()> {
        let noise_generator = |settings: &str, biome_source: Nbt| {
            compound(vec![("settings", string(settings)), ("biome_source", biome_source), ("type", string("minecraft:noise"))])
        };
        let noise = |name: &str, biome_source: Nbt| {
            compound(vec![
                ("generator", noise_generator(&format!("minecraft:{name}"), biome_source)),
                ("type", string(&format!("minecraft:the_{name}"))),
            ])
        };
        let (overworld_generator, generate_structures) = match &self.generator {
            ChunkGenerator::Flat(flat) => {
                let layers = flat
                    .layers
                    .iter()
                    .map(|&(state, height): &(BlockState, u32)| compound(vec![("block", string(state.block().name())), ("height", Nbt::Int(height as i32))]))
                    .collect();
                let mut settings = vec![("features", Nbt::Byte(flat.features as i8)), ("biome", string(flat.biome.name())), ("layers", Nbt::List(layers))];
                if let Some(overrides) = &flat.structure_overrides {
                    settings.push(("structure_overrides", overrides.to_nbt()));
                }
                settings.push(("lakes", Nbt::Byte(flat.lakes as i8)));
                (compound(vec![("settings", compound(settings)), ("type", string("minecraft:flat"))]), self.generate_structures as i8)
            }
            ChunkGenerator::Noise(_) => (
                noise_generator(
                    "minecraft:overworld",
                    compound(vec![("preset", string("minecraft:overworld")), ("type", string("minecraft:multi_noise"))]),
                ),
                self.generate_structures as i8,
            ),
        };
        let generation = compound(vec![
            ("bonus_chest", Nbt::Byte(0)),
            ("seed", Nbt::Long(self.seed)),
            ("generate_structures", Nbt::Byte(generate_structures)),
            (
                "dimensions",
                compound(vec![
                    ("minecraft:overworld", compound(vec![("generator", overworld_generator), ("type", string("minecraft:overworld"))])),
                    (
                        "minecraft:the_nether",
                        noise("nether", compound(vec![("preset", string("minecraft:nether")), ("type", string("minecraft:multi_noise"))])),
                    ),
                    ("minecraft:the_end", noise("end", compound(vec![("type", string("minecraft:the_end"))]))),
                ]),
            ),
        ]);
        write_dat(&dir.data("world_gen_settings"), &data_file(generation))?;

        write_dat(&dir.data("world_clocks"), &data_file(self.clocks.clone()))?;
        write_dat(&dir.data("weather"), &data_file(self.weather.to_nbt()))?;
        write_dat(&dir.data("game_rules"), &data_file(self.game_rules.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_format() {
        assert_eq!(format_uuid(0xd76279edcc573f1885f4367c1defa1ee), "d76279ed-cc57-3f18-85f4-367c1defa1ee");
    }
}
