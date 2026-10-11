use crisp_core::BlockPos;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BoundingBox {
    pub min: BlockPos,
    pub max: BlockPos,
}

impl BoundingBox {
    pub fn new(min_x: i32, min_y: i32, min_z: i32, max_x: i32, max_y: i32, max_z: i32) -> BoundingBox {
        BoundingBox::from_corners(BlockPos::new(min_x, min_y, min_z), BlockPos::new(max_x, max_y, max_z))
    }

    pub fn move_by(mut self, amount: BlockPos) -> BoundingBox {
        self.min = self.min.offset(amount.x, amount.y, amount.z);
        self.max = self.max.offset(amount.x, amount.y, amount.z);
        self
    }

    pub fn for_all_corners(&self, mut consumer: impl FnMut(BlockPos)) {
        let (min, max) = (self.min, self.max);
        consumer(BlockPos::new(max.x, max.y, max.z));
        consumer(BlockPos::new(min.x, max.y, max.z));
        consumer(BlockPos::new(max.x, min.y, max.z));
        consumer(BlockPos::new(min.x, min.y, max.z));
        consumer(BlockPos::new(max.x, max.y, min.z));
        consumer(BlockPos::new(min.x, max.y, min.z));
        consumer(BlockPos::new(max.x, min.y, min.z));
        consumer(BlockPos::new(min.x, min.y, min.z));
    }

    pub fn from_corners(a: BlockPos, b: BlockPos) -> BoundingBox {
        BoundingBox { min: BlockPos::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)), max: BlockPos::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)) }
    }

    pub fn x_span(&self) -> i32 {
        self.max.x - self.min.x + 1
    }

    pub fn y_span(&self) -> i32 {
        self.max.y - self.min.y + 1
    }

    pub fn z_span(&self) -> i32 {
        self.max.z - self.min.z + 1
    }

    pub fn length(&self) -> BlockPos {
        BlockPos::new(self.max.x - self.min.x, self.max.y - self.min.y, self.max.z - self.min.z)
    }

    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.max.x >= other.min.x
            && self.min.x <= other.max.x
            && self.max.z >= other.min.z
            && self.min.z <= other.max.z
            && self.max.y >= other.min.y
            && self.min.y <= other.max.y
    }

    pub fn is_inside(&self, pos: BlockPos) -> bool {
        pos.x >= self.min.x && pos.x <= self.max.x && pos.z >= self.min.z && pos.z <= self.max.z && pos.y >= self.min.y && pos.y <= self.max.y
    }

    pub fn positions(&self) -> impl Iterator<Item = BlockPos> + use<> {
        let (min, max) = (self.min, self.max);
        (min.z..=max.z).flat_map(move |z| (min.y..=max.y).flat_map(move |y| (min.x..=max.x).map(move |x| BlockPos::new(x, y, z))))
    }

    pub fn of(content: BlockPos) -> BoundingBox {
        BoundingBox { min: content, max: content }
    }

    pub fn intersects_xz(&self, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> bool {
        self.max.x >= min_x && self.min.x <= max_x && self.max.z >= min_z && self.min.z <= max_z
    }

    pub fn encapsulating(a: &BoundingBox, b: &BoundingBox) -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y), a.min.z.min(b.min.z)),
            max: BlockPos::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y), a.max.z.max(b.max.z)),
        }
    }

    pub fn encapsulating_boxes<'a>(mut boxes: impl Iterator<Item = &'a BoundingBox>) -> Option<BoundingBox> {
        let first = *boxes.next()?;
        Some(boxes.fold(first, |result, b| BoundingBox::encapsulating(&result, b)))
    }

    pub fn inflated_by(&self, amount: i32) -> BoundingBox {
        BoundingBox::new(self.min.x - amount, self.min.y - amount, self.min.z - amount, self.max.x + amount, self.max.y + amount, self.max.z + amount)
    }

    pub fn move_in_place(&mut self, dx: i32, dy: i32, dz: i32) {
        self.min = self.min.offset(dx, dy, dz);
        self.max = self.max.offset(dx, dy, dz);
    }

    pub fn get_center(&self) -> BlockPos {
        BlockPos::new(
            self.min.x + (self.max.x - self.min.x + 1) / 2,
            self.min.y + (self.max.y - self.min.y + 1) / 2,
            self.min.z + (self.max.z - self.min.z + 1) / 2,
        )
    }

    pub fn to_nbt(&self) -> crisp_protocol::nbt::Nbt {
        crisp_protocol::nbt::Nbt::IntArray(vec![self.min.x, self.min.y, self.min.z, self.max.x, self.max.y, self.max.z])
    }

    pub fn from_nbt(tag: &crisp_protocol::nbt::Nbt) -> Option<BoundingBox> {
        match tag {
            crisp_protocol::nbt::Nbt::IntArray(v) if v.len() == 6 => Some(BoundingBox::new(v[0], v[1], v[2], v[3], v[4], v[5])),
            _ => None,
        }
    }
}

pub mod pieces;
pub mod placement;
pub mod pool_element_structure_piece;
pub mod pools;
pub mod scattered_feature_piece;
pub mod single_piece_structure;
pub mod structure;
pub mod structure_piece;
pub mod structure_set;
pub mod structure_start;
pub mod structures;
pub mod template_structure_piece;
pub mod templatesystem;
#[cfg(test)]
mod tests;
pub mod terrain_adjustment;
