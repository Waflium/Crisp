use super::sampler::Sampler;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DensityVolume {
    pub size_x: i32,
    pub size_y: i32,
    pub size_z: i32,
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub step_x: i32,
    pub step_y: i32,
    pub step_z: i32,
}

impl DensityVolume {
    pub fn new(size_x: i32, size_y: i32, size_z: i32, min_x: i32, min_y: i32, min_z: i32) -> DensityVolume {
        DensityVolume::stepped(size_x, size_y, size_z, min_x, min_y, min_z, 1, 1, 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn stepped(size_x: i32, size_y: i32, size_z: i32, min_x: i32, min_y: i32, min_z: i32, step_x: i32, step_y: i32, step_z: i32) -> DensityVolume {
        assert!(size_x > 0 && size_y > 0 && size_z > 0, "Size must be positive, was: {size_x}x{size_y}x{size_z}");
        assert!(step_x > 0 && step_y > 0 && step_z > 0, "Step must be positive");
        DensityVolume { size_x, size_y, size_z, min_x, min_y, min_z, step_x, step_y, step_z }
    }

    #[inline(always)]
    pub fn index(&self, x: i32, y: i32, z: i32) -> usize {
        (y + (x + z * self.size_x) * self.size_y) as usize
    }

    #[inline(always)]
    pub fn block_x(&self, x: i32) -> i32 {
        self.min_x + x * self.step_x
    }

    #[inline(always)]
    pub fn block_y(&self, y: i32) -> i32 {
        self.min_y + y * self.step_y
    }

    #[inline(always)]
    pub fn block_z(&self, z: i32) -> i32 {
        self.min_z + z * self.step_z
    }

    pub fn max_x(&self) -> i32 {
        self.min_x + self.size_x * self.step_x - 1
    }

    pub fn max_y(&self) -> i32 {
        self.min_y + self.size_y * self.step_y - 1
    }

    pub fn max_z(&self) -> i32 {
        self.min_z + self.size_z * self.step_z - 1
    }

    pub fn size(&self) -> usize {
        (self.size_x * self.size_y * self.size_z) as usize
    }

    pub fn index_of_block(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (rx, ry, rz) = (x - self.min_x, y - self.min_y, z - self.min_z);
        if self.step_x == 1 && self.step_y == 1 && self.step_z == 1 {
            if rx >= 0 && ry >= 0 && rz >= 0 && rx < self.size_x && ry < self.size_y && rz < self.size_z {
                return Some(self.index(rx, ry, rz));
            }
        } else if rx >= 0
            && ry >= 0
            && rz >= 0
            && rx < self.size_x * self.step_x
            && ry < self.size_y * self.step_y
            && rz < self.size_z * self.step_z
            && rx.rem_euclid(self.step_x) == 0
            && ry.rem_euclid(self.step_y) == 0
            && rz.rem_euclid(self.step_z) == 0
        {
            return Some(self.index(rx.div_euclid(self.step_x), ry.div_euclid(self.step_y), rz.div_euclid(self.step_z)));
        }
        None
    }
}

struct CacheCell {
    volume: Option<DensityVolume>,
    buffer: Option<Vec<f32>>,
    value_key: i64,
    value: f32,
}

impl Default for CacheCell {
    fn default() -> CacheCell {
        CacheCell { volume: None, buffer: None, value_key: 0, value: f32::NAN }
    }
}

pub struct SamplerContext {
    cells: Option<Vec<CacheCell>>,
    pool: Vec<Vec<f32>>,
    pub beardifier: Option<std::sync::Arc<crate::world::level::levelgen::beardifier::Beardifier>>,
}

fn block_key(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FF_FFFF) << 38) | (y as i64 & 0xFFF) | ((z as i64 & 0x3FF_FFFF) << 12)
}

impl SamplerContext {
    pub fn cached() -> SamplerContext {
        SamplerContext { cells: Some(Vec::new()), pool: Vec::new(), beardifier: None }
    }

    pub fn uncached() -> SamplerContext {
        SamplerContext { cells: None, pool: Vec::new(), beardifier: None }
    }

    pub fn acquire(&mut self, size: usize) -> Vec<f32> {
        let mut buffer = self.pool.pop().unwrap_or_default();
        if buffer.len() >= size {
            buffer.truncate(size);
        } else {
            buffer.resize(size, 0.0);
        }
        buffer
    }

    pub fn release(&mut self, buffer: Vec<f32>) {
        self.pool.push(buffer);
    }

    fn cell(&mut self, id: usize) -> Option<&mut CacheCell> {
        let cells = self.cells.as_mut()?;
        if id >= cells.len() {
            cells.resize_with(id + 1, CacheCell::default);
        }
        Some(&mut cells[id])
    }

    pub(super) fn sample_volume_cached(&mut self, id: usize, input: &Sampler, out: &mut [f32], volume: &DensityVolume) {
        let Some(cell) = self.cell(id) else {
            return input.sample_volume(self, out, volume);
        };
        if cell.buffer.is_none() || cell.volume != Some(*volume) {
            let old = cell.buffer.take();
            cell.volume = Some(*volume);
            if let Some(old) = old {
                self.release(old);
            }
            let mut buffer = self.acquire(volume.size());
            input.sample_volume(self, &mut buffer, volume);
            self.cell(id).unwrap().buffer = Some(buffer);
        }
        out.copy_from_slice(self.cell(id).unwrap().buffer.as_ref().unwrap());
    }

    pub(super) fn sample_value_cached(&mut self, id: usize, input: &Sampler, x: i32, y: i32, z: i32) -> f32 {
        let Some(cell) = self.cell(id) else {
            return input.sample_value(self, x, y, z);
        };
        let key = block_key(x, y, z);
        if cell.value_key == key && !cell.value.is_nan() {
            return cell.value;
        }
        if let (Some(buffer), Some(volume)) = (&cell.buffer, &cell.volume)
            && let Some(index) = volume.index_of_block(x, y, z)
        {
            return buffer[index];
        }
        let value = input.sample_value(self, x, y, z);
        let cell = self.cell(id).unwrap();
        cell.value_key = key;
        cell.value = value;
        value
    }
}
