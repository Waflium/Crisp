use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FrontAndTop, ORIENTATION};
use crisp_data::blocks;
use crisp_protocol::nbt::Nbt;
use serde_json::Value;

use super::structure_template_pool::{Projection, shuffle};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::placement::feature_placer::FeaturePlacer;
use crate::world::level::levelgen::placement::placed_feature::{self, placed_feature_index};
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::{StructureProcessor, StructureProcessorListHolder};
use crate::world::level::levelgen::structure::templatesystem::structure_template::{JigsawBlockInfo, JointType, StructureTemplate};
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

static STRUCTURE_BLOCK: LazyLock<StructureProcessor> = LazyLock::new(|| StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK]));
static STRUCTURE_AND_AIR: LazyLock<StructureProcessor> = LazyLock::new(|| StructureProcessor::BlockIgnore(vec![blocks::AIR, blocks::STRUCTURE_BLOCK]));
static JIGSAW_REPLACEMENT: StructureProcessor = StructureProcessor::JigsawReplacement;

pub struct SinglePoolElement {
    pub template: String,
    pub processors: StructureProcessorListHolder,
    pub projection: Projection,
    pub override_liquid_settings: Option<bool>,
}

pub enum StructurePoolElement {
    Single(SinglePoolElement),
    LegacySingle(SinglePoolElement),
    List { elements: Vec<StructurePoolElement>, projection: Projection },
    Feature { feature: u32, projection: Projection },
    Empty,
}

fn parse_liquid_settings(name: &str) -> Result<bool, String> {
    match name {
        "apply_waterlogging" => Ok(true),
        "ignore_waterlogging" => Ok(false),
        other => Err(format!("unknown liquid settings {other}")),
    }
}

pub fn liquid_settings_name(apply_waterlogging: bool) -> &'static str {
    if apply_waterlogging { "apply_waterlogging" } else { "ignore_waterlogging" }
}

impl SinglePoolElement {
    fn parse(json: &Value, projection: Projection) -> Result<SinglePoolElement, String> {
        Ok(SinglePoolElement {
            template: resource(json.get("location").and_then(Value::as_str).ok_or("missing location")?),
            processors: StructureProcessorListHolder::parse(json.get("processors").ok_or("missing processors")?)?,
            projection,
            override_liquid_settings: json.get("override_liquid_settings").and_then(Value::as_str).map(parse_liquid_settings).transpose()?,
        })
    }

    fn get_template(&self) -> &'static StructureTemplate {
        structure_template_manager::get_or_create(&self.template)
    }

    fn to_nbt(&self, element_type: &str) -> Nbt {
        let mut fields = vec![
            ("element_type".to_owned(), Nbt::String(element_type.to_owned())),
            ("location".to_owned(), Nbt::String(self.template.clone())),
            ("processors".to_owned(), self.processors.to_nbt()),
            ("projection".to_owned(), Nbt::String(self.projection.get_name().to_owned())),
        ];
        if let Some(apply) = self.override_liquid_settings {
            fields.push(("override_liquid_settings".to_owned(), Nbt::String(liquid_settings_name(apply).to_owned())));
        }
        Nbt::Compound(fields)
    }

    fn get_settings(&self, rotation: Rotation, chunk_bb: &BoundingBox, apply_waterlogging: bool, keep_jigsaws: bool, legacy: bool) -> StructurePlaceSettings<'_> {
        let mut settings = StructurePlaceSettings {
            bounding_box: Some(*chunk_bb),
            rotation,
            known_shape: true,
            apply_waterlogging: self.override_liquid_settings.unwrap_or(apply_waterlogging),
            ..StructurePlaceSettings::default()
        };
        if !legacy {
            settings.processors.push(&*STRUCTURE_BLOCK);
        }
        if !keep_jigsaws {
            settings.processors.push(&JIGSAW_REPLACEMENT);
        }
        settings.processors.extend(self.processors.value().list.iter());
        settings.processors.extend(self.projection.get_processors().iter());
        if legacy {
            settings.processors.push(&*STRUCTURE_AND_AIR);
        }
        settings
    }
}

impl StructurePoolElement {
    pub fn parse(json: &Value, list_projection: Option<Projection>) -> Result<StructurePoolElement, String> {
        let ty = json.get("element_type").and_then(Value::as_str).ok_or("pool element without element_type")?;
        let projection = || -> Result<Projection, String> {
            match list_projection {
                Some(projection) => Ok(projection),
                None => {
                    let name = json.get("projection").and_then(Value::as_str).ok_or("missing projection")?;
                    Projection::by_name(name).ok_or_else(|| format!("unknown projection {name}"))
                }
            }
        };
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "single_pool_element" => StructurePoolElement::Single(SinglePoolElement::parse(json, projection()?)?),
            "legacy_single_pool_element" => StructurePoolElement::LegacySingle(SinglePoolElement::parse(json, projection()?)?),
            "list_pool_element" => {
                let projection = projection()?;
                let elements: Vec<StructurePoolElement> = json
                    .get("elements")
                    .and_then(Value::as_array)
                    .ok_or("missing elements")?
                    .iter()
                    .map(|element| StructurePoolElement::parse(element, Some(projection)))
                    .collect::<Result<_, _>>()?;
                if elements.is_empty() {
                    return Err("Elements are empty".into());
                }
                StructurePoolElement::List { elements, projection }
            }
            "feature_pool_element" => {
                let name = json.get("feature").and_then(Value::as_str).ok_or("missing feature")?;
                StructurePoolElement::Feature { feature: placed_feature_index(name).ok_or_else(|| format!("unknown placed feature {name}"))?, projection: projection()? }
            }
            "empty_pool_element" => StructurePoolElement::Empty,
            other => return Err(format!("unknown pool element {other}")),
        })
    }

    pub fn load(tag: &Nbt) -> Result<StructurePoolElement, String> {
        StructurePoolElement::parse(&crate::util::codec::to_json(tag), None)
    }

    pub fn to_nbt(&self) -> Nbt {
        match self {
            StructurePoolElement::Single(element) => element.to_nbt("minecraft:single_pool_element"),
            StructurePoolElement::LegacySingle(element) => element.to_nbt("minecraft:legacy_single_pool_element"),
            StructurePoolElement::List { elements, projection } => Nbt::Compound(vec![
                ("element_type".to_owned(), Nbt::String("minecraft:list_pool_element".to_owned())),
                ("elements".to_owned(), Nbt::List(elements.iter().map(StructurePoolElement::to_nbt).collect())),
                ("projection".to_owned(), Nbt::String(projection.get_name().to_owned())),
            ]),
            StructurePoolElement::Feature { feature, projection } => Nbt::Compound(vec![
                ("element_type".to_owned(), Nbt::String("minecraft:feature_pool_element".to_owned())),
                ("feature".to_owned(), Nbt::String(crisp_data::PLACED_FEATURES[*feature as usize].0.to_owned())),
                ("projection".to_owned(), Nbt::String(projection.get_name().to_owned())),
            ]),
            StructurePoolElement::Empty => Nbt::Compound(vec![("element_type".to_owned(), Nbt::String("minecraft:empty_pool_element".to_owned()))]),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, StructurePoolElement::Empty)
    }

    pub fn get_projection(&self) -> Projection {
        match self {
            StructurePoolElement::Single(element) | StructurePoolElement::LegacySingle(element) => element.projection,
            StructurePoolElement::List { projection, .. } | StructurePoolElement::Feature { projection, .. } => *projection,
            StructurePoolElement::Empty => Projection::TerrainMatching,
        }
    }

    pub fn get_ground_level_delta(&self) -> i32 {
        1
    }

    pub fn get_size(&self, rotation: Rotation) -> BlockPos {
        match self {
            StructurePoolElement::Single(element) | StructurePoolElement::LegacySingle(element) => element.get_template().get_size_rotated(rotation),
            StructurePoolElement::List { elements, .. } => elements.iter().fold(BlockPos::ZERO, |size, element| {
                let element_size = element.get_size(rotation);
                BlockPos::new(size.x.max(element_size.x), size.y.max(element_size.y), size.z.max(element_size.z))
            }),
            StructurePoolElement::Feature { .. } | StructurePoolElement::Empty => BlockPos::ZERO,
        }
    }

    pub fn get_shuffled_jigsaw_blocks(&self, position: BlockPos, rotation: Rotation, random: &mut impl RandomSource) -> Vec<JigsawBlockInfo> {
        match self {
            StructurePoolElement::Single(element) | StructurePoolElement::LegacySingle(element) => {
                let mut jigsaws = element.get_template().get_jigsaws(position, rotation);
                shuffle(&mut jigsaws, random);
                jigsaws.sort_by(|a, b| b.selection_priority.cmp(&a.selection_priority));
                jigsaws
            }
            StructurePoolElement::List { elements, .. } => elements[0].get_shuffled_jigsaw_blocks(position, rotation, random),
            StructurePoolElement::Feature { .. } => vec![JigsawBlockInfo {
                pos: position,
                state: blocks::JIGSAW.default_state().set(ORIENTATION, FrontAndTop::from_front_and_top(Direction::Down, Direction::South)),
                joint_type: JointType::Rollable,
                name: None,
                pool: "minecraft:empty".into(),
                target: "minecraft:empty".into(),
                placement_priority: 0,
                selection_priority: 0,
            }],
            StructurePoolElement::Empty => Vec::new(),
        }
    }

    pub fn get_bounding_box(&self, position: BlockPos, rotation: Rotation) -> BoundingBox {
        match self {
            StructurePoolElement::Single(element) | StructurePoolElement::LegacySingle(element) => {
                element.get_template().get_bounding_box(&StructurePlaceSettings { rotation, ..StructurePlaceSettings::default() }, position)
            }
            StructurePoolElement::List { elements, .. } => {
                let boxes: Vec<BoundingBox> = elements.iter().filter(|e| !e.is_empty()).map(|e| e.get_bounding_box(position, rotation)).collect();
                BoundingBox::encapsulating_boxes(boxes.iter()).expect("Unable to calculate boundingbox for ListPoolElement")
            }
            StructurePoolElement::Feature { .. } => {
                let size = self.get_size(rotation);
                BoundingBox::new(position.x, position.y, position.z, position.x + size.x, position.y + size.y, position.z + size.z)
            }
            StructurePoolElement::Empty => panic!("Invalid call to EmptyPoolElement.getBoundingBox, filter me!"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn place(
        &self,
        level: &mut impl WorldGenLevel,
        position: BlockPos,
        reference_pos: BlockPos,
        rotation: Rotation,
        chunk_bb: &BoundingBox,
        random: &mut impl RandomSource,
        apply_waterlogging: bool,
        keep_jigsaws: bool,
    ) -> bool {
        match self {
            StructurePoolElement::Single(element) | StructurePoolElement::LegacySingle(element) => {
                let legacy = matches!(self, StructurePoolElement::LegacySingle(_));
                let settings = element.get_settings(rotation, chunk_bb, apply_waterlogging, keep_jigsaws, legacy);
                element.get_template().place_in_world(level, position, reference_pos, &settings, random, 18)
            }
            StructurePoolElement::List { elements, .. } => {
                elements.iter().all(|element| element.place(level, position, reference_pos, rotation, chunk_bb, random, apply_waterlogging, keep_jigsaws))
            }
            StructurePoolElement::Feature { feature, .. } => FeaturePlacer::default().place(level, placed_feature::get(*feature), random, position),
            StructurePoolElement::Empty => true,
        }
    }
}
