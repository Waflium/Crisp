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
}

pub mod templatesystem;
