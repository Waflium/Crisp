use crisp_core::{BlockPos, Vec3, mth};

use super::path_type::PathType;

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    hash: i32,
    pub heap_idx: i32,
    pub g: f32,
    pub h: f32,
    pub f: f32,
    pub came_from: Option<u32>,
    pub closed: bool,
    pub walked_distance: f32,
    pub cost_malus: f32,
    pub r#type: PathType,
}

impl Node {
    pub fn new(x: i32, y: i32, z: i32) -> Node {
        Node {
            x,
            y,
            z,
            hash: Node::create_hash(x, y, z),
            heap_idx: -1,
            g: 0.0,
            h: 0.0,
            f: 0.0,
            came_from: None,
            closed: false,
            walked_distance: 0.0,
            cost_malus: 0.0,
            r#type: PathType::Blocked,
        }
    }

    pub fn clone_and_move(&self, x: i32, y: i32, z: i32) -> Node {
        Node { x, y, z, hash: Node::create_hash(x, y, z), ..*self }
    }

    pub fn create_hash(x: i32, y: i32, z: i32) -> i32 {
        y & 0xFF | (x & 32767) << 8 | (z & 32767) << 24 | if x < 0 { i32::MIN } else { 0 } | if z < 0 { 32768 } else { 0 }
    }

    pub fn hash_code(&self) -> i32 {
        self.hash
    }

    pub fn distance_to(&self, to: &Node) -> f32 {
        let xd = (to.x - self.x) as f32;
        let yd = (to.y - self.y) as f32;
        let zd = (to.z - self.z) as f32;
        mth::sqrt(xd * xd + yd * yd + zd * zd)
    }

    pub fn distance_to_xz(&self, to: &Node) -> f32 {
        let xd = (to.x - self.x) as f32;
        let zd = (to.z - self.z) as f32;
        mth::sqrt(xd * xd + zd * zd)
    }

    pub fn distance_to_sqr(&self, to: &Node) -> f32 {
        let xd = (to.x - self.x) as f32;
        let yd = (to.y - self.y) as f32;
        let zd = (to.z - self.z) as f32;
        xd * xd + yd * yd + zd * zd
    }

    pub fn distance_manhattan(&self, to: &Node) -> f32 {
        let xd = (to.x - self.x).abs() as f32;
        let yd = (to.y - self.y).abs() as f32;
        let zd = (to.z - self.z).abs() as f32;
        xd + yd + zd
    }

    pub fn distance_manhattan_pos(&self, pos: BlockPos) -> f32 {
        let xd = (pos.x - self.x).abs() as f32;
        let yd = (pos.y - self.y).abs() as f32;
        let zd = (pos.z - self.z).abs() as f32;
        xd + yd + zd
    }

    pub fn as_block_pos(&self) -> BlockPos {
        BlockPos::new(self.x, self.y, self.z)
    }

    pub fn as_vec3(&self) -> Vec3 {
        Vec3::new(self.x as f64, self.y as f64, self.z as f64)
    }

    pub fn in_open_set(&self) -> bool {
        self.heap_idx >= 0
    }
}

impl PartialEq for Node {
    fn eq(&self, o: &Node) -> bool {
        self.hash == o.hash && self.x == o.x && self.y == o.y && self.z == o.z
    }
}
