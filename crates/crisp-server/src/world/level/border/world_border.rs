use std::io;
use std::path::{Path, PathBuf};

use crisp_core::shapes::VoxelShape;
use crisp_core::{AABB, BlockPos, Vec3, mth};
use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use super::border_change_listener::BorderChangeListener;
use crate::server::level::ServerLevel;
use crate::world::entity::Entity;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::storage::{RespawnData, read_dat, write_dat};

pub const MAX_SIZE: f64 = 5.999997E7f32 as f64;
pub const MAX_CENTER_COORDINATE: f64 = 2.9999984E7;
const FAR_EDGE: f64 = 1.0e-5f32 as f64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub center_x: f64,
    pub center_z: f64,
    pub damage_per_block: f64,
    pub safe_zone: f64,
    pub warning_blocks: i32,
    pub warning_time: i32,
    pub size: f64,
    pub lerp_time: i64,
    pub lerp_target: f64,
}

impl Settings {
    pub const DEFAULT: Settings =
        Settings { center_x: 0.0, center_z: 0.0, damage_per_block: 0.2, safe_zone: 5.0, warning_blocks: 5, warning_time: 300, size: MAX_SIZE, lerp_time: 0, lerp_target: 0.0 };

    pub fn of(border: &WorldBorder) -> Settings {
        Settings {
            center_x: border.center_x,
            center_z: border.center_z,
            damage_per_block: border.damage_per_block,
            safe_zone: border.safe_zone,
            warning_blocks: border.warning_blocks,
            warning_time: border.warning_time,
            size: border.get_size(),
            lerp_time: border.get_lerp_time(),
            lerp_target: border.get_lerp_target(),
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("center_x".into(), Nbt::Double(self.center_x)),
            ("center_z".into(), Nbt::Double(self.center_z)),
            ("damage_per_block".into(), Nbt::Double(self.damage_per_block)),
            ("safe_zone".into(), Nbt::Double(self.safe_zone)),
            ("warning_blocks".into(), Nbt::Int(self.warning_blocks)),
            ("warning_time".into(), Nbt::Int(self.warning_time)),
            ("size".into(), Nbt::Double(self.size)),
            ("lerp_time".into(), Nbt::Long(self.lerp_time)),
            ("lerp_target".into(), Nbt::Double(self.lerp_target)),
        ])
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<Settings> {
        let number = |key: &str| nbt.get(key).filter(|v| v.as_f64().is_some());
        let double = |key: &str| number(key)?.as_f64();
        let long = |key: &str| number(key).map(|v| v.as_i64().unwrap_or_else(|| v.as_f64().unwrap() as i64));
        let int = |key: &str| number(key).map(|v| v.as_i64().map_or_else(|| v.as_f64().unwrap() as i32, |v| v as i32));
        let center = |key: &str| double(key).filter(|v| (-MAX_CENTER_COORDINATE..=MAX_CENTER_COORDINATE).contains(v));
        Some(Settings {
            center_x: center("center_x")?,
            center_z: center("center_z")?,
            damage_per_block: double("damage_per_block")?,
            safe_zone: double("safe_zone")?,
            warning_blocks: int("warning_blocks")?,
            warning_time: int("warning_time")?,
            size: double("size")?,
            lerp_time: long("lerp_time")?,
            lerp_target: double("lerp_target")?,
        })
    }
}

#[derive(Clone, Debug)]
struct MovingBorderExtent {
    from: f64,
    to: f64,
    lerp_duration: f64,
    lerp_progress: i64,
    size: f64,
    previous_size: f64,
}

impl MovingBorderExtent {
    fn new(from: f64, to: f64, duration: i64) -> MovingBorderExtent {
        let mut extent = MovingBorderExtent { from, to, lerp_duration: duration as f64, lerp_progress: duration, size: 0.0, previous_size: 0.0 };
        extent.size = extent.calculate_size();
        extent.previous_size = extent.size;
        extent
    }

    fn calculate_size(&self) -> f64 {
        let progress = (self.lerp_duration - self.lerp_progress as f64) / self.lerp_duration;
        if progress < 1.0 { mth::lerp(progress, self.from, self.to) } else { self.to }
    }
}

#[derive(Clone, Debug)]
enum BorderExtent {
    Static { size: f64 },
    Moving(MovingBorderExtent),
}

pub struct WorldBorder {
    dir: Option<PathBuf>,
    dirty: bool,
    pub listener: BorderChangeListener,
    damage_per_block: f64,
    safe_zone: f64,
    warning_time: i32,
    warning_blocks: i32,
    center_x: f64,
    center_z: f64,
    absolute_max_size: i32,
    extent: BorderExtent,
    min_x: f64,
    min_z: f64,
    max_x: f64,
    max_z: f64,
    shape: VoxelShape,
}

impl WorldBorder {
    pub fn new(settings: Settings, dir: Option<PathBuf>) -> WorldBorder {
        let mut border = WorldBorder {
            dir,
            dirty: false,
            listener: BorderChangeListener::default(),
            damage_per_block: 0.2,
            safe_zone: 5.0,
            warning_time: 15,
            warning_blocks: 5,
            center_x: 0.0,
            center_z: 0.0,
            absolute_max_size: 29_999_984,
            extent: BorderExtent::Static { size: MAX_SIZE },
            min_x: 0.0,
            min_z: 0.0,
            max_x: 0.0,
            max_z: 0.0,
            shape: VoxelShape::empty(),
        };
        border.update_box();
        border.set_center(settings.center_x, settings.center_z);
        border.set_damage_per_block(settings.damage_per_block);
        border.set_safe_zone(settings.safe_zone);
        border.set_warning_blocks(settings.warning_blocks);
        border.set_warning_time(settings.warning_time);
        if settings.lerp_time > 0 {
            border.lerp_size_between(settings.size, settings.lerp_target, settings.lerp_time);
        } else {
            border.set_size(settings.size);
        }
        border
    }

    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("world_border.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> WorldBorder {
        let settings = dir.as_deref().and_then(|dir| match read_dat(&WorldBorder::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").and_then(Settings::from_nbt)),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:world_border]");
                None
            }
        });
        WorldBorder::new(settings.unwrap_or(Settings::DEFAULT), dir)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let file = Nbt::Compound(vec![("data".into(), Settings::of(self).to_nbt()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&WorldBorder::file(dir), &file)
    }

    pub fn is_within_bounds(&self, x: f64, z: f64) -> bool {
        self.is_within_bounds_margin(x, z, 0.0)
    }

    pub fn is_within_bounds_margin(&self, x: f64, z: f64, margin: f64) -> bool {
        x >= self.min_x - margin && x < self.max_x + margin && z >= self.min_z - margin && z < self.max_z + margin
    }

    pub fn is_within_bounds_pos(&self, pos: BlockPos) -> bool {
        self.is_within_bounds(pos.x as f64, pos.z as f64)
    }

    pub fn is_within_bounds_aabb(&self, aabb: &AABB) -> bool {
        self.is_within_bounds(aabb.min_x, aabb.min_z) && self.is_within_bounds(aabb.max_x - FAR_EDGE, aabb.max_z - FAR_EDGE)
    }

    pub fn clamp_to_bounds(&self, x: f64, y: f64, z: f64) -> BlockPos {
        BlockPos::containing_vec(self.clamp_vec3_to_bound(x, y, z))
    }

    pub fn clamp_vec3_to_bound(&self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(mth::jclamp(x, self.min_x, self.max_x - FAR_EDGE), y, mth::jclamp(z, self.min_z, self.max_z - FAR_EDGE))
    }

    pub fn get_distance_to_border(&self, x: f64, z: f64) -> f64 {
        let from_north = z - self.min_z;
        let from_south = self.max_z - z;
        let from_west = x - self.min_x;
        let from_east = self.max_x - x;
        let min = mth::jmin(from_west, from_east);
        let min = mth::jmin(min, from_north);
        mth::jmin(min, from_south)
    }

    pub fn get_collision_shape(&self) -> &VoxelShape {
        &self.shape
    }

    pub fn is_inside_close_to_border(&self, source: &Entity, bounding_box: &AABB) -> bool {
        let x_size = bounding_box.max_x - bounding_box.min_x;
        let z_size = bounding_box.max_z - bounding_box.min_z;
        let bb_max = mth::jmax(mth::jmax(x_size.abs(), z_size.abs()), 1.0);
        self.get_distance_to_border(source.x(), source.z()) < bb_max * 2.0 && self.is_within_bounds_margin(source.x(), source.z(), bb_max)
    }

    pub fn get_min_x(&self) -> f64 {
        self.min_x
    }

    pub fn get_min_z(&self) -> f64 {
        self.min_z
    }

    pub fn get_max_x(&self) -> f64 {
        self.max_x
    }

    pub fn get_max_z(&self) -> f64 {
        self.max_z
    }

    pub fn get_center_x(&self) -> f64 {
        self.center_x
    }

    pub fn get_center_z(&self) -> f64 {
        self.center_z
    }

    pub fn set_center(&mut self, x: f64, z: f64) {
        self.center_x = x;
        self.center_z = z;
        self.update_box();
        self.dirty = true;
        self.listener.on_set_center(self);
    }

    pub fn get_size(&self) -> f64 {
        match &self.extent {
            BorderExtent::Static { size } => *size,
            BorderExtent::Moving(e) => e.size,
        }
    }

    pub fn get_lerp_time(&self) -> i64 {
        match &self.extent {
            BorderExtent::Static { .. } => 0,
            BorderExtent::Moving(e) => e.lerp_progress,
        }
    }

    pub fn get_lerp_target(&self) -> f64 {
        match &self.extent {
            BorderExtent::Static { size } => *size,
            BorderExtent::Moving(e) => e.to,
        }
    }

    pub fn set_size(&mut self, size: f64) {
        self.extent = BorderExtent::Static { size };
        self.update_box();
        self.dirty = true;
        self.listener.on_set_size(self);
    }

    pub fn lerp_size_between(&mut self, from: f64, to: f64, ticks: i64) {
        self.extent = if from == to { BorderExtent::Static { size: to } } else { BorderExtent::Moving(MovingBorderExtent::new(from, to, ticks)) };
        self.update_box();
        self.dirty = true;
        self.listener.on_lerp_size(self);
    }

    pub fn set_absolute_max_size(&mut self, absolute_max_size: i32) {
        self.absolute_max_size = absolute_max_size;
        self.update_box();
    }

    pub fn get_absolute_max_size(&self) -> i32 {
        self.absolute_max_size
    }

    pub fn get_safe_zone(&self) -> f64 {
        self.safe_zone
    }

    pub fn set_safe_zone(&mut self, safe_zone: f64) {
        self.safe_zone = safe_zone;
        self.dirty = true;
    }

    pub fn get_damage_per_block(&self) -> f64 {
        self.damage_per_block
    }

    pub fn set_damage_per_block(&mut self, damage_per_block: f64) {
        self.damage_per_block = damage_per_block;
        self.dirty = true;
    }

    pub fn get_warning_time(&self) -> i32 {
        self.warning_time
    }

    pub fn set_warning_time(&mut self, warning_time: i32) {
        self.warning_time = warning_time;
        self.dirty = true;
        self.listener.on_set_warning_time(self);
    }

    pub fn get_warning_blocks(&self) -> i32 {
        self.warning_blocks
    }

    pub fn set_warning_blocks(&mut self, warning_blocks: i32) {
        self.warning_blocks = warning_blocks;
        self.dirty = true;
        self.listener.on_set_warning_blocks(self);
    }

    pub fn tick(&mut self) {
        let BorderExtent::Moving(e) = &mut self.extent else { return };
        e.lerp_progress -= 1;
        e.previous_size = e.size;
        e.size = e.calculate_size();
        self.dirty = true;
        if e.lerp_progress <= 0 {
            self.extent = BorderExtent::Static { size: e.to };
        }
        self.update_box();
    }

    fn update_box(&mut self) {
        let size = match &self.extent {
            BorderExtent::Static { size } => *size,
            BorderExtent::Moving(e) => mth::lerp(0.0, e.previous_size, e.size),
        };
        let max = self.absolute_max_size as f64;
        self.min_x = mth::jclamp(self.center_x - size / 2.0, -max, max);
        self.min_z = mth::jclamp(self.center_z - size / 2.0, -max, max);
        self.max_x = mth::jclamp(self.center_x + size / 2.0, -max, max);
        self.max_z = mth::jclamp(self.center_z + size / 2.0, -max, max);
        self.shape = collision_shape(self.min_x.floor(), self.min_z.floor(), self.max_x.ceil(), self.max_z.ceil());
    }
}

impl ServerLevel {
    pub fn get_world_border_adjusted_respawn_data(&mut self, respawn_data: RespawnData) -> RespawnData {
        let border = &self.world_border;
        if border.is_within_bounds_pos(respawn_data.pos) {
            return respawn_data;
        }
        let center = BlockPos::containing(border.get_center_x(), 0.0, border.get_center_z());
        let pos = self.heightmap_pos(HeightmapType::MotionBlocking, center);
        RespawnData::of(&respawn_data.dimension, pos, respawn_data.yaw, respawn_data.pitch)
    }
}

fn collision_shape(min_x: f64, min_z: f64, max_x: f64, max_z: f64) -> VoxelShape {
    let all = || vec![f64::NEG_INFINITY, f64::INFINITY];
    if max_x - min_x < 1.0e-7 || max_z - min_z < 1.0e-7 {
        return VoxelShape::from_cells([all(), all(), all()], vec![true]);
    }
    let coords = |min: f64, max: f64| vec![f64::NEG_INFINITY, min, max, f64::INFINITY];
    let filled = (0..9).map(|i| i != 4).collect();
    VoxelShape::from_cells([coords(min_x, max_x), all(), coords(min_z, max_z)], filled)
}
