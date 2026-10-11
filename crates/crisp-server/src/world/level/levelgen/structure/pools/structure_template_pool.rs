use std::sync::{Arc, LazyLock, OnceLock};

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::structure_pool_element::StructurePoolElement;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Projection {
    TerrainMatching,
    Rigid,
}

static TERRAIN_MATCHING_PROCESSORS: LazyLock<[StructureProcessor; 1]> =
    LazyLock::new(|| [StructureProcessor::Gravity { heightmap: crate::world::level::chunk::HeightmapType::WorldSurfaceWg, offset: -1 }]);

impl Projection {
    pub fn by_name(name: &str) -> Option<Projection> {
        match name {
            "terrain_matching" => Some(Projection::TerrainMatching),
            "rigid" => Some(Projection::Rigid),
            _ => None,
        }
    }

    pub fn get_name(self) -> &'static str {
        match self {
            Projection::TerrainMatching => "terrain_matching",
            Projection::Rigid => "rigid",
        }
    }

    pub fn get_processors(self) -> &'static [StructureProcessor] {
        match self {
            Projection::TerrainMatching => &*TERRAIN_MATCHING_PROCESSORS,
            Projection::Rigid => &[],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TemplatePoolHolder(pub u16);

pub const EMPTY: &str = "minecraft:empty";

impl TemplatePoolHolder {
    pub fn by_name(name: &str) -> Option<TemplatePoolHolder> {
        let name = resource(name);
        crisp_data::TEMPLATE_POOLS.binary_search_by(|(n, _)| (*n).cmp(name.as_str())).ok().map(|i| TemplatePoolHolder(i as u16))
    }

    pub fn key(self) -> &'static str {
        crisp_data::TEMPLATE_POOLS[self.0 as usize].0
    }

    pub fn value(self) -> &'static StructureTemplatePool {
        POOLS[self.0 as usize].get_or_init(|| {
            let (name, source) = crisp_data::TEMPLATE_POOLS[self.0 as usize];
            let json: Value = serde_json::from_str(source).expect("template pool json");
            StructureTemplatePool::parse(&json).unwrap_or_else(|e| panic!("template pool {name}: {e}"))
        })
    }

    pub fn is(self, name: &str) -> bool {
        self.key() == name
    }
}

static POOLS: LazyLock<Vec<OnceLock<StructureTemplatePool>>> = LazyLock::new(|| (0..crisp_data::TEMPLATE_POOLS.len()).map(|_| OnceLock::new()).collect());

pub struct StructureTemplatePool {
    templates: Vec<Arc<StructurePoolElement>>,
    fallback: TemplatePoolHolder,
    max_size: OnceLock<i32>,
}

impl StructureTemplatePool {
    fn parse(json: &Value) -> Result<StructureTemplatePool, String> {
        let fallback_name = json.get("fallback").and_then(Value::as_str).ok_or("missing fallback")?;
        let fallback = TemplatePoolHolder::by_name(fallback_name).ok_or_else(|| format!("unknown pool {fallback_name}"))?;
        let mut templates = Vec::new();
        for entry in json.get("elements").and_then(Value::as_array).ok_or("missing elements")? {
            let element = Arc::new(StructurePoolElement::parse(entry.get("element").ok_or("missing element")?, None)?);
            let weight = entry.get("weight").and_then(Value::as_i64).ok_or("missing weight")?;
            for _ in 0..weight {
                templates.push(element.clone());
            }
        }
        Ok(StructureTemplatePool { templates, fallback, max_size: OnceLock::new() })
    }

    pub fn get_max_size(&self) -> i32 {
        *self.max_size.get_or_init(|| {
            self.templates
                .iter()
                .filter(|t| !t.is_empty())
                .map(|t| t.get_bounding_box(BlockPos::ZERO, Rotation::None).y_span())
                .max()
                .unwrap_or(0)
        })
    }

    pub fn get_fallback(&self) -> TemplatePoolHolder {
        self.fallback
    }

    pub fn get_random_template(&self, random: &mut impl RandomSource) -> Arc<StructurePoolElement> {
        if self.templates.is_empty() {
            Arc::new(StructurePoolElement::Empty)
        } else {
            self.templates[random.next_int_bounded(self.templates.len() as i32) as usize].clone()
        }
    }

    pub fn get_shuffled_templates(&self, random: &mut impl RandomSource) -> Vec<Arc<StructurePoolElement>> {
        let mut copy = self.templates.clone();
        shuffle(&mut copy, random);
        copy
    }

    pub fn size(&self) -> usize {
        self.templates.len()
    }
}

pub fn shuffle<T>(list: &mut [T], random: &mut impl RandomSource) {
    for i in (2..=list.len()).rev() {
        let swap_to = random.next_int_bounded(i as i32) as usize;
        list.swap(i - 1, swap_to);
    }
}
