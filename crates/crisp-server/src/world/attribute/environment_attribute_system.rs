use std::cell::OnceCell;

use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::{DimensionType, SyncedRegistry};

use super::environment_attribute::EnvironmentAttributeInfo;
use super::environment_attribute_map::Entry;
use super::environment_attributes::ATTRIBUTES;
use super::{AttributeValue, EnvironmentAttribute, EnvironmentAttributeMap, LerpFunction, Value, weather_attributes};
use crate::world::clock::Timeline;
use crate::world::level::biome::BIOME_ATTRIBUTES;
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::density_function::resource_name;
use crate::world::timeline::AttributeTrackSampler;

pub trait EnvironmentAccess {
    fn biome(&self, x: i32, y: i32, z: i32) -> Biome;
    fn noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome;
    fn clock_total_ticks(&self, clock: usize) -> i64;
    fn rain_level(&self) -> f32;
    fn thunder_level(&self) -> f32;
}

fn dimension_timelines(dimension: &DimensionType) -> Vec<&'static Timeline> {
    let json: serde_json::Value = serde_json::from_str(dimension.timelines).expect("dimension timelines JSON");
    let registry = SyncedRegistry::get("minecraft:timeline");
    let ids: Vec<&str> = match &json {
        serde_json::Value::String(s) => match s.strip_prefix('#') {
            Some(tag) => crisp_data::tag("minecraft:timeline", &resource_name(tag)).iter().map(|&i| registry.entries[i as usize]).collect(),
            None => vec![s.as_str()],
        },
        serde_json::Value::Array(list) => list.iter().filter_map(serde_json::Value::as_str).collect(),
        _ => Vec::new(),
    };
    ids.into_iter().map(|id| Timeline::get(&resource_name(id)).unwrap_or_else(|| panic!("unknown timeline {id}"))).collect()
}

#[derive(Debug)]
enum Layer {
    Constant(Entry),
    Biome(Vec<Option<Entry>>),
    Track(AttributeTrackSampler),
    Weather { rain: Option<Entry>, thunder: Option<Entry>, state_change_lerp: LerpFunction },
}

#[derive(Default)]
pub struct Builder {
    layers_by_attribute: Vec<Vec<Layer>>,
}

impl Builder {
    fn add_layer(&mut self, id: usize, layer: Layer) {
        if self.layers_by_attribute.is_empty() {
            self.layers_by_attribute.resize_with(ATTRIBUTES.len(), Vec::new);
        }
        self.layers_by_attribute[id].push(layer);
    }

    pub fn add_default_layers(self, dimension: &DimensionType, can_have_weather: bool) -> Self {
        let attributes = EnvironmentAttributeMap::parse(&serde_json::from_str(dimension.attributes).expect("dimension attributes JSON"), false)
            .unwrap_or_else(|e| panic!("attributes of {}: {e}", dimension.name));
        let mut builder = self.add_constant_layer(&attributes).add_biome_layer(&BIOME_ATTRIBUTES);
        for timeline in dimension_timelines(dimension) {
            builder = builder.add_timeline_layer(timeline);
        }
        if can_have_weather { builder.add_weather_layers() } else { builder }
    }

    pub fn add_constant_layer(mut self, map: &EnvironmentAttributeMap) -> Self {
        for (id, entry) in map.entries() {
            self.add_layer(*id, Layer::Constant(entry.clone()));
        }
        self
    }

    pub fn add_biome_layer(mut self, biomes: &[EnvironmentAttributeMap]) -> Self {
        let mut provided: Vec<usize> = Vec::new();
        for biome in biomes {
            for id in biome.key_set() {
                if !provided.contains(&id) {
                    provided.push(id);
                }
            }
        }
        for id in provided {
            self.add_layer(id, Layer::Biome(biomes.iter().map(|b| b.get(id).cloned()).collect()));
        }
        self
    }

    pub fn add_timeline_layer(mut self, timeline: &Timeline) -> Self {
        for id in timeline.attributes() {
            self.add_layer(id, Layer::Track(timeline.create_track_sampler(&ATTRIBUTES[id])));
        }
        self
    }

    pub fn add_weather_layers(mut self) -> Self {
        let (rain, thunder) = (&*weather_attributes::RAIN, &*weather_attributes::THUNDER);
        let mut attributes: Vec<usize> = rain.key_set().collect();
        attributes.extend(thunder.key_set().filter(|id| !rain.contains(*id)));
        for id in attributes {
            let state_change_lerp = ATTRIBUTES[id].ty.state_change_lerp();
            self.add_layer(id, Layer::Weather { rain: rain.get(id).cloned(), thunder: thunder.get(id).cloned(), state_change_lerp });
        }
        self
    }

    pub fn build(self) -> EnvironmentAttributeSystem {
        let samplers = self
            .layers_by_attribute
            .into_iter()
            .enumerate()
            .map(|(id, layers)| (!layers.is_empty()).then(|| ValueSampler::bake(&ATTRIBUTES[id], layers)))
            .collect();
        EnvironmentAttributeSystem { samplers }
    }
}

struct ValueSampler {
    attribute: &'static EnvironmentAttributeInfo,
    base_value: Value,
    layers: Vec<Layer>,
    is_affected_by_position: bool,
    cached_tick_value: OnceCell<Value>,
    cache_tick_id: i32,
}

impl ValueSampler {
    fn bake(attribute: &'static EnvironmentAttributeInfo, mut layers: Vec<Layer>) -> ValueSampler {
        let mut base_value = attribute.default_value().clone();
        let constants = layers.iter().take_while(|l| matches!(l, Layer::Constant(_))).count();
        for layer in layers.drain(..constants) {
            let Layer::Constant(entry) = layer else { unreachable!() };
            base_value = entry.apply_modifier(&base_value);
        }
        let is_affected_by_position = layers.iter().any(|l| matches!(l, Layer::Biome(_)));
        ValueSampler { attribute, base_value, layers, is_affected_by_position, cached_tick_value: OnceCell::new(), cache_tick_id: 0 }
    }

    fn invalidate_tick_cache(&mut self) {
        self.cached_tick_value.take();
        self.cache_tick_id += 1;
    }

    fn dimension_value(&self, access: &impl EnvironmentAccess) -> &Value {
        self.cached_tick_value.get_or_init(|| self.compute(None, access))
    }

    fn compute(&self, pos: Option<Vec3>, access: &impl EnvironmentAccess) -> Value {
        let mut result = self.base_value.clone();
        for layer in &self.layers {
            result = match layer {
                Layer::Constant(entry) => entry.apply_modifier(&result),
                Layer::Track(sampler) => sampler.apply_time_based(&result, self.cache_tick_id, |clock| access.clock_total_ticks(clock)),
                Layer::Weather { rain, thunder, state_change_lerp } => {
                    let thunder_level = access.thunder_level();
                    let rain_level = access.rain_level() - thunder_level;
                    if let Some(rain) = rain
                        && rain_level > 0.0
                    {
                        let rain_value = rain.apply_modifier(&result);
                        result = state_change_lerp.apply(rain_level, &result, &rain_value);
                    }
                    if let Some(thunder) = thunder
                        && thunder_level > 0.0
                    {
                        let thunder_value = thunder.apply_modifier(&result);
                        result = state_change_lerp.apply(thunder_level, &result, &thunder_value);
                    }
                    result
                }
                Layer::Biome(by_biome) => match pos {
                    None => result,
                    Some(pos) => {
                        let biome = if self.attribute.full_resolution_biomes {
                            access.biome(mth::floor(pos.x), mth::floor(pos.y), mth::floor(pos.z))
                        } else {
                            access.noise_biome(mth::floor(pos.x) >> 2, mth::floor(pos.y) >> 2, mth::floor(pos.z) >> 2)
                        };
                        match &by_biome[biome.0 as usize] {
                            Some(entry) => entry.apply_modifier(&result),
                            None => result,
                        }
                    }
                },
            };
        }
        self.attribute.sanitize_value(result)
    }
}

#[derive(Default)]
pub struct EnvironmentAttributeSystem {
    samplers: Vec<Option<ValueSampler>>,
}

impl EnvironmentAttributeSystem {
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn invalidate_tick_cache(&mut self) {
        self.samplers.iter_mut().flatten().for_each(ValueSampler::invalidate_tick_cache);
    }

    fn sampler(&self, id: usize) -> Option<&ValueSampler> {
        self.samplers.get(id).and_then(Option::as_ref)
    }

    pub fn get_dimension_value<T: AttributeValue>(&self, attribute: EnvironmentAttribute<T>, access: &impl EnvironmentAccess) -> T {
        match self.sampler(attribute.id()) {
            Some(sampler) => T::from_value(sampler.dimension_value(access)),
            None => T::from_value(attribute.info().default_value()),
        }
    }

    pub fn get_value<T: AttributeValue>(&self, attribute: EnvironmentAttribute<T>, pos: Vec3, access: &impl EnvironmentAccess) -> T {
        match self.sampler(attribute.id()) {
            Some(sampler) if sampler.is_affected_by_position => T::from_value(&sampler.compute(Some(pos), access)),
            Some(sampler) => T::from_value(sampler.dimension_value(access)),
            None => T::from_value(attribute.info().default_value()),
        }
    }

    pub fn get_loot_value(&self, attribute: &EnvironmentAttributeInfo, origin: Option<Vec3>, access: &impl EnvironmentAccess) -> Value {
        if attribute.is_positional {
            let Some(position) = origin else { return attribute.default_value().clone() };
            return match self.sampler(attribute.id) {
                Some(sampler) if sampler.is_affected_by_position => sampler.compute(Some(position), access),
                Some(sampler) => sampler.dimension_value(access).clone(),
                None => attribute.default_value().clone(),
            };
        }
        match self.sampler(attribute.id) {
            Some(sampler) => sampler.dimension_value(access).clone(),
            None => attribute.default_value().clone(),
        }
    }

    pub fn reader<'a, A: EnvironmentAccess>(&'a self, access: &'a A) -> EnvironmentAttributeReader<'a, A> {
        EnvironmentAttributeReader { system: self, access }
    }

    #[cfg(test)]
    pub(super) fn constant_base_value(&self, id: usize) -> &Value {
        self.sampler(id).map_or(ATTRIBUTES[id].default_value(), |s| &s.base_value)
    }

    #[cfg(test)]
    pub(super) fn is_affected_by_position(&self, id: usize) -> bool {
        self.sampler(id).is_some_and(|s| s.is_affected_by_position)
    }
}

pub struct EnvironmentAttributeReader<'a, A: EnvironmentAccess> {
    system: &'a EnvironmentAttributeSystem,
    access: &'a A,
}

impl<A: EnvironmentAccess> EnvironmentAttributeReader<'_, A> {
    pub fn get_dimension_value<T: AttributeValue>(&self, attribute: EnvironmentAttribute<T>) -> T {
        self.system.get_dimension_value(attribute, self.access)
    }

    pub fn get_value<T: AttributeValue>(&self, attribute: EnvironmentAttribute<T>, pos: Vec3) -> T {
        self.system.get_value(attribute, pos, self.access)
    }

    pub fn get_value_at<T: AttributeValue>(&self, attribute: EnvironmentAttribute<T>, pos: BlockPos) -> T {
        self.get_value(attribute, Vec3::at_center_of(pos))
    }
}
