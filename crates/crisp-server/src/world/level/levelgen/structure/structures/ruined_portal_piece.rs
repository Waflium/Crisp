use std::sync::Mutex;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, shapes};
use crisp_data::properties::PERSISTENT;
use crisp_data::{Block, BlockState, block_tags, blocks};
use crisp_protocol::nbt::Nbt;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::block::vine_block;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure_piece::get_random_horizontal_direction;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::pos_rule_test::PosRuleTest;
use crate::world::level::levelgen::structure::templatesystem::rule_test::RuleTest;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::{ProcessorRule, RuleBlockEntityModifier, StructureProcessor};
use crate::world::level::levelgen::structure::templatesystem::structure_template::StructureTemplate;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

pub const RUINED_PORTAL: &str = "minecraft:rupo";
const PROBABILITY_OF_GOLD_GONE: f32 = 0.3;
const PROBABILITY_OF_MAGMA_INSTEAD_OF_NETHERRACK: f32 = 0.07;
const PROBABILITY_OF_MAGMA_INSTEAD_OF_LAVA: f32 = 0.2;
const NETHERRACK_PROBABILITY_BY_DISTANCE: [f32; 14] = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.9, 0.9, 0.8, 0.7, 0.6, 0.4, 0.2];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VerticalPlacement {
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    Underground,
    InNether,
}

impl VerticalPlacement {
    const VALUES: [VerticalPlacement; 6] = [
        VerticalPlacement::OnLandSurface,
        VerticalPlacement::PartlyBuried,
        VerticalPlacement::OnOceanFloor,
        VerticalPlacement::InMountain,
        VerticalPlacement::Underground,
        VerticalPlacement::InNether,
    ];

    pub fn get_name(self) -> &'static str {
        match self {
            VerticalPlacement::OnLandSurface => "on_land_surface",
            VerticalPlacement::PartlyBuried => "partly_buried",
            VerticalPlacement::OnOceanFloor => "on_ocean_floor",
            VerticalPlacement::InMountain => "in_mountain",
            VerticalPlacement::Underground => "underground",
            VerticalPlacement::InNether => "in_nether",
        }
    }

    pub fn by_name(name: &str) -> Option<VerticalPlacement> {
        VerticalPlacement::VALUES.into_iter().find(|placement| placement.get_name() == name)
    }
}

#[derive(Clone, Copy)]
pub struct Properties {
    pub cold: bool,
    pub mossiness: f32,
    pub air_pocket: bool,
    pub overgrown: bool,
    pub vines: bool,
    pub replace_with_blackstone: bool,
}

impl Properties {
    fn load(tag: &Nbt) -> Option<Properties> {
        let flag = |key: &str| match tag.get(key) {
            Some(Nbt::Byte(v)) => Some(*v != 0),
            _ => None,
        };
        Some(Properties {
            cold: flag("cold")?,
            mossiness: tag.get("mossiness")?.as_f64()? as f32,
            air_pocket: flag("air_pocket")?,
            overgrown: flag("overgrown")?,
            vines: flag("vines")?,
            replace_with_blackstone: flag("replace_with_blackstone")?,
        })
    }

    fn save(&self) -> Nbt {
        Nbt::Compound(vec![
            ("cold".into(), Nbt::Byte(self.cold as i8)),
            ("mossiness".into(), Nbt::Float(self.mossiness)),
            ("air_pocket".into(), Nbt::Byte(self.air_pocket as i8)),
            ("overgrown".into(), Nbt::Byte(self.overgrown as i8)),
            ("vines".into(), Nbt::Byte(self.vines as i8)),
            ("replace_with_blackstone".into(), Nbt::Byte(self.replace_with_blackstone as i8)),
        ])
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ProcessorsKey {
    on_ocean_floor: bool,
    cold: bool,
    air_pocket: bool,
    replace_with_blackstone: bool,
    mossiness: u32,
}

static PROCESSORS: Mutex<Vec<(ProcessorsKey, &'static [StructureProcessor])>> = Mutex::new(Vec::new());

#[derive(Clone)]
pub struct RuinedPortalPiece {
    pub template: TemplateStructurePiece,
    vertical_placement: VerticalPlacement,
    properties: Properties,
}

fn pivot(template: &StructureTemplate) -> BlockPos {
    let size = template.get_size();
    BlockPos::new(size.x / 2, 0, size.z / 2)
}

fn make_settings(mirror: Mirror, rotation: Rotation, vertical_placement: VerticalPlacement, pivot: BlockPos, properties: &Properties) -> TemplatePlaceSettings {
    TemplatePlaceSettings { mirror, rotation, rotation_pivot: pivot, apply_waterlogging: true, processors: processors(vertical_placement, properties) }
}

fn processors(vertical_placement: VerticalPlacement, properties: &Properties) -> &'static [StructureProcessor] {
    let key = ProcessorsKey {
        on_ocean_floor: vertical_placement == VerticalPlacement::OnOceanFloor,
        cold: properties.cold,
        air_pocket: properties.air_pocket,
        replace_with_blackstone: properties.replace_with_blackstone,
        mossiness: properties.mossiness.to_bits(),
    };
    let mut cache = PROCESSORS.lock().unwrap();
    if let Some((_, processors)) = cache.iter().find(|(k, _)| *k == key) {
        return processors;
    }
    let processors: &'static [StructureProcessor] = Box::leak(make_processors(vertical_placement, properties).into_boxed_slice());
    cache.push((key, processors));
    processors
}

fn make_processors(vertical_placement: VerticalPlacement, properties: &Properties) -> Vec<StructureProcessor> {
    let ignore_processor = if properties.air_pocket {
        StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK])
    } else {
        StructureProcessor::BlockIgnore(vec![blocks::AIR, blocks::STRUCTURE_BLOCK])
    };
    let mut rules = Vec::with_capacity(3);
    rules.push(get_block_replace_rule_random(blocks::GOLD_BLOCK, PROBABILITY_OF_GOLD_GONE, blocks::AIR));
    rules.push(get_lava_processor_rule(vertical_placement, properties));
    if !properties.cold {
        rules.push(get_block_replace_rule_random(blocks::NETHERRACK, PROBABILITY_OF_MAGMA_INSTEAD_OF_NETHERRACK, blocks::MAGMA_BLOCK));
    }
    let mut processors = vec![
        ignore_processor,
        StructureProcessor::Rule(rules),
        StructureProcessor::BlockAge { mossiness: properties.mossiness },
        StructureProcessor::ProtectedBlocks(BlockSet::Tag(&block_tags::FEATURES_CANNOT_REPLACE)),
        StructureProcessor::LavaSubmergedBlock,
    ];
    if properties.replace_with_blackstone {
        processors.push(StructureProcessor::BlackstoneReplace);
    }
    processors
}

fn get_lava_processor_rule(vertical_placement: VerticalPlacement, properties: &Properties) -> ProcessorRule {
    if vertical_placement == VerticalPlacement::OnOceanFloor {
        get_block_replace_rule(blocks::LAVA, blocks::MAGMA_BLOCK)
    } else if properties.cold {
        get_block_replace_rule(blocks::LAVA, blocks::NETHERRACK)
    } else {
        get_block_replace_rule_random(blocks::LAVA, PROBABILITY_OF_MAGMA_INSTEAD_OF_LAVA, blocks::MAGMA_BLOCK)
    }
}

fn get_block_replace_rule_random(source: Block, probability: f32, target: Block) -> ProcessorRule {
    ProcessorRule::new(RuleTest::RandomBlockMatch { block: source, probability }, RuleTest::AlwaysTrue, PosRuleTest::AlwaysTrue, target.default_state(), RuleBlockEntityModifier::Passthrough)
}

fn get_block_replace_rule(source: Block, target: Block) -> ProcessorRule {
    ProcessorRule::new(RuleTest::BlockMatch(source), RuleTest::AlwaysTrue, PosRuleTest::AlwaysTrue, target.default_state(), RuleBlockEntityModifier::Passthrough)
}

pub fn get_height_map_type(vertical_placement: VerticalPlacement) -> HeightmapType {
    if vertical_placement == VerticalPlacement::OnOceanFloor { HeightmapType::OceanFloorWg } else { HeightmapType::WorldSurfaceWg }
}

fn get_surface_y(level: &mut WorldGenRegion, x: i32, z: i32, vertical_placement: VerticalPlacement) -> i32 {
    level.get_height(get_height_map_type(vertical_placement), x, z) - 1
}

impl RuinedPortalPiece {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        template_position: BlockPos,
        vertical_placement: VerticalPlacement,
        properties: Properties,
        template_location: &str,
        rotation: Rotation,
        mirror: Mirror,
        pivot: BlockPos,
    ) -> RuinedPortalPiece {
        RuinedPortalPiece {
            template: TemplateStructurePiece::new(
                RUINED_PORTAL,
                0,
                template_location,
                template_location.to_owned(),
                make_settings(mirror, rotation, vertical_placement, pivot, &properties),
                template_position,
            ),
            vertical_placement,
            properties,
        }
    }

    pub fn load(tag: &Nbt) -> Option<RuinedPortalPiece> {
        let vertical_placement = VerticalPlacement::by_name(tag.get("VerticalPlacement").and_then(Nbt::as_str)?)?;
        let properties = Properties::load(tag.get("Properties")?)?;
        let mirror = Mirror::by_legacy_name(tag.get("Mirror").and_then(Nbt::as_str)?)?;
        let rotation = Rotation::by_legacy_name(tag.get("Rotation").and_then(Nbt::as_str)?)?;
        let template = TemplateStructurePiece::load(RUINED_PORTAL, tag, |location| {
            make_settings(mirror, rotation, vertical_placement, pivot(structure_template_manager::get_or_create(location)), &properties)
        })?;
        Some(RuinedPortalPiece { template, vertical_placement, properties })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("Rotation".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
        tag.push(("Mirror".into(), Nbt::String(self.template.place_settings.mirror.legacy_name().to_owned())));
        tag.push(("VerticalPlacement".into(), Nbt::String(self.vertical_placement.get_name().to_owned())));
        tag.push(("Properties".into(), self.properties.save()));
    }

    pub fn post_process(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        let bounding_box = self.template.template.get_bounding_box(&self.template.place_settings.settings(None), self.template.template_position);
        if !chunk_bb.is_inside(bounding_box.get_center()) {
            return;
        }
        let chunk_bb = BoundingBox::encapsulating(chunk_bb, &bounding_box);
        self.template.post_process(level, random, &chunk_bb, reference_pos, |_, _, _, _, _| {});
        self.spread_netherrack(random, level);
        self.add_netherrack_drip_columns_below_portal(random, level);
        if self.properties.vines || self.properties.overgrown {
            for pos in self.template.piece.bounding_box.positions() {
                if self.properties.vines {
                    self.maybe_add_vines(random, level, pos);
                }
                if self.properties.overgrown {
                    self.maybe_add_leaves_above(random, level, pos);
                }
            }
        }
    }

    fn maybe_add_vines(&self, random: &mut impl RandomSource, level: &mut WorldGenRegion, pos: BlockPos) {
        let state = level.get_block_state(pos);
        if !state.is_air() && !state.is_block(blocks::VINE) {
            let direction = get_random_horizontal_direction(random);
            let neighbour_pos = pos.relative(direction);
            let neighour_state = level.get_block_state(neighbour_pos);
            if neighour_state.is_air() && shapes::is_face_full(state.collision_voxel_shape(), direction) {
                let vine_dir = vine_block::property_for_face(direction.opposite());
                level.set_block_and_update(neighbour_pos, blocks::VINE.default_state().set(vine_dir, true));
            }
        }
    }

    fn maybe_add_leaves_above(&self, random: &mut impl RandomSource, level: &mut WorldGenRegion, pos: BlockPos) {
        if random.next_float() < 0.5 && level.get_block_state(pos).is_block(blocks::NETHERRACK) && level.get_block_state(pos.above()).is_air() {
            level.set_block_and_update(pos.above(), blocks::JUNGLE_LEAVES.default_state().set(PERSISTENT, true));
        }
    }

    fn add_netherrack_drip_columns_below_portal(&self, random: &mut impl RandomSource, level: &mut WorldGenRegion) {
        let bounding_box = self.template.piece.bounding_box;
        for x in bounding_box.min.x + 1..bounding_box.max.x {
            for z in bounding_box.min.z + 1..bounding_box.max.z {
                let pos = BlockPos::new(x, bounding_box.min.y, z);
                if level.get_block_state(pos).is_block(blocks::NETHERRACK) {
                    self.add_netherrack_drip_column(random, level, pos.below());
                }
            }
        }
    }

    fn add_netherrack_drip_column(&self, random: &mut impl RandomSource, level: &mut WorldGenRegion, pos: BlockPos) {
        let mut current_pos = pos;
        self.place_netherrack_or_magma(random, level, current_pos);
        let mut remaining_cap = 8;
        while remaining_cap > 0 && random.next_float() < 0.5 {
            current_pos = current_pos.below();
            remaining_cap -= 1;
            self.place_netherrack_or_magma(random, level, current_pos);
        }
    }

    fn spread_netherrack(&self, random: &mut impl RandomSource, level: &mut WorldGenRegion) {
        let bounding_box = self.template.piece.bounding_box;
        let follow_ground_surface = self.vertical_placement == VerticalPlacement::OnLandSurface || self.vertical_placement == VerticalPlacement::OnOceanFloor;
        let center = bounding_box.get_center();
        let center_x = center.x;
        let center_z = center.z;
        let max_distance = NETHERRACK_PROBABILITY_BY_DISTANCE.len() as i32;
        let average_width = (bounding_box.x_span() + bounding_box.z_span()) / 2;
        let distance_adjustment = random.next_int_bounded(1.max(8 - average_width / 2));
        for x in center_x - max_distance..=center_x + max_distance {
            for z in center_z - max_distance..=center_z + max_distance {
                let distance = (x - center_x).abs() + (z - center_z).abs();
                let adjusted_distance = 0.max(distance + distance_adjustment);
                if adjusted_distance < max_distance {
                    let probability_of_netherrack = NETHERRACK_PROBABILITY_BY_DISTANCE[adjusted_distance as usize];
                    if random.next_double() < probability_of_netherrack as f64 {
                        let surface_y = get_surface_y(level, x, z, self.vertical_placement);
                        let y = if follow_ground_surface { surface_y } else { bounding_box.min.y.min(surface_y) };
                        let pos = BlockPos::new(x, y, z);
                        if (y - bounding_box.min.y).abs() <= 3 && self.can_block_be_replaced_by_netherrack_or_magma_at(level, pos) {
                            self.place_netherrack_or_magma(random, level, pos);
                            if self.properties.overgrown {
                                self.maybe_add_leaves_above(random, level, pos);
                            }
                            self.add_netherrack_drip_column(random, level, pos.below());
                        }
                    }
                }
            }
        }
    }

    fn can_block_be_replaced_by_netherrack_or_magma_at(&self, level: &WorldGenRegion, pos: BlockPos) -> bool {
        self.can_block_be_replaced_by_netherrack_or_magma(level.get_block_state(pos))
    }

    fn can_block_be_replaced_by_netherrack_or_magma(&self, state: BlockState) -> bool {
        !state.is_block(blocks::AIR)
            && !state.is_block(blocks::OBSIDIAN)
            && !state.is(&block_tags::FEATURES_CANNOT_REPLACE)
            && (self.vertical_placement == VerticalPlacement::InNether || !state.is_block(blocks::LAVA))
    }

    fn place_netherrack_or_magma(&self, random: &mut impl RandomSource, level: &mut WorldGenRegion, pos: BlockPos) {
        if self.can_block_be_replaced_by_netherrack_or_magma_at(level, pos) {
            if !self.properties.cold && random.next_float() < PROBABILITY_OF_MAGMA_INSTEAD_OF_NETHERRACK {
                level.set_block_and_update(pos, blocks::MAGMA_BLOCK.default_state());
            } else {
                level.set_block_and_update(pos, blocks::NETHERRACK.default_state());
            }
        }
    }
}
