use crisp_core::bit_storage::SimpleBitStorage;
use crisp_core::mth;
use crisp_data::{BlockState, block_tags, blocks};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeightmapType {
    WorldSurfaceWg,
    WorldSurface,
    OceanFloorWg,
    OceanFloor,
    MotionBlocking,
    MotionBlockingNoLeaves,
}

impl HeightmapType {
    pub const LIVE: [HeightmapType; 4] =
        [HeightmapType::WorldSurface, HeightmapType::OceanFloor, HeightmapType::MotionBlocking, HeightmapType::MotionBlockingNoLeaves];
    pub const WORLDGEN: [HeightmapType; 2] = [HeightmapType::OceanFloorWg, HeightmapType::WorldSurfaceWg];
    pub const VALUES: [HeightmapType; 6] = [
        HeightmapType::WorldSurfaceWg,
        HeightmapType::WorldSurface,
        HeightmapType::OceanFloorWg,
        HeightmapType::OceanFloor,
        HeightmapType::MotionBlocking,
        HeightmapType::MotionBlockingNoLeaves,
    ];

    pub fn id(self) -> i32 {
        match self {
            HeightmapType::WorldSurfaceWg => 0,
            HeightmapType::WorldSurface => 1,
            HeightmapType::OceanFloorWg => 2,
            HeightmapType::OceanFloor => 3,
            HeightmapType::MotionBlocking => 4,
            HeightmapType::MotionBlockingNoLeaves => 5,
        }
    }

    pub fn serialization_key(self) -> &'static str {
        match self {
            HeightmapType::WorldSurfaceWg => "WORLD_SURFACE_WG",
            HeightmapType::WorldSurface => "WORLD_SURFACE",
            HeightmapType::OceanFloorWg => "OCEAN_FLOOR_WG",
            HeightmapType::OceanFloor => "OCEAN_FLOOR",
            HeightmapType::MotionBlocking => "MOTION_BLOCKING",
            HeightmapType::MotionBlockingNoLeaves => "MOTION_BLOCKING_NO_LEAVES",
        }
    }

    pub fn send_to_client(self) -> bool {
        matches!(self, HeightmapType::WorldSurface | HeightmapType::MotionBlocking | HeightmapType::MotionBlockingNoLeaves)
    }

    pub fn is_opaque(self, state: BlockState) -> bool {
        match self {
            HeightmapType::WorldSurfaceWg | HeightmapType::WorldSurface => !state.is_air(),
            HeightmapType::OceanFloorWg | HeightmapType::OceanFloor => state.is(&block_tags::BLOCKS_MOTION_IN_HEIGHTMAP),
            HeightmapType::MotionBlocking => state.is(&block_tags::BLOCKS_MOTION_IN_HEIGHTMAP) || !state.fluid_state().is_empty(),
            HeightmapType::MotionBlockingNoLeaves => {
                state.is(&block_tags::BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES) || !state.fluid_state().is_empty()
            }
        }
    }
}

#[derive(Clone)]
pub struct Heightmap {
    pub kind: HeightmapType,
    data: SimpleBitStorage,
    min_y: i32,
}

impl Heightmap {
    pub fn new(kind: HeightmapType, min_y: i32, height: i32) -> Heightmap {
        Heightmap { kind, data: SimpleBitStorage::new(mth::ceillog2(height + 1) as u32, 256), min_y }
    }

    fn index(x: usize, z: usize) -> usize {
        x + z * 16
    }

    pub fn first_available(&self, x: usize, z: usize) -> i32 {
        self.data.get(Self::index(x, z)) as i32 + self.min_y
    }

    pub fn highest_taken(&self, x: usize, z: usize) -> i32 {
        self.first_available(x, z) - 1
    }

    fn set_height(&mut self, x: usize, z: usize, height: i32) {
        self.data.set(Self::index(x, z), (height - self.min_y) as u32);
    }

    pub fn update(&mut self, x: usize, y: i32, z: usize, state: BlockState, column: impl Fn(i32) -> BlockState) -> bool {
        let first = self.first_available(x, z);
        if y <= first - 2 {
            return false;
        }
        if self.kind.is_opaque(state) {
            if y >= first {
                self.set_height(x, z, y + 1);
                return true;
            }
        } else if first - 1 == y {
            for below in (self.min_y..y).rev() {
                if self.kind.is_opaque(column(below)) {
                    self.set_height(x, z, below + 1);
                    return true;
                }
            }
            self.set_height(x, z, self.min_y);
            return true;
        }
        false
    }

    pub fn raw(&self) -> &[i64] {
        self.data.raw()
    }

    pub fn set_raw(&mut self, data: &[i64]) -> bool {
        if data.len() != self.data.raw().len() {
            return false;
        }
        self.data.raw_mut().copy_from_slice(data);
        true
    }

    pub fn prime(heightmaps: &mut [&mut Heightmap], min_y: i32, top: i32, block: impl Fn(usize, i32, usize) -> BlockState) {
        for x in 0..16 {
            for z in 0..16 {
                let mut remaining: Vec<usize> = (0..heightmaps.len()).collect();
                for y in (min_y..top).rev() {
                    let state = block(x, y, z);
                    if state.is_block(blocks::AIR) {
                        continue;
                    }
                    remaining.retain(|&i| {
                        if heightmaps[i].kind.is_opaque(state) {
                            heightmaps[i].set_height(x, z, y + 1);
                            false
                        } else {
                            true
                        }
                    });
                    if remaining.is_empty() {
                        break;
                    }
                }
            }
        }
    }
}
