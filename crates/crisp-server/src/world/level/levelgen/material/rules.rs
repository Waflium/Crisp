use std::collections::HashMap;
use std::sync::Arc;

use crisp_core::mth;
use crisp_core::{BlockPos, RandomSource};
use crisp_data::BlockState;
use serde_json::Value;

use super::MaterialSystem;
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::density_function::{DensityVolume, Df, Parser, Sampler, SamplerContext, float, resource_name};
use crate::world::level::levelgen::synth::NoiseStack;
use crate::world::level::levelgen::{PositionalRandomFactory, RandomState, parse_block_state};

#[derive(Clone, Copy, Debug)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
    RelativeToSeaLevel(i32),
}

#[derive(Clone, Copy)]
pub struct WorldGenerationContext {
    pub min_gen_y: i32,
    pub gen_depth: i32,
    pub sea_level: i32,
}

impl VerticalAnchor {
    pub fn parse(v: &Value) -> Result<VerticalAnchor, String> {
        let get = |key| v.get(key).and_then(Value::as_i64).map(|i| i as i32);
        if let Some(y) = get("absolute") {
            Ok(VerticalAnchor::Absolute(y))
        } else if let Some(o) = get("above_bottom") {
            Ok(VerticalAnchor::AboveBottom(o))
        } else if let Some(o) = get("below_top") {
            Ok(VerticalAnchor::BelowTop(o))
        } else if let Some(o) = get("relative_to_sea_level") {
            Ok(VerticalAnchor::RelativeToSeaLevel(o))
        } else {
            Err(format!("bad vertical anchor {v}"))
        }
    }

    pub fn resolve_y(self, context: &WorldGenerationContext) -> i32 {
        match self {
            VerticalAnchor::Absolute(y) => y,
            VerticalAnchor::AboveBottom(o) => context.min_gen_y + o,
            VerticalAnchor::BelowTop(o) => context.gen_depth - 1 + context.min_gen_y - o,
            VerticalAnchor::RelativeToSeaLevel(o) => context.sea_level + o,
        }
    }
}

pub enum MaterialCondition {
    Biome(Vec<Biome>),
    NoiseThreshold { noise: String, min: f64, max: f64, is_3d: bool },
    VerticalGradient { random_name: String, true_at_and_below: VerticalAnchor, false_at_and_above: VerticalAnchor },
    YAbove { anchor: VerticalAnchor, surface_depth_multiplier: i32, add_stone_depth: bool },
    Water { offset: i32, surface_depth_multiplier: i32, add_stone_depth: bool },
    Temperature,
    Steep,
    Not(Arc<MaterialCondition>),
    Hole,
    AbovePreliminarySurface,
    StoneDepth { offset: i32, add_surface_depth: bool, secondary_depth_range: i32, ceiling: bool },
}

pub enum MaterialRule {
    Block(BlockState),
    Bandlands,
    Sequence(Vec<Arc<MaterialRule>>),
    Condition(Arc<MaterialCondition>, Arc<MaterialRule>),
    OreVein { ore: BlockState, raw_ore: BlockState, filler: BlockState, raw_ore_chance: f32, density: Df, richness: Df, filler_gap: Df },
}

pub struct MaterialParser {
    rules: HashMap<String, Arc<MaterialRule>>,
    conditions: HashMap<String, Arc<MaterialCondition>>,
}

fn source(table: &[(&str, &'static str)], name: &str) -> Result<Value, String> {
    let source = table.iter().find(|(n, _)| *n == name).ok_or_else(|| format!("unknown entry {name}"))?.1;
    serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))
}

impl MaterialParser {
    pub fn new() -> MaterialParser {
        MaterialParser { rules: HashMap::new(), conditions: HashMap::new() }
    }

    pub fn rule(&mut self, v: &Value, parser: &mut Parser) -> Result<Arc<MaterialRule>, String> {
        if let Some(name) = v.as_str() {
            let name = resource_name(name);
            if let Some(rule) = self.rules.get(&name) {
                return Ok(rule.clone());
            }
            let rule = self.rule(&source(crisp_data::MATERIAL_RULES, &name)?, parser).map_err(|e| format!("{name}: {e}"))?;
            self.rules.insert(name, rule.clone());
            return Ok(rule);
        }
        let field = |key: &str| v.get(key).ok_or_else(|| format!("missing {key}"));
        let kind = v.get("type").and_then(Value::as_str).ok_or("rule without type")?;
        let rule = match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "block" => MaterialRule::Block(parse_block_state(field("result_state")?)?),
            "bandlands" => MaterialRule::Bandlands,
            "sequence" => {
                let rules = field("sequence")?.as_array().ok_or("sequence")?.iter().map(|r| self.rule(r, parser)).collect::<Result<Vec<_>, _>>()?;
                MaterialRule::Sequence(rules)
            }
            "condition" => {
                let condition = self.condition(field("if_true")?)?;
                MaterialRule::Condition(condition, self.rule(field("then_run")?, parser)?)
            }
            "ore_vein" => MaterialRule::OreVein {
                ore: parse_block_state(field("ore_block")?)?,
                raw_ore: parse_block_state(field("raw_ore_block")?)?,
                filler: parse_block_state(field("filler_block")?)?,
                raw_ore_chance: float(field("raw_ore_chance")?)?,
                density: parser.parse(field("density")?)?,
                richness: parser.parse(field("richness")?)?,
                filler_gap: parser.parse(field("filler_gap")?)?,
            },
            other => return Err(format!("unknown material rule {other}")),
        };
        Ok(Arc::new(rule))
    }

    fn condition(&mut self, v: &Value) -> Result<Arc<MaterialCondition>, String> {
        if let Some(name) = v.as_str() {
            let name = resource_name(name);
            if let Some(condition) = self.conditions.get(&name) {
                return Ok(condition.clone());
            }
            let condition = self.condition(&source(crisp_data::MATERIAL_CONDITIONS, &name)?).map_err(|e| format!("{name}: {e}"))?;
            self.conditions.insert(name, condition.clone());
            return Ok(condition);
        }
        let field = |key: &str| v.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| field(key).and_then(|v| v.as_i64().map(|i| i as i32).ok_or_else(|| format!("{key} is not an int")));
        let boolean = |key: &str| field(key).and_then(|v| v.as_bool().ok_or_else(|| format!("{key} is not a boolean")));
        let kind = v.get("type").and_then(Value::as_str).ok_or("condition without type")?;
        let condition = match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "biome" => MaterialCondition::Biome(parse_biomes(field("biome_is")?)?),
            "noise_threshold" => MaterialCondition::NoiseThreshold {
                noise: resource_name(field("noise")?.as_str().ok_or("noise")?),
                min: field("min_threshold")?.as_f64().ok_or("min_threshold")?,
                max: field("max_threshold")?.as_f64().ok_or("max_threshold")?,
                is_3d: v.get("is_3d").and_then(Value::as_bool).unwrap_or(false),
            },
            "vertical_gradient" => MaterialCondition::VerticalGradient {
                random_name: resource_name(field("random_name")?.as_str().ok_or("random_name")?),
                true_at_and_below: VerticalAnchor::parse(field("true_at_and_below")?)?,
                false_at_and_above: VerticalAnchor::parse(field("false_at_and_above")?)?,
            },
            "y_above" => MaterialCondition::YAbove {
                anchor: VerticalAnchor::parse(field("anchor")?)?,
                surface_depth_multiplier: int("surface_depth_multiplier")?,
                add_stone_depth: boolean("add_stone_depth")?,
            },
            "water" => MaterialCondition::Water {
                offset: int("offset")?,
                surface_depth_multiplier: int("surface_depth_multiplier")?,
                add_stone_depth: boolean("add_stone_depth")?,
            },
            "temperature" => MaterialCondition::Temperature,
            "steep" => MaterialCondition::Steep,
            "not" => MaterialCondition::Not(self.condition(field("invert")?)?),
            "hole" => MaterialCondition::Hole,
            "above_preliminary_surface" => MaterialCondition::AbovePreliminarySurface,
            "stone_depth" => MaterialCondition::StoneDepth {
                offset: int("offset")?,
                add_surface_depth: boolean("add_surface_depth")?,
                secondary_depth_range: int("secondary_depth_range")?,
                ceiling: match field("surface_type")?.as_str() {
                    Some("ceiling") => true,
                    Some("floor") => false,
                    s => return Err(format!("bad surface_type {s:?}")),
                },
            },
            other => return Err(format!("unknown material condition {other}")),
        };
        Ok(Arc::new(condition))
    }
}

fn parse_biomes(v: &Value) -> Result<Vec<Biome>, String> {
    let names: Vec<&str> = match v {
        Value::String(s) => vec![s.as_str()],
        Value::Array(a) => a.iter().map(|b| b.as_str().ok_or("biome ids are strings")).collect::<Result<_, _>>()?,
        _ => return Err(format!("bad biome set {v}")),
    };
    let mut biomes = Vec::new();
    for name in names {
        if let Some(tag) = name.strip_prefix('#') {
            for &id in crisp_data::tag("minecraft:worldgen/biome", &resource_name(tag)) {
                biomes.push(Biome(id as u16));
            }
        } else {
            biomes.push(Biome::by_name(&resource_name(name)).ok_or_else(|| format!("unknown biome {name}"))?);
        }
    }
    Ok(biomes)
}

pub struct RuleContext<'a> {
    pub system: &'a MaterialSystem,
    pub state: &'a RandomState,
    pub samplers: &'a mut SamplerContext,
    pub biome_getter: &'a mut dyn FnMut(i32, i32, i32) -> Biome,
    pub generation: WorldGenerationContext,
    pub possible_biomes: Option<&'a [bool]>,
    expected_volume: DensityVolume,
    preliminary_volume: DensityVolume,
    preliminary_buffer: Option<Vec<f32>>,
    preliminary_sampler: Arc<Sampler>,
    last_update_xz: i64,
    pub block_x: i32,
    pub block_z: i32,
    surface_gradient_x: i32,
    surface_gradient_z: i32,
    surface_depth: i32,
    last_surface_secondary_update: i64,
    surface_secondary: f64,
    last_min_surface_level_update: i64,
    min_surface_level: i32,
    last_update_y: i64,
    biome: Option<Biome>,
    pub block_y: i32,
    water_height: i32,
    stone_depth_below: i32,
    stone_depth_above: i32,
    noises: Vec<NoiseSlot>,
}

struct NoiseSlot {
    name: String,
    is_3d: bool,
    noise: Arc<NoiseStack>,
    last_update: i64,
    value: f64,
}

impl<'a> RuleContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        system: &'a MaterialSystem,
        state: &'a RandomState,
        expected_volume: DensityVolume,
        samplers: &'a mut SamplerContext,
        biome_getter: &'a mut dyn FnMut(i32, i32, i32) -> Biome,
        generation: WorldGenerationContext,
        possible_biomes: Option<&'a [bool]>,
    ) -> RuleContext<'a> {
        let last_update_xz = -9223372036854775807;
        RuleContext {
            preliminary_sampler: state.sampler(&system.preliminary_surface_function),
            system,
            state,
            samplers,
            biome_getter,
            generation,
            possible_biomes,
            preliminary_volume: DensityVolume::new(expected_volume.size_x, 1, expected_volume.size_z, expected_volume.min_x, 0, expected_volume.min_z),
            expected_volume,
            preliminary_buffer: None,
            last_update_xz,
            block_x: 0,
            block_z: 0,
            surface_gradient_x: 0,
            surface_gradient_z: 0,
            surface_depth: 0,
            last_surface_secondary_update: last_update_xz - 1,
            surface_secondary: 0.0,
            last_min_surface_level_update: last_update_xz - 1,
            min_surface_level: 0,
            last_update_y: -9223372036854775807,
            biome: None,
            block_y: 0,
            water_height: 0,
            stone_depth_below: 0,
            stone_depth_above: 0,
            noises: Vec::new(),
        }
    }

    pub fn update_xz(&mut self, x: i32, z: i32, gradient_x: i32, gradient_z: i32) {
        self.last_update_xz += 1;
        self.last_update_y += 1;
        self.block_x = x;
        self.block_z = z;
        self.surface_gradient_x = gradient_x;
        self.surface_gradient_z = gradient_z;
        self.surface_depth = self.system.surface_depth(x, z);
    }

    pub fn update_y(&mut self, stone_depth_above: i32, stone_depth_below: i32, water_height: i32, y: i32) {
        self.last_update_y += 1;
        self.biome = None;
        self.block_y = y;
        self.water_height = water_height;
        self.stone_depth_below = stone_depth_below;
        self.stone_depth_above = stone_depth_above;
    }

    fn surface_secondary(&mut self) -> f64 {
        if self.last_surface_secondary_update != self.last_update_xz {
            self.last_surface_secondary_update = self.last_update_xz;
            self.surface_secondary = self.system.surface_secondary(self.block_x, self.block_z);
        }
        self.surface_secondary
    }

    fn biome(&mut self) -> Biome {
        if self.biome.is_none() {
            self.biome = Some((self.biome_getter)(self.block_x, self.block_y, self.block_z));
        }
        self.biome.unwrap()
    }

    pub fn min_surface_level(&mut self) -> i32 {
        if self.last_min_surface_level_update != self.last_update_xz {
            self.last_min_surface_level_update = self.last_update_xz;
            let level = match self.preliminary_volume.index_of_block(self.block_x, 0, self.block_z) {
                Some(index) => {
                    if self.preliminary_buffer.is_none() {
                        let mut buffer = vec![0.0; self.preliminary_volume.size()];
                        self.preliminary_sampler.sample_volume(self.samplers, &mut buffer, &self.preliminary_volume);
                        self.preliminary_buffer = Some(buffer);
                    }
                    self.preliminary_buffer.as_ref().unwrap()[index]
                }
                None => self.preliminary_sampler.sample_value(self.samplers, self.block_x, 0, self.block_z),
            };
            self.min_surface_level = mth::floor_f(level) + self.surface_depth - 8;
        }
        self.min_surface_level
    }

    fn noise_slot(&mut self, name: &str, is_3d: bool) -> usize {
        if let Some(i) = self.noises.iter().position(|s| s.name == name && s.is_3d == is_3d) {
            return i;
        }
        let last_update = if is_3d { self.last_update_y } else { self.last_update_xz } - 1;
        self.noises.push(NoiseSlot { name: name.to_owned(), is_3d, noise: self.state.noise(name), last_update, value: 0.0 });
        self.noises.len() - 1
    }

    fn noise_value(&mut self, slot: usize) -> f64 {
        let update = if self.noises[slot].is_3d { self.last_update_y } else { self.last_update_xz };
        let s = &mut self.noises[slot];
        if s.last_update != update {
            let y = if s.is_3d { self.block_y as f64 } else { 0.0 };
            s.value = s.noise.get(self.block_x as f64, y, self.block_z as f64) as f64;
            s.last_update = update;
        }
        s.value
    }

    fn densities(&mut self, function: &Df, prefill: bool) -> DensityGetter {
        let sampler = self.state.sampler(function);
        let buffer = prefill.then(|| {
            let mut buffer = vec![0.0; self.expected_volume.size()];
            sampler.sample_volume(self.samplers, &mut buffer, &self.expected_volume);
            buffer
        });
        DensityGetter { sampler, buffer }
    }

    fn density(&mut self, getter: &DensityGetter) -> f32 {
        if let Some(buffer) = &getter.buffer
            && let Some(index) = self.expected_volume.index_of_block(self.block_x, self.block_y, self.block_z)
        {
            return buffer[index];
        }
        getter.sampler.sample_value(self.samplers, self.block_x, self.block_y, self.block_z)
    }

    fn block_pos(&self) -> BlockPos {
        BlockPos::new(self.block_x, self.block_y, self.block_z)
    }
}

struct DensityGetter {
    sampler: Arc<Sampler>,
    buffer: Option<Vec<f32>>,
}

pub enum Laziness {
    None,
    Xz,
    Y,
}

pub enum Condition {
    Constant(bool),
    Lazy { laziness: Laziness, last_update: i64, result: bool, test: Test },
}

pub enum Test {
    Biome(Vec<Biome>),
    Noise { slot: usize, min: f64, max: f64 },
    VerticalGradient { true_at_and_below: i32, false_at_and_above: i32, random: PositionalRandomFactory },
    YAbove { anchor: VerticalAnchor, surface_depth_multiplier: i32, add_stone_depth: bool },
    Water { offset: i32, surface_depth_multiplier: i32, add_stone_depth: bool },
    Temperature,
    Steep,
    Not(Box<Condition>),
    Hole,
    AbovePreliminarySurface,
    StoneDepth { offset: i32, add_surface_depth: bool, secondary_depth_range: i32, ceiling: bool },
}

impl Condition {
    fn compile(condition: &MaterialCondition, ctx: &mut RuleContext) -> Condition {
        let lazy = |laziness: Laziness, test: Test, ctx: &RuleContext| {
            let last_update = match laziness {
                Laziness::Xz => ctx.last_update_xz - 1,
                Laziness::Y => ctx.last_update_y - 1,
                Laziness::None => 0,
            };
            Condition::Lazy { laziness, last_update, result: false, test }
        };
        match condition {
            MaterialCondition::Biome(biomes) => {
                if let Some(possible) = ctx.possible_biomes {
                    if !biomes.iter().any(|b| possible[b.0 as usize]) {
                        return Condition::Constant(false);
                    }
                    if possible.iter().enumerate().all(|(i, &p)| !p || biomes.contains(&Biome(i as u16))) {
                        return Condition::Constant(true);
                    }
                }
                lazy(Laziness::Y, Test::Biome(biomes.clone()), ctx)
            }
            MaterialCondition::NoiseThreshold { noise, min, max, is_3d } => {
                let slot = ctx.noise_slot(noise, *is_3d);
                lazy(Laziness::None, Test::Noise { slot, min: *min, max: *max }, ctx)
            }
            MaterialCondition::VerticalGradient { random_name, true_at_and_below, false_at_and_above } => {
                let true_at_and_below = true_at_and_below.resolve_y(&ctx.generation);
                let false_at_and_above = false_at_and_above.resolve_y(&ctx.generation);
                let random = ctx.state.random_factory(random_name);
                lazy(Laziness::Y, Test::VerticalGradient { true_at_and_below, false_at_and_above, random }, ctx)
            }
            MaterialCondition::YAbove { anchor, surface_depth_multiplier, add_stone_depth } => lazy(
                Laziness::Y,
                Test::YAbove { anchor: *anchor, surface_depth_multiplier: *surface_depth_multiplier, add_stone_depth: *add_stone_depth },
                ctx,
            ),
            MaterialCondition::Water { offset, surface_depth_multiplier, add_stone_depth } => lazy(
                Laziness::Y,
                Test::Water { offset: *offset, surface_depth_multiplier: *surface_depth_multiplier, add_stone_depth: *add_stone_depth },
                ctx,
            ),
            MaterialCondition::Temperature => lazy(Laziness::None, Test::Temperature, ctx),
            MaterialCondition::Steep => lazy(Laziness::Xz, Test::Steep, ctx),
            MaterialCondition::Not(target) => {
                let target = Condition::compile(target, ctx);
                lazy(Laziness::None, Test::Not(Box::new(target)), ctx)
            }
            MaterialCondition::Hole => lazy(Laziness::Xz, Test::Hole, ctx),
            MaterialCondition::AbovePreliminarySurface => lazy(Laziness::None, Test::AbovePreliminarySurface, ctx),
            MaterialCondition::StoneDepth { offset, add_surface_depth, secondary_depth_range, ceiling } => lazy(
                Laziness::Y,
                Test::StoneDepth {
                    offset: *offset,
                    add_surface_depth: *add_surface_depth,
                    secondary_depth_range: *secondary_depth_range,
                    ceiling: *ceiling,
                },
                ctx,
            ),
        }
    }

    fn test(&mut self, ctx: &mut RuleContext) -> bool {
        match self {
            Condition::Constant(b) => *b,
            Condition::Lazy { laziness, last_update, result, test } => {
                let update = match laziness {
                    Laziness::None => return Self::compute(test, ctx),
                    Laziness::Xz => ctx.last_update_xz,
                    Laziness::Y => ctx.last_update_y,
                };
                if *last_update != update {
                    *last_update = update;
                    *result = Self::compute(test, ctx);
                }
                *result
            }
        }
    }

    fn compute(test: &mut Test, ctx: &mut RuleContext) -> bool {
        match test {
            Test::Biome(biomes) => {
                let biome = ctx.biome();
                biomes.contains(&biome)
            }
            Test::Noise { slot, min, max } => {
                let value = ctx.noise_value(*slot);
                value >= *min && value <= *max
            }
            Test::VerticalGradient { true_at_and_below, false_at_and_above, random } => {
                let y = ctx.block_y;
                if y <= *true_at_and_below {
                    return true;
                }
                if y >= *false_at_and_above {
                    return false;
                }
                let probability = mth::map(y as f64, *true_at_and_below as f64, *false_at_and_above as f64, 1.0, 0.0);
                let mut random = random.at(ctx.block_x, y, ctx.block_z);
                (random.next_float() as f64) < probability
            }
            Test::YAbove { anchor, surface_depth_multiplier, add_stone_depth } => {
                ctx.block_y + if *add_stone_depth { ctx.stone_depth_above } else { 0 }
                    >= anchor.resolve_y(&ctx.generation) + ctx.surface_depth * *surface_depth_multiplier
            }
            Test::Water { offset, surface_depth_multiplier, add_stone_depth } => {
                ctx.water_height == i32::MIN
                    || ctx.block_y + if *add_stone_depth { ctx.stone_depth_above } else { 0 }
                        >= ctx.water_height + *offset + ctx.surface_depth * *surface_depth_multiplier
            }
            Test::Temperature => {
                let biome = ctx.biome();
                biome.cold_enough_to_snow(ctx.block_pos(), ctx.system.sea_level)
            }
            Test::Steep => ctx.surface_gradient_x <= -4 || ctx.surface_gradient_z >= 4,
            Test::Not(target) => !target.test(ctx),
            Test::Hole => ctx.surface_depth <= 0,
            Test::AbovePreliminarySurface => ctx.block_y >= ctx.min_surface_level(),
            Test::StoneDepth { offset, add_surface_depth, secondary_depth_range, ceiling } => {
                let stone_depth = if *ceiling { ctx.stone_depth_below } else { ctx.stone_depth_above };
                let surface_depth = if *add_surface_depth { ctx.surface_depth } else { 0 };
                let secondary = if *secondary_depth_range == 0 {
                    0
                } else {
                    mth::map(ctx.surface_secondary(), -1.0, 1.0, 0.0, *secondary_depth_range as f64) as i32
                };
                stone_depth <= 1 + *offset + surface_depth + secondary
            }
        }
    }
}

pub enum Rule {
    Block(BlockState),
    Bandlands,
    Sequence(Vec<Rule>),
    Condition(Condition, Box<Rule>),
    OreVein(Box<OreVein>),
}

pub struct OreVein {
    density: DensityGetter,
    richness: DensityGetter,
    filler_gap: DensityGetter,
    random: PositionalRandomFactory,
    ore: BlockState,
    raw_ore: BlockState,
    filler: BlockState,
    raw_ore_chance: f32,
}

impl Rule {
    pub fn compile(rule: &MaterialRule, ctx: &mut RuleContext) -> Rule {
        match rule {
            MaterialRule::Block(state) => Rule::Block(*state),
            MaterialRule::Bandlands => Rule::Bandlands,
            MaterialRule::Sequence(rules) if rules.len() == 1 => Rule::compile(&rules[0], ctx),
            MaterialRule::Sequence(rules) => Rule::Sequence(rules.iter().map(|r| Rule::compile(r, ctx)).collect()),
            MaterialRule::Condition(condition, then_run) => {
                let condition = Condition::compile(condition, ctx);
                Rule::Condition(condition, Box::new(Rule::compile(then_run, ctx)))
            }
            MaterialRule::OreVein { ore, raw_ore, filler, raw_ore_chance, density, richness, filler_gap } => {
                let density = ctx.densities(density, true);
                let richness = ctx.densities(richness, true);
                let filler_gap = ctx.densities(filler_gap, false);
                let random = ctx.state.random_factory("minecraft:ore");
                Rule::OreVein(Box::new(OreVein {
                    density,
                    richness,
                    filler_gap,
                    random,
                    ore: *ore,
                    raw_ore: *raw_ore,
                    filler: *filler,
                    raw_ore_chance: *raw_ore_chance,
                }))
            }
        }
    }

    pub fn try_apply(&mut self, ctx: &mut RuleContext, x: i32, y: i32, z: i32) -> Option<BlockState> {
        match self {
            Rule::Block(state) => Some(*state),
            Rule::Bandlands => Some(ctx.system.band(x, y, z)),
            Rule::Sequence(rules) => rules.iter_mut().find_map(|r| r.try_apply(ctx, x, y, z)),
            Rule::Condition(condition, then_run) => {
                if condition.test(ctx) {
                    then_run.try_apply(ctx, x, y, z)
                } else {
                    None
                }
            }
            Rule::OreVein(vein) => {
                let density = ctx.density(&vein.density);
                if density <= 0.0 {
                    return None;
                }
                let mut random = vein.random.at(x, y, z);
                if random.next_float() > density {
                    return None;
                }
                let richness = ctx.density(&vein.richness);
                if random.next_float() < richness && ctx.density(&vein.filler_gap) < 0.0 {
                    Some(if random.next_float() < vein.raw_ore_chance { vein.raw_ore } else { vein.ore })
                } else {
                    Some(vein.filler)
                }
            }
        }
    }
}
