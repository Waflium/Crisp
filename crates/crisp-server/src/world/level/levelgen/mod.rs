pub mod aquifer;
pub mod blockpredicates;
pub mod carver;
pub mod chunk_generator;
pub mod density_function;
pub mod feature;
pub mod flat;
pub mod heightproviders;
pub mod material;
pub mod placement;
pub mod random_state;
pub mod structure;
pub mod synth;
pub mod worldgen_random;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use crisp_core::random::{LegacyPositionalRandomFactory, LegacyRandomSource, XoroshiroPositionalRandomFactory, XoroshiroRandomSource};
use crisp_core::RandomSource;
use crisp_data::{Block, BlockState};
use serde_json::Value;

pub use chunk_generator::NoiseBasedChunkGenerator;
pub use random_state::RandomState;

use density_function::{Df, Parser, resource_name};
use material::MaterialRule;
use synth::{NormalNoise, NormalNoiseParameters, Normalization};

#[derive(Clone)]
pub enum AlgorithmRandom {
    Xoroshiro(XoroshiroRandomSource),
    Legacy(LegacyRandomSource),
}

macro_rules! delegate {
    ($self:ident, $r:ident => $e:expr) => {
        match $self {
            AlgorithmRandom::Xoroshiro($r) => $e,
            AlgorithmRandom::Legacy($r) => $e,
        }
    };
}

impl RandomSource for AlgorithmRandom {
    fn set_seed(&mut self, seed: i64) {
        delegate!(self, r => r.set_seed(seed))
    }
    fn next_int(&mut self) -> i32 {
        delegate!(self, r => r.next_int())
    }
    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        delegate!(self, r => r.next_int_bounded(bound))
    }
    fn next_long(&mut self) -> i64 {
        delegate!(self, r => r.next_long())
    }
    fn next_boolean(&mut self) -> bool {
        delegate!(self, r => r.next_boolean())
    }
    fn next_float(&mut self) -> f32 {
        delegate!(self, r => r.next_float())
    }
    fn next_double(&mut self) -> f64 {
        delegate!(self, r => r.next_double())
    }
    fn next_gaussian(&mut self) -> f64 {
        delegate!(self, r => r.next_gaussian())
    }
    fn consume_count(&mut self, rounds: i32) {
        delegate!(self, r => r.consume_count(rounds))
    }
}

impl AlgorithmRandom {
    pub fn new(legacy: bool, seed: i64) -> AlgorithmRandom {
        if legacy { AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)) } else { AlgorithmRandom::Xoroshiro(XoroshiroRandomSource::new(seed)) }
    }

    pub fn fork_positional(&mut self) -> PositionalRandomFactory {
        match self {
            AlgorithmRandom::Xoroshiro(r) => PositionalRandomFactory::Xoroshiro(r.fork_positional()),
            AlgorithmRandom::Legacy(r) => PositionalRandomFactory::Legacy(r.fork_positional()),
        }
    }
}

#[derive(Clone, Copy)]
pub enum PositionalRandomFactory {
    Xoroshiro(XoroshiroPositionalRandomFactory),
    Legacy(LegacyPositionalRandomFactory),
}

impl PositionalRandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> AlgorithmRandom {
        match self {
            PositionalRandomFactory::Xoroshiro(f) => AlgorithmRandom::Xoroshiro(f.at(x, y, z)),
            PositionalRandomFactory::Legacy(f) => AlgorithmRandom::Legacy(f.at(x, y, z)),
        }
    }

    pub fn from_hash_of(&self, name: &str) -> AlgorithmRandom {
        match self {
            PositionalRandomFactory::Xoroshiro(f) => AlgorithmRandom::Xoroshiro(f.from_hash_of(name)),
            PositionalRandomFactory::Legacy(f) => AlgorithmRandom::Legacy(f.from_hash_of(name)),
        }
    }
}

pub mod quart_pos {
    pub const fn from_block(block: i32) -> i32 {
        block >> 2
    }
    pub const fn to_block(quart: i32) -> i32 {
        quart << 2
    }
}

pub fn parse_block_state(v: &Value) -> Result<BlockState, String> {
    let (name, properties) = match v {
        Value::String(name) => (name.as_str(), None),
        _ => (v.get("id").and_then(Value::as_str).ok_or("block state without id")?, v.get("properties").and_then(Value::as_object)),
    };
    let block = Block::by_name(&resource_name(name)).ok_or_else(|| format!("unknown block {name}"))?;
    let mut state = block.default_state();
    for (key, value) in properties.into_iter().flatten() {
        let value = value.as_str().ok_or("property values are strings")?;
        state = state.with_property_by_name(key, value).ok_or_else(|| format!("{name} has no {key}={value}"))?;
    }
    Ok(state)
}

#[derive(Clone, Copy, Debug)]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: i32,
}

impl NoiseSettings {
    pub fn clamp_to(self, min_y: i32, height: i32) -> NoiseSettings {
        let new_min = self.min_y.max(min_y);
        let new_height = (self.min_y + self.height).min(min_y + height) - new_min;
        NoiseSettings { min_y: new_min, height: new_height }
    }
}

#[derive(Clone)]
pub struct NoiseRouter {
    pub temperature: Df,
    pub vegetation: Df,
    pub continents: Df,
    pub erosion: Df,
    pub depth: Df,
    pub ridges: Df,
    pub chunk_surface_level: Df,
    pub final_density: Df,
}

#[derive(Clone)]
pub struct AquiferConfig {
    pub barrier: Df,
    pub fluid_level_floodedness: Df,
    pub fluid_level_spread: Df,
    pub lava: Df,
    pub exclusion: Df,
    pub surface_level: Df,
}

#[derive(Clone)]
pub struct SpawnTargetPoint {
    pub parameters: Vec<(Df, crate::world::level::biome::climate::Parameter)>,
}

pub struct NoiseGeneratorSettings {
    pub name: String,
    pub noise: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: NoiseRouter,
    pub material_rule: Arc<MaterialRule>,
    pub spawn_target: Vec<SpawnTargetPoint>,
    pub sea_level: i32,
    pub aquifers: Option<AquiferConfig>,
    pub legacy_random_source: bool,
}

pub struct WorldgenRegistries {
    pub noises: HashMap<String, Arc<NormalNoise>>,
    pub noise_settings: HashMap<String, Arc<NoiseGeneratorSettings>>,
}

pub static WORLDGEN: LazyLock<WorldgenRegistries> = LazyLock::new(|| WorldgenRegistries::load().expect("vanilla worldgen data"));

pub fn parse_noise(json: &Value) -> Result<NormalNoise, String> {
    let normalize = match json.get("normalize") {
        None | Some(Value::Bool(true)) => Normalization::Enabled,
        Some(Value::Bool(false)) => Normalization::Disabled,
        Some(Value::String(s)) if s == "legacy" => Normalization::Legacy,
        Some(other) => return Err(format!("bad normalize {other}")),
    };
    let modifiers = match json.get("amplitude_modifiers") {
        Some(Value::Array(a)) => a.iter().map(|v| v.as_f64().ok_or("bad amplitude")).collect::<Result<_, _>>()?,
        _ => Vec::new(),
    };
    Ok(NormalNoise::new(NormalNoiseParameters {
        base_amplitude: json.get("base_amplitude").and_then(Value::as_f64).unwrap_or(1.0),
        base_octave: json.get("base_octave").and_then(Value::as_i64).ok_or("missing base_octave")? as i32,
        octave_count: json.get("octave_count").and_then(Value::as_i64).unwrap_or(1) as i32,
        normalize,
        amplitude_modifiers: modifiers,
    }))
}

impl WorldgenRegistries {
    fn load() -> Result<WorldgenRegistries, String> {
        let mut noises = HashMap::new();
        for (name, source) in crisp_data::NOISES {
            let json: Value = serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))?;
            noises.insert(name.to_string(), Arc::new(parse_noise(&json).map_err(|e| format!("{name}: {e}"))?));
        }
        let sources: HashMap<&'static str, &'static str> = crisp_data::DENSITY_FUNCTIONS.iter().copied().collect();
        let mut parser = Parser { noises: &noises, sources: &sources, parsed: HashMap::new() };
        let mut materials = material::MaterialParser::new();
        let mut noise_settings = HashMap::new();
        for (name, source) in crisp_data::NOISE_SETTINGS {
            let json: Value = serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))?;
            let settings = Self::parse_settings(name, &json, &mut parser, &mut materials).map_err(|e| format!("{name}: {e}"))?;
            noise_settings.insert(name.to_string(), Arc::new(settings));
        }
        Ok(WorldgenRegistries { noises, noise_settings })
    }

    fn parse_settings(
        name: &str,
        json: &Value,
        parser: &mut Parser,
        materials: &mut material::MaterialParser,
    ) -> Result<NoiseGeneratorSettings, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let noise = field("noise")?;
        let router = field("noise_router")?;
        let mut f = |key: &str| parser.parse(router.get(key).ok_or_else(|| format!("missing noise_router.{key}"))?);
        let noise_router = NoiseRouter {
            temperature: f("temperature")?,
            vegetation: f("vegetation")?,
            continents: f("continents")?,
            erosion: f("erosion")?,
            depth: f("depth")?,
            ridges: f("ridges")?,
            chunk_surface_level: f("chunk_surface_level")?,
            final_density: f("final_density")?,
        };
        let aquifers = match json.get("aquifers") {
            Some(a) => {
                let mut f = |key: &str| parser.parse(a.get(key).ok_or_else(|| format!("missing aquifers.{key}"))?);
                Some(AquiferConfig {
                    barrier: f("barrier")?,
                    fluid_level_floodedness: f("fluid_level_floodedness")?,
                    fluid_level_spread: f("fluid_level_spread")?,
                    lava: f("lava")?,
                    exclusion: f("exclusion")?,
                    surface_level: f("surface_level")?,
                })
            }
            None => None,
        };
        let mut spawn_target = Vec::new();
        for point in field("spawn_target")?.as_array().ok_or("spawn_target is a list")? {
            let mut parameters = Vec::new();
            for (function, range) in point.as_object().ok_or("spawn target is a map")? {
                let function = parser.parse(&Value::String(function.clone()))?;
                parameters.push((function, crate::world::level::biome::climate::Parameter::parse(range)?));
            }
            spawn_target.push(SpawnTargetPoint { parameters });
        }
        Ok(NoiseGeneratorSettings {
            name: name.to_owned(),
            noise: NoiseSettings {
                min_y: noise.get("min_y").and_then(Value::as_i64).ok_or("min_y")? as i32,
                height: noise.get("height").and_then(Value::as_i64).ok_or("height")? as i32,
            },
            default_block: parse_block_state(field("default_block")?)?,
            default_fluid: parse_block_state(field("default_fluid")?)?,
            noise_router,
            material_rule: materials.rule(field("material_rule")?, parser)?,
            spawn_target,
            sea_level: field("sea_level")?.as_i64().ok_or("sea_level")? as i32,
            aquifers,
            legacy_random_source: field("legacy_random_source")?.as_bool().ok_or("legacy_random_source")?,
        })
    }
}
