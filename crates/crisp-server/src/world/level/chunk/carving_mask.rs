pub struct CarvingMask {
    min_y: i32,
    max_y: i32,
    height: i32,
    mask: Vec<u64>,
}

impl CarvingMask {
    pub fn new(min_y: i32, max_y: i32) -> CarvingMask {
        let height = max_y - min_y + 1;
        CarvingMask { min_y, max_y, height, mask: vec![0; (256 * height as usize).div_ceil(64)] }
    }

    fn index(&self, x: i32, y: i32, z: i32) -> usize {
        (y - self.min_y + (z + (x << 4)) * self.height) as usize
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    pub fn max_y(&self) -> i32 {
        self.max_y
    }

    pub fn carve(&mut self, x: i32, y: i32, z: i32) {
        let i = self.index(x, y, z);
        self.mask[i / 64] |= 1 << (i % 64);
    }

    pub fn is_empty(&self) -> bool {
        self.mask.iter().all(|&w| w == 0)
    }

    fn next_bit(&self, from: usize, clear: bool) -> Option<usize> {
        let mut word_index = from / 64;
        if word_index >= self.mask.len() {
            return if clear { Some(from) } else { None };
        }
        let flip = if clear { !0 } else { 0 };
        let mut word = (self.mask[word_index] ^ flip) & (!0u64 << (from % 64));
        loop {
            if word != 0 {
                return Some(word_index * 64 + word.trailing_zeros() as usize);
            }
            word_index += 1;
            if word_index == self.mask.len() {
                return if clear { Some(word_index * 64) } else { None };
            }
            word = self.mask[word_index] ^ flip;
        }
    }

    pub fn visit(&self, mut visitor: impl FnMut(i32, i32, i32, i32)) {
        let mut start = self.next_bit(0, false);
        while let Some(start_index) = start {
            let end_index = self.next_bit(start_index, true).unwrap() - 1;
            let height = self.height as usize;
            for column in start_index / height..=end_index / height {
                let base = column * height;
                let bottom = start_index.saturating_sub(base) as i32 + self.min_y;
                let top = (end_index - base).min(height - 1) as i32 + self.min_y;
                visitor((column >> 4 & 15) as i32, (column & 15) as i32, bottom, top);
            }
            start = self.next_bit(end_index + 1, false);
        }
    }
}
