use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crisp_core::random::LegacyRandomSource;

use super::density_function::{CompileContext, Df, DensityFunctionCompiler, NoiseHolder, Sampler};
use super::material::MaterialSystem;
use super::synth::NoiseStack;
use super::{AlgorithmRandom, NoiseGeneratorSettings, PositionalRandomFactory, WORLDGEN};

pub struct RandomState {
    pub seed: i64,
    legacy: bool,
    pub random: PositionalRandomFactory,
    noises: RwLock<HashMap<String, Arc<NoiseStack>>>,
    positional_randoms: RwLock<HashMap<String, PositionalRandomFactory>>,
    compiler: Mutex<DensityFunctionCompiler>,
    by_address: RwLock<HashMap<usize, (Df, Arc<Sampler>)>>,
    material_system: Option<MaterialSystem>,
}

impl RandomState {
    pub fn dummy(seed: i64) -> RandomState {
        RandomState {
            seed,
            legacy: false,
            random: AlgorithmRandom::new(false, seed).fork_positional(),
            noises: RwLock::new(HashMap::new()),
            positional_randoms: RwLock::new(HashMap::new()),
            compiler: Mutex::new(DensityFunctionCompiler::default()),
            by_address: RwLock::new(HashMap::new()),
            material_system: None,
        }
    }

    pub fn new(settings: &NoiseGeneratorSettings, seed: i64) -> RandomState {
        let random = AlgorithmRandom::new(settings.legacy_random_source, seed).fork_positional();
        let mut state = RandomState {
            seed,
            legacy: settings.legacy_random_source,
            random,
            noises: RwLock::new(HashMap::new()),
            positional_randoms: RwLock::new(HashMap::new()),
            compiler: Mutex::new(DensityFunctionCompiler::default()),
            by_address: RwLock::new(HashMap::new()),
            material_system: None,
        };
        let system = MaterialSystem::new(&state, settings.default_block, settings.sea_level, settings.noise_router.chunk_surface_level.clone(), random);
        state.material_system = Some(system);
        state
    }

    pub fn material_system(&self) -> &MaterialSystem {
        self.material_system.as_ref().unwrap()
    }

    pub fn noise(&self, name: &str) -> Arc<NoiseStack> {
        if let Some(noise) = self.noises.read().unwrap().get(name) {
            return noise.clone();
        }
        let definition = WORLDGEN.noises.get(name).unwrap_or_else(|| panic!("unknown noise {name}"));
        let noise = Arc::new(definition.create(&mut self.random.from_hash_of(name)));
        self.noises.write().unwrap().entry(name.to_owned()).or_insert(noise).clone()
    }

    pub fn random_factory(&self, name: &str) -> PositionalRandomFactory {
        if let Some(factory) = self.positional_randoms.read().unwrap().get(name) {
            return *factory;
        }
        *self.positional_randoms.write().unwrap().entry(name.to_owned()).or_insert_with(|| self.random.from_hash_of(name).fork_positional())
    }

    pub fn sampler(&self, function: &Df) -> Arc<Sampler> {
        let address = Arc::as_ptr(function) as usize;
        if let Some((_, sampler)) = self.by_address.read().unwrap().get(&address) {
            return sampler.clone();
        }
        let sampler = self.compiler.lock().unwrap().get_sampler(function, self);
        self.by_address.write().unwrap().insert(address, (function.clone(), sampler.clone()));
        sampler
    }
}

impl CompileContext for RandomState {
    fn noise(&self, holder: &NoiseHolder) -> Arc<NoiseStack> {
        match &*holder.name {
            "minecraft:nether/temperature" => Arc::new(holder.noise.create_for_legacy_nether_biome(&mut LegacyRandomSource::new(self.seed))),
            "minecraft:nether/vegetation" => {
                Arc::new(holder.noise.create_for_legacy_nether_biome(&mut LegacyRandomSource::new(self.seed.wrapping_add(1))))
            }
            name => RandomState::noise(self, name),
        }
    }

    fn random(&self, name: &str) -> AlgorithmRandom {
        if self.legacy && name == "minecraft:terrain" {
            AlgorithmRandom::Legacy(LegacyRandomSource::new(self.seed))
        } else {
            self.random.from_hash_of(name)
        }
    }

    fn end_island_random(&self) -> AlgorithmRandom {
        AlgorithmRandom::Legacy(LegacyRandomSource::new(self.seed))
    }
}
