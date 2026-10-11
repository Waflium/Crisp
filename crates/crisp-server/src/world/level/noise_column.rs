use crisp_data::{BlockState, blocks};

pub struct NoiseColumn {
    min_y: i32,
    column: Vec<BlockState>,
}

impl NoiseColumn {
    pub fn new(min_y: i32, column: Vec<BlockState>) -> NoiseColumn {
        NoiseColumn { min_y, column }
    }

    pub fn get_block(&self, pos: i32) -> BlockState {
        let index = pos - self.min_y;
        if index >= 0 && (index as usize) < self.column.len() { self.column[index as usize] } else { blocks::AIR.default_state() }
    }
}
