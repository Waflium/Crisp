use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use serde_json::Value;

use super::Piece;
use super::ruined_portal_piece::{self, Properties, RuinedPortalPiece, VerticalPlacement};
use crate::util::valueproviders::float;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::chunk::ChunkGenerator;
use crate::world::level::levelgen::quart_pos;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};
use crate::world::level::levelgen::structure::templatesystem::structure_template;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

const STRUCTURE_LOCATION_PORTALS: [&str; 10] = [
    "minecraft:ruined_portal/portal_1",
    "minecraft:ruined_portal/portal_2",
    "minecraft:ruined_portal/portal_3",
    "minecraft:ruined_portal/portal_4",
    "minecraft:ruined_portal/portal_5",
    "minecraft:ruined_portal/portal_6",
    "minecraft:ruined_portal/portal_7",
    "minecraft:ruined_portal/portal_8",
    "minecraft:ruined_portal/portal_9",
    "minecraft:ruined_portal/portal_10",
];
const STRUCTURE_LOCATION_GIANT_PORTALS: [&str; 3] = ["minecraft:ruined_portal/giant_portal_1", "minecraft:ruined_portal/giant_portal_2", "minecraft:ruined_portal/giant_portal_3"];
const PROBABILITY_OF_GIANT_PORTAL: f32 = 0.05;
const MIN_Y_INDEX: i32 = 15;

#[derive(Clone, Copy)]
pub struct Setup {
    placement: VerticalPlacement,
    air_pocket_probability: f32,
    mossiness: f32,
    overgrown: bool,
    vines: bool,
    can_be_cold: bool,
    replace_with_blackstone: bool,
    weight: f32,
}

impl Setup {
    fn parse(json: &Value) -> Result<Setup, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let flag = |key: &str| field(key)?.as_bool().ok_or_else(|| format!("{key} is not a boolean"));
        let placement = field("placement")?.as_str().ok_or("placement is not a string")?;
        Ok(Setup {
            placement: VerticalPlacement::by_name(placement).ok_or_else(|| format!("unknown placement {placement}"))?,
            air_pocket_probability: float(field("air_pocket_probability")?)?,
            mossiness: float(field("mossiness")?)?,
            overgrown: flag("overgrown")?,
            vines: flag("vines")?,
            can_be_cold: flag("can_be_cold")?,
            replace_with_blackstone: flag("replace_with_blackstone")?,
            weight: float(field("weight")?)?,
        })
    }
}

pub struct RuinedPortalStructure {
    setups: Vec<Setup>,
}

impl RuinedPortalStructure {
    pub fn parse(json: &Value) -> Result<RuinedPortalStructure, String> {
        let setups: Vec<Setup> = json.get("setups").and_then(Value::as_array).ok_or("missing setups")?.iter().map(Setup::parse).collect::<Result<_, _>>()?;
        if setups.is_empty() {
            return Err("empty setups".into());
        }
        Ok(RuinedPortalStructure { setups })
    }

    pub fn find_generation_point(&self, context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
        let random = &mut context.random;
        let mut chosen_setup = None;
        if self.setups.len() > 1 {
            let mut total = 0.0f32;
            for s in &self.setups {
                total += s.weight;
            }
            let mut pick = random.next_float();
            for s in &self.setups {
                pick -= s.weight / total;
                if pick < 0.0 {
                    chosen_setup = Some(*s);
                    break;
                }
            }
        } else {
            chosen_setup = Some(self.setups[0]);
        }
        let setup = chosen_setup.expect("ruined portal setup");
        let air_pocket = sample(random, setup.air_pocket_probability);
        let template_location = if random.next_float() < PROBABILITY_OF_GIANT_PORTAL {
            STRUCTURE_LOCATION_GIANT_PORTALS[random.next_int_bounded(STRUCTURE_LOCATION_GIANT_PORTALS.len() as i32) as usize]
        } else {
            STRUCTURE_LOCATION_PORTALS[random.next_int_bounded(STRUCTURE_LOCATION_PORTALS.len() as i32) as usize]
        };
        let template = structure_template_manager::get_or_create(template_location);
        let rotation = Rotation::get_random(random);
        let mirror = if random.next_float() < 0.5 { Mirror::None } else { Mirror::FrontBack };
        let pivot = BlockPos::new(template.get_size().x / 2, 0, template.get_size().z / 2);
        let chunk_generator = context.chunk_generator;
        let base_position = BlockPos::new(context.chunk_pos.min_block_x(), 0, context.chunk_pos.min_block_z());
        let bounding_box = structure_template::get_bounding_box(base_position, rotation, pivot, mirror, template.get_size());
        let center = bounding_box.get_center();
        let surface_y = chunk_generator.get_base_height(center.x, center.z, ruined_portal_piece::get_height_map_type(setup.placement), context.min_y, context.height) - 1;
        let projected_y = find_suitable_y(
            &mut context.random,
            chunk_generator,
            setup.placement,
            air_pocket,
            surface_y,
            bounding_box.y_span(),
            &bounding_box,
            context.min_y,
            context.height,
        );
        let origin = BlockPos::new(base_position.x, projected_y, base_position.z);
        Some(GenerationStub {
            position: origin,
            generator: Box::new(move |builder, context| {
                let cold = setup.can_be_cold
                    && is_cold(
                        origin,
                        context.get_noise_biome(quart_pos::from_block(origin.x), quart_pos::from_block(origin.y), quart_pos::from_block(origin.z)),
                        context.chunk_generator.sea_level(),
                    );
                let properties = Properties {
                    cold,
                    mossiness: setup.mossiness,
                    air_pocket,
                    overgrown: setup.overgrown,
                    vines: setup.vines,
                    replace_with_blackstone: setup.replace_with_blackstone,
                };
                builder.add_piece(Piece::RuinedPortal(RuinedPortalPiece::new(origin, setup.placement, properties, template_location, rotation, mirror, pivot)));
            }),
        })
    }
}

fn sample(random: &mut impl RandomSource, limit: f32) -> bool {
    if limit == 0.0 {
        false
    } else if limit == 1.0 {
        true
    } else {
        random.next_float() < limit
    }
}

fn is_cold(pos: BlockPos, biome: crate::world::level::chunk::Biome, sea_level: i32) -> bool {
    biome.cold_enough_to_snow(pos, sea_level)
}

#[allow(clippy::too_many_arguments)]
fn find_suitable_y(
    random: &mut impl RandomSource,
    generator: &ChunkGenerator,
    vertical_placement: VerticalPlacement,
    air_pocket: bool,
    surface_y_at_center: i32,
    y_span: i32,
    bounding_box: &BoundingBox,
    level_min_y: i32,
    level_height: i32,
) -> i32 {
    let min_y = level_min_y + MIN_Y_INDEX;
    let new_y = match vertical_placement {
        VerticalPlacement::InNether => {
            if air_pocket {
                mth::random_between_inclusive(random, 32, 100)
            } else if random.next_float() < 0.5 {
                mth::random_between_inclusive(random, 27, 29)
            } else {
                mth::random_between_inclusive(random, 29, 100)
            }
        }
        VerticalPlacement::InMountain => get_random_within_interval(random, 70, surface_y_at_center - y_span),
        VerticalPlacement::Underground => get_random_within_interval(random, min_y, surface_y_at_center - y_span),
        VerticalPlacement::PartlyBuried => surface_y_at_center - y_span + mth::random_between_inclusive(random, 2, 8),
        _ => surface_y_at_center,
    };
    let bottom_corners = [
        (bounding_box.min.x, bounding_box.min.z),
        (bounding_box.max.x, bounding_box.min.z),
        (bounding_box.min.x, bounding_box.max.z),
        (bounding_box.max.x, bounding_box.max.z),
    ];
    let columns = bottom_corners.map(|(x, z)| generator.get_base_column(x, z, level_min_y, level_height));
    let heightmap = if vertical_placement == VerticalPlacement::OnOceanFloor { HeightmapType::OceanFloorWg } else { HeightmapType::WorldSurfaceWg };
    let mut projected_y = new_y;
    while projected_y > min_y {
        let mut corners_on_solid_ground = 0;
        for column in &columns {
            let block_state = column.get_block(projected_y);
            if heightmap.is_opaque(block_state) {
                corners_on_solid_ground += 1;
                if corners_on_solid_ground == 3 {
                    return projected_y;
                }
            }
        }
        projected_y -= 1;
    }
    projected_y
}

fn get_random_within_interval(random: &mut impl RandomSource, min_preferred: i32, max: i32) -> i32 {
    if min_preferred < max { mth::random_between_inclusive(random, min_preferred, max) } else { max }
}
