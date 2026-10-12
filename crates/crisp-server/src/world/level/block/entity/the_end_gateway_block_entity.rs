use crisp_core::random::LegacyRandomSource;
use crisp_core::{BlockPos, ChunkPos, Vec3, mth};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::THE_END;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::feature::end_gateway_feature::EndGatewayFeature;
use crate::world::level::levelgen::feature::feature;

pub const SPAWN_TIME: i64 = 200;
pub const COOLDOWN_TIME: i32 = 40;
pub const ATTENTION_INTERVAL: i64 = 2400;
const DEFAULT_AGE: i64 = 0;
const DEFAULT_EXACT_TELEPORT: bool = false;

#[derive(Clone, Default)]
pub struct TheEndGatewayBlockEntity {
    pub age: i64,
    pub teleport_cooldown: i32,
    pub exit_portal: Option<BlockPos>,
    pub exact_teleport: bool,
}

impl TheEndGatewayBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Age".into(), Nbt::Long(self.age)));
        if let Some(exit_portal) = self.exit_portal {
            output.push(("exit_portal".into(), Nbt::IntArray(vec![exit_portal.x, exit_portal.y, exit_portal.z])));
        }
        if self.exact_teleport {
            output.push(("ExactTeleport".into(), Nbt::Byte(1)));
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.age = input.get("Age").and_then(super::long_value).unwrap_or(DEFAULT_AGE);
        self.exit_portal = match input.get("exit_portal") {
            Some(Nbt::IntArray(v)) if v.len() == 3 => Some(BlockPos::new(v[0], v[1], v[2])).filter(|&pos| is_in_spawnable_bounds(pos)),
            _ => None,
        };
        self.exact_teleport = super::int_or(input, "ExactTeleport", DEFAULT_EXACT_TELEPORT as i32) != 0;
    }

    pub fn is_spawning(&self) -> bool {
        self.age < SPAWN_TIME
    }

    pub fn is_cooling_down(&self) -> bool {
        self.teleport_cooldown > 0
    }

    pub fn set_exit_position(&mut self, exit_portal: BlockPos, exact_teleport: bool) {
        self.exact_teleport = exact_teleport;
        self.exit_portal = Some(exit_portal);
    }
}

impl ServerLevel {
    fn end_gateway(&mut self, pos: BlockPos) -> Option<&mut TheEndGatewayBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::EndGateway(gateway) => Some(gateway),
            _ => None,
        }
    }

    pub fn end_gateway_is_cooling_down(&mut self, pos: BlockPos) -> Option<bool> {
        self.end_gateway(pos).map(|gateway| gateway.is_cooling_down())
    }

    pub fn end_gateway_portal_tick(&mut self, pos: BlockPos, state: BlockState) {
        let Some(entity) = self.end_gateway(pos) else { return };
        let spawning = entity.is_spawning();
        let cooling_down = entity.is_cooling_down();
        entity.age += 1;
        if cooling_down {
            entity.teleport_cooldown -= 1;
        } else if entity.age % ATTENTION_INTERVAL == 0 {
            self.end_gateway_trigger_cooldown(pos, state);
        }
        let Some(entity) = self.end_gateway(pos) else { return };
        if spawning != entity.is_spawning() || cooling_down != entity.is_cooling_down() {
            self.block_entity_set_changed_static(pos, state);
        }
    }

    pub fn end_gateway_trigger_cooldown(&mut self, pos: BlockPos, block_state: BlockState) {
        let Some(entity) = self.end_gateway(pos) else { return };
        entity.teleport_cooldown = COOLDOWN_TIME;
        self.block_event(pos, block_state.block(), 1, 0);
        self.block_entity_set_changed_static(pos, block_state);
    }

    pub fn end_gateway_get_portal_position(&mut self, portal_entry_pos: BlockPos) -> Option<Vec3> {
        let (exit_portal, exact_teleport) = {
            let entity = self.end_gateway(portal_entry_pos)?;
            (entity.exit_portal, entity.exact_teleport)
        };
        let exit_portal = match exit_portal {
            None if self.dimension == THE_END => {
                let exit_portal_pos = find_or_create_valid_teleport_pos(self, portal_entry_pos).offset(0, 10, 0);
                EndGatewayFeature::known_exit(portal_entry_pos, false).place(self, exit_portal_pos);
                let entity = self.end_gateway(portal_entry_pos)?;
                entity.set_exit_position(exit_portal_pos, exact_teleport);
                self.block_entity_set_changed(portal_entry_pos);
                Some(exit_portal_pos)
            }
            other => other,
        };
        let exit_portal = exit_portal?;
        let pos = if exact_teleport { exit_portal } else { find_exit_position(self, exit_portal) };
        Some(Vec3::at_bottom_center_of(pos))
    }
}

fn find_exit_position(level: &mut ServerLevel, exit_portal: BlockPos) -> BlockPos {
    find_tallest_block(level, exit_portal.offset(0, 2, 0), 5, false).above()
}

fn find_or_create_valid_teleport_pos(level: &mut ServerLevel, end_gateway_pos: BlockPos) -> BlockPos {
    let exit_portal_xz_pos_tentative = find_exit_portal_xz_pos_tentative(level, end_gateway_pos);
    let exit_portal_chunk = chunk_pos(exit_portal_xz_pos_tentative);
    let exit_portal_pos = match find_valid_spawn_in_chunk(level, exit_portal_chunk) {
        Some(pos) => pos,
        None => {
            let new_exit_portal_pos = BlockPos::containing(exit_portal_xz_pos_tentative.x + 0.5, 75.0, exit_portal_xz_pos_tentative.z + 0.5);
            if let Some(end_island) = feature::feature("minecraft:end_island") {
                end_island.place(level, &mut LegacyRandomSource::new(new_exit_portal_pos.as_long()), new_exit_portal_pos);
            }
            new_exit_portal_pos
        }
    };
    find_tallest_block(level, exit_portal_pos, 16, true)
}

fn find_exit_portal_xz_pos_tentative(level: &mut ServerLevel, end_gateway_pos: BlockPos) -> Vec3 {
    let teleport_xz_direction_vector = Vec3::new(end_gateway_pos.x as f64, 0.0, end_gateway_pos.z as f64).normalize();
    let mut exit_portal_xz_pos_tentative = teleport_xz_direction_vector.scale(1024.0);
    let mut chunk_limit = 16;
    while !is_chunk_empty(level, exit_portal_xz_pos_tentative) && {
        let more = chunk_limit > 0;
        chunk_limit -= 1;
        more
    } {
        exit_portal_xz_pos_tentative = exit_portal_xz_pos_tentative.add_vec(teleport_xz_direction_vector.scale(-16.0));
    }
    let mut chunk_limit = 16;
    while is_chunk_empty(level, exit_portal_xz_pos_tentative) && {
        let more = chunk_limit > 0;
        chunk_limit -= 1;
        more
    } {
        exit_portal_xz_pos_tentative = exit_portal_xz_pos_tentative.add_vec(teleport_xz_direction_vector.scale(16.0));
    }
    exit_portal_xz_pos_tentative
}

fn chunk_pos(pos: Vec3) -> ChunkPos {
    ChunkPos::new(mth::floor(pos.x / 16.0), mth::floor(pos.z / 16.0))
}

fn is_chunk_empty(level: &mut ServerLevel, xz_pos: Vec3) -> bool {
    level.chunks.chunk(chunk_pos(xz_pos)).sections.iter().all(|section| section.has_only_air())
}

fn find_tallest_block(level: &mut ServerLevel, around: BlockPos, dist: i32, allow_bedrock: bool) -> BlockPos {
    let mut tallest: Option<BlockPos> = None;
    let max_y = level.chunks.max_y();
    let min_y = level.chunks.min_y();
    for xd in -dist..=dist {
        for zd in -dist..=dist {
            if xd != 0 || zd != 0 || allow_bedrock {
                let mut y = max_y;
                while y > tallest.map_or(min_y, |t| t.y) {
                    let pos = BlockPos::new(around.x + xd, y, around.z + zd);
                    let state = level.block_state(pos);
                    if state.is_collision_shape_full_block() && (allow_bedrock || !state.is_block(blocks::BEDROCK)) {
                        tallest = Some(pos);
                        break;
                    }
                    y -= 1;
                }
            }
        }
    }
    tallest.unwrap_or(around)
}

fn find_valid_spawn_in_chunk(level: &mut ServerLevel, chunk_pos: ChunkPos) -> Option<BlockPos> {
    let chunk = level.chunks.chunk(chunk_pos);
    let start = BlockPos::new(chunk_pos.min_block_x(), 30, chunk_pos.min_block_z());
    let max_y = chunk.highest_section_position() + 16 - 1;
    let end = BlockPos::new(chunk_pos.min_block_x() + 15, max_y, chunk_pos.min_block_z() + 15);
    let mut closest: Option<f64> = None;
    for pos in BlockPos::between_closed(start, end) {
        let state = chunk.block_state(pos);
        if state.is_block(blocks::END_STONE) && !chunk.block_state(pos.above()).is_collision_shape_full_block() && !chunk.block_state(pos.offset(0, 2, 0)).is_collision_shape_full_block() {
            let dist = pos.dist_to_center_sqr(0.0, 0.0, 0.0);
            if closest.is_none_or(|closest_dist| dist < closest_dist) {
                closest = Some(dist);
            }
        }
    }
    closest.map(|_| end)
}

fn is_in_spawnable_bounds(pos: BlockPos) -> bool {
    (-20_000_000..20_000_000).contains(&pos.y) && (-30_000_000..30_000_000).contains(&pos.x) && (-30_000_000..30_000_000).contains(&pos.z)
}
