use std::sync::LazyLock;

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::{AXIS, FACING, HORIZONTAL_FACING};
use crisp_data::{Block, BlockState};
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::random::WeightedList;
use crate::util::valueproviders::{IntProvider, float};
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockPredicate, resource};
use crate::world::level::levelgen::synth::NoiseStack;
use crate::world::level::levelgen::{AlgorithmRandom, parse_block_state, parse_noise};

pub enum BlockStateProvider {
    Simple(BlockState),
    Weighted(WeightedList<BlockState>),
    Noise(NoiseProvider),
    DualNoise { variety: (i32, i32), slow_noise: NoiseStack, slow_scale: f32, provider: NoiseProvider },
    NoiseThreshold { noise: NoiseStack, scale: f32, threshold: f32, high_chance: f32, default_state: BlockState, low_states: Vec<BlockState>, high_states: Vec<BlockState> },
    RandomBlock(&'static [i32]),
    RandomizedInt { source: BlockStateProviderHolder, property: String, values: IntProvider },
    Rotated { state: BlockStateProviderHolder, direction: Option<Direction> },
    RuleBased { fallback: Option<BlockStateProviderHolder>, rules: Vec<(BlockPredicate, BlockStateProviderHolder)> },
    CopyProperties(BlockStateProviderHolder),
}

pub struct NoiseProvider {
    noise: NoiseStack,
    scale: f32,
    states: Vec<BlockState>,
}

impl NoiseProvider {
    fn parse(json: &Value) -> Result<NoiseProvider, String> {
        let (noise, scale) = parse_noise_based(json)?;
        Ok(NoiseProvider { noise, scale, states: parse_states(json.get("states").ok_or("missing states")?)? })
    }

    fn get_noise_value(&self, pos: BlockPos, scale: f64) -> f32 {
        get_noise_value(&self.noise, pos, scale)
    }

    fn get_random_state_at(&self, states: &[BlockState], pos: BlockPos, scale: f64) -> BlockState {
        let noise_value = self.get_noise_value(pos, scale);
        get_random_state(states, noise_value)
    }
}

fn get_noise_value(noise: &NoiseStack, pos: BlockPos, scale: f64) -> f32 {
    noise.get(pos.x as f64 * scale, pos.y as f64 * scale, pos.z as f64 * scale)
}

fn get_random_state(states: &[BlockState], noise_value: f32) -> BlockState {
    let placement_value = ((1.0 + noise_value) / 2.0).clamp(0.0, 0.9999);
    states[(placement_value * states.len() as f32) as usize]
}

fn parse_noise_based(json: &Value) -> Result<(NoiseStack, f32), String> {
    let seed = json.get("seed").and_then(Value::as_i64).ok_or("missing seed")?;
    let parameters = parse_noise(json.get("noise").ok_or("missing noise")?)?;
    let noise = parameters.create(&mut AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)));
    Ok((noise, float(json.get("scale").ok_or("missing scale")?)?))
}

fn parse_states(json: &Value) -> Result<Vec<BlockState>, String> {
    json.as_array().ok_or("expected a list of states")?.iter().map(parse_block_state).collect()
}

static BLOCK_STATE_PROVIDERS: LazyLock<Vec<BlockStateProvider>> = LazyLock::new(|| {
    crisp_data::BLOCK_STATE_PROVIDERS
        .iter()
        .map(|(name, json)| {
            let json: Value = serde_json::from_str(json).expect("block state provider json");
            BlockStateProvider::parse(&json).unwrap_or_else(|e| panic!("block state provider {name}: {e}"))
        })
        .collect()
});

pub enum BlockStateProviderHolder {
    Reference(u32),
    Direct(Box<BlockStateProvider>),
}

impl BlockStateProviderHolder {
    pub fn parse(json: &Value) -> Result<BlockStateProviderHolder, String> {
        match json {
            Value::String(name) => {
                let name = resource(name);
                crisp_data::BLOCK_STATE_PROVIDERS
                    .iter()
                    .position(|(n, _)| *n == name)
                    .map(|i| BlockStateProviderHolder::Reference(i as u32))
                    .ok_or_else(|| format!("unknown block state provider {name}"))
            }
            other => Ok(BlockStateProviderHolder::Direct(Box::new(BlockStateProvider::parse(other)?))),
        }
    }

    pub fn value(&self) -> &BlockStateProvider {
        match self {
            BlockStateProviderHolder::Reference(index) => &BLOCK_STATE_PROVIDERS[*index as usize],
            BlockStateProviderHolder::Direct(provider) => provider,
        }
    }
}

impl BlockStateProvider {
    pub fn parse(json: &Value) -> Result<BlockStateProvider, String> {
        let Some(ty) = json.get("type").and_then(Value::as_str) else {
            return Ok(BlockStateProvider::Simple(parse_block_state(json)?));
        };
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "simple" => BlockStateProvider::Simple(parse_block_state(field("state")?)?),
            "weighted" => BlockStateProvider::Weighted(WeightedList::parse(field("entries")?, parse_block_state)?),
            "noise" => BlockStateProvider::Noise(NoiseProvider::parse(json)?),
            "dual_noise" => {
                let variety = field("variety")?;
                let variety = match variety {
                    Value::Array(v) if v.len() == 2 => (v[0].as_i64().unwrap_or(1) as i32, v[1].as_i64().unwrap_or(1) as i32),
                    Value::Number(n) => (n.as_i64().unwrap_or(1) as i32, n.as_i64().unwrap_or(1) as i32),
                    other => (
                        other.get("min_inclusive").and_then(Value::as_i64).ok_or("bad variety")? as i32,
                        other.get("max_inclusive").and_then(Value::as_i64).ok_or("bad variety")? as i32,
                    ),
                };
                let seed = field("seed")?.as_i64().ok_or("bad seed")?;
                let slow_noise = parse_noise(field("slow_noise")?)?.create(&mut AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)));
                BlockStateProvider::DualNoise { variety, slow_noise, slow_scale: float(field("slow_scale")?)?, provider: NoiseProvider::parse(json)? }
            }
            "noise_threshold" => {
                let (noise, scale) = parse_noise_based(json)?;
                BlockStateProvider::NoiseThreshold {
                    noise,
                    scale,
                    threshold: float(field("threshold")?)?,
                    high_chance: float(field("high_chance")?)?,
                    default_state: parse_block_state(field("default_state")?)?,
                    low_states: parse_states(field("low_states")?)?,
                    high_states: parse_states(field("high_states")?)?,
                }
            }
            "random_block" => {
                let blocks = field("blocks")?.as_str().and_then(|s| s.strip_prefix('#')).ok_or("random_block needs a block tag")?;
                BlockStateProvider::RandomBlock(crisp_data::tag("minecraft:block", &resource(blocks)))
            }
            "randomized_int" => BlockStateProvider::RandomizedInt {
                source: BlockStateProviderHolder::parse(field("source")?)?,
                property: field("property")?.as_str().ok_or("bad property")?.to_owned(),
                values: IntProvider::parse(field("values")?)?,
            },
            "rotated" => BlockStateProvider::Rotated {
                state: BlockStateProviderHolder::parse(field("state")?)?,
                direction: json.get("direction").and_then(Value::as_str).map(|d| Direction::by_name(d).ok_or_else(|| format!("unknown direction {d}"))).transpose()?,
            },
            "rule_based" => BlockStateProvider::RuleBased {
                fallback: json.get("fallback").map(BlockStateProviderHolder::parse).transpose()?,
                rules: field("rules")?
                    .as_array()
                    .ok_or("rules is a list")?
                    .iter()
                    .map(|rule| {
                        Ok((
                            BlockPredicate::parse(rule.get("if_true").ok_or("missing if_true")?)?,
                            BlockStateProviderHolder::parse(rule.get("then").ok_or("missing then")?)?,
                        ))
                    })
                    .collect::<Result<_, String>>()?,
            },
            "copy_properties" => BlockStateProvider::CopyProperties(BlockStateProviderHolder::parse(field("source")?)?),
            other => return Err(format!("unknown block state provider type {other}")),
        })
    }

    pub fn get_state(&self, level: &mut WorldGenRegion, random: &mut impl RandomSource, pos: BlockPos) -> BlockState {
        match self {
            BlockStateProvider::Simple(state) => *state,
            BlockStateProvider::Weighted(list) => *list.get_random_or_throw(random),
            BlockStateProvider::Noise(provider) => provider.get_random_state_at(&provider.states, pos, provider.scale as f64),
            BlockStateProvider::DualNoise { variety, slow_noise, slow_scale, provider } => {
                let slow_noise_value = |pos: BlockPos| slow_noise.get(pos.x as f64 * *slow_scale as f64, pos.y as f64 * *slow_scale as f64, pos.z as f64 * *slow_scale as f64);
                let variety_noise = slow_noise_value(pos) as f64;
                let local_variety = mth::clamped_map(variety_noise, -1.0, 1.0, variety.0 as f64, (variety.1 + 1) as f64) as i32;
                let mut possible_states = Vec::with_capacity(local_variety.max(0) as usize);
                for i in 0..local_variety {
                    possible_states.push(get_random_state(&provider.states, slow_noise_value(pos.offset(i * 54545, 0, i * 34234))));
                }
                provider.get_random_state_at(&possible_states, pos, provider.scale as f64)
            }
            BlockStateProvider::NoiseThreshold { noise, scale, threshold, high_chance, default_state, low_states, high_states } => {
                let local_value = get_noise_value(noise, pos, *scale as f64) as f64;
                if local_value < *threshold as f64 {
                    low_states[random.next_int_bounded(low_states.len() as i32) as usize]
                } else if random.next_float() < *high_chance {
                    high_states[random.next_int_bounded(high_states.len() as i32) as usize]
                } else {
                    *default_state
                }
            }
            BlockStateProvider::RandomBlock(_) | BlockStateProvider::RuleBased { .. } => match self.get_optional_state(level, random, pos) {
                Some(state) => state,
                None => level.get_block_state(pos),
            },
            BlockStateProvider::RandomizedInt { source, property, values } => {
                let unmodified_state = source.value().get_state(level, random, pos);
                let is_integer = unmodified_state
                    .block()
                    .property_by_name(property)
                    .is_some_and(|(_, possible)| possible.iter().all(|v| v.parse::<i32>().is_ok()));
                if !is_integer {
                    return unmodified_state;
                }
                let value = values.sample(random).to_string();
                unmodified_state.with_property_by_name(property, &value).unwrap_or_else(|| panic!("Cannot set property {property} to {value} on {:?}", unmodified_state.block()))
            }
            BlockStateProvider::Rotated { state, direction } => {
                let direction = direction.unwrap_or_else(|| Direction::VALUES[random.next_int_bounded(Direction::VALUES.len() as i32) as usize]);
                let new_state = state.value().get_state(level, random, pos).try_set(AXIS, direction.axis()).try_set(FACING, direction);
                if direction.axis().is_horizontal() { new_state.try_set(HORIZONTAL_FACING, direction) } else { new_state }
            }
            BlockStateProvider::CopyProperties(source) => {
                let state = source.value().get_state(level, random, pos);
                state.with_properties_of(level.get_block_state(pos))
            }
        }
    }

    pub fn get_optional_state(&self, level: &mut WorldGenRegion, random: &mut impl RandomSource, pos: BlockPos) -> Option<BlockState> {
        match self {
            BlockStateProvider::RandomBlock(blocks) => {
                if blocks.is_empty() {
                    return None;
                }
                let block = Block(blocks[random.next_int_bounded(blocks.len() as i32) as usize] as u16);
                Some(block.default_state())
            }
            BlockStateProvider::RuleBased { fallback, rules } => {
                for (if_true, then) in rules {
                    if if_true.test(level, pos)
                        && let Some(state) = then.value().get_optional_state(level, random, pos)
                    {
                        return Some(state);
                    }
                }
                fallback.as_ref().and_then(|fallback| fallback.value().get_optional_state(level, random, pos))
            }
            _ => Some(self.get_state(level, random, pos)),
        }
    }
}
