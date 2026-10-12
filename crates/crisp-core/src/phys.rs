use crate::mth::{self, jmax, jmin};
use crate::random::RandomSource;
use crate::{Axis, BlockPos, Direction};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    pub fn at_lower_corner_of(pos: BlockPos) -> Vec3 {
        Vec3::new(pos.x as f64, pos.y as f64, pos.z as f64)
    }

    pub fn at_lower_corner_with_offset(pos: BlockPos, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(pos.x as f64 + x, pos.y as f64 + y, pos.z as f64 + z)
    }

    pub fn at_center_of(pos: BlockPos) -> Vec3 {
        Vec3::at_lower_corner_with_offset(pos, 0.5, 0.5, 0.5)
    }

    pub fn at_bottom_center_of(pos: BlockPos) -> Vec3 {
        Vec3::at_lower_corner_with_offset(pos, 0.5, 0.0, 0.5)
    }

    pub fn up_from_bottom_center_of(pos: BlockPos, y_offset: f64) -> Vec3 {
        Vec3::at_lower_corner_with_offset(pos, 0.5, y_offset, 0.5)
    }

    pub fn vector_to(self, v: Vec3) -> Vec3 {
        Vec3::new(v.x - self.x, v.y - self.y, v.z - self.z)
    }

    pub fn normalize(self) -> Vec3 {
        let dist = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if dist < 1.0e-5f32 as f64 { Vec3::ZERO } else { Vec3::new(self.x / dist, self.y / dist, self.z / dist) }
    }

    pub fn dot(self, v: Vec3) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    pub fn cross(self, v: Vec3) -> Vec3 {
        Vec3::new(self.y * v.z - self.z * v.y, self.z * v.x - self.x * v.z, self.x * v.y - self.y * v.x)
    }

    pub fn subtract(self, v: Vec3) -> Vec3 {
        self.add(-v.x, -v.y, -v.z)
    }

    pub fn add(self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(self.x + x, self.y + y, self.z + z)
    }

    pub fn add_vec(self, v: Vec3) -> Vec3 {
        self.add(v.x, v.y, v.z)
    }

    pub fn distance_to(self, v: Vec3) -> f64 {
        self.distance_to_sqr(v).sqrt()
    }

    pub fn distance_to_sqr(self, v: Vec3) -> f64 {
        let (dx, dy, dz) = (v.x - self.x, v.y - self.y, v.z - self.z);
        dx * dx + dy * dy + dz * dz
    }

    pub fn closer_than(self, v: Vec3, distance: f64) -> bool {
        self.distance_to_sqr(v) < distance * distance
    }

    pub fn scale(self, s: f64) -> Vec3 {
        self.multiply(s, s, s)
    }

    pub fn reverse(self) -> Vec3 {
        self.scale(-1.0)
    }

    pub fn multiply(self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(self.x * x, self.y * y, self.z * z)
    }

    pub fn horizontal(self) -> Vec3 {
        Vec3::new(self.x, 0.0, self.z)
    }

    pub fn offset_random(self, random: &mut impl RandomSource, offset: f32) -> Vec3 {
        let x = (random.next_float() - 0.5) * offset;
        let y = (random.next_float() - 0.5) * offset;
        let z = (random.next_float() - 0.5) * offset;
        self.add(x as f64, y as f64, z as f64)
    }

    pub fn offset_random_xz(self, random: &mut impl RandomSource, offset: f32) -> Vec3 {
        let x = (random.next_float() - 0.5) * offset;
        let z = (random.next_float() - 0.5) * offset;
        self.add(x as f64, 0.0, z as f64)
    }

    pub fn length(self) -> f64 {
        self.length_sqr().sqrt()
    }

    pub fn length_sqr(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn horizontal_distance(self) -> f64 {
        self.horizontal_distance_sqr().sqrt()
    }

    pub fn horizontal_distance_sqr(self) -> f64 {
        self.x * self.x + self.z * self.z
    }

    pub fn lerp(self, v: Vec3, a: f64) -> Vec3 {
        Vec3::new(mth::lerp(a, self.x, v.x), mth::lerp(a, self.y, v.y), mth::lerp(a, self.z, v.z))
    }

    pub fn x_rot(self, radians: f32) -> Vec3 {
        let cos = mth::cos(radians as f64) as f64;
        let sin = mth::sin(radians as f64) as f64;
        Vec3::new(self.x, self.y * cos + self.z * sin, self.z * cos - self.y * sin)
    }

    pub fn y_rot(self, radians: f32) -> Vec3 {
        let cos = mth::cos(radians as f64) as f64;
        let sin = mth::sin(radians as f64) as f64;
        Vec3::new(self.x * cos + self.z * sin, self.y, self.z * cos - self.x * sin)
    }

    pub fn direction_from_rotation(x_rot: f32, y_rot: f32) -> Vec3 {
        let y_cos = mth::cos((-y_rot * mth::DEG_TO_RAD - mth::PI) as f64);
        let y_sin = mth::sin((-y_rot * mth::DEG_TO_RAD - mth::PI) as f64);
        let x_cos = -mth::cos((-x_rot * mth::DEG_TO_RAD) as f64);
        let x_sin = mth::sin((-x_rot * mth::DEG_TO_RAD) as f64);
        Vec3::new((y_sin * x_cos) as f64, x_sin as f64, (y_cos * x_cos) as f64)
    }

    pub fn get(self, axis: Axis) -> f64 {
        axis.choose(self.x, self.y, self.z)
    }

    pub fn with(self, axis: Axis, value: f64) -> Vec3 {
        match axis {
            Axis::X => Vec3::new(value, self.y, self.z),
            Axis::Y => Vec3::new(self.x, value, self.z),
            Axis::Z => Vec3::new(self.x, self.y, value),
        }
    }

    pub fn relative(self, direction: Direction, distance: f64) -> Vec3 {
        Vec3::new(
            self.x + distance * direction.step_x() as f64,
            self.y + distance * direction.step_y() as f64,
            self.z + distance * direction.step_z() as f64,
        )
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AABB {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl AABB {
    pub fn new(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> AABB {
        AABB { min_x: jmin(x0, x1), min_y: jmin(y0, y1), min_z: jmin(z0, z1), max_x: jmax(x0, x1), max_y: jmax(y0, y1), max_z: jmax(z0, z1) }
    }

    pub fn of_block(pos: BlockPos) -> AABB {
        AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64)
    }

    pub fn from_corners(a: Vec3, b: Vec3) -> AABB {
        AABB::new(a.x, a.y, a.z, b.x, b.y, b.z)
    }

    pub fn of_size(center: Vec3, x: f64, y: f64, z: f64) -> AABB {
        AABB::new(center.x - x / 2.0, center.y - y / 2.0, center.z - z / 2.0, center.x + x / 2.0, center.y + y / 2.0, center.z + z / 2.0)
    }

    pub fn min(&self, axis: Axis) -> f64 {
        axis.choose(self.min_x, self.min_y, self.min_z)
    }

    pub fn max(&self, axis: Axis) -> f64 {
        axis.choose(self.max_x, self.max_y, self.max_z)
    }

    pub fn set_min_y(&self, min_y: f64) -> AABB {
        AABB::new(self.min_x, min_y, self.min_z, self.max_x, self.max_y, self.max_z)
    }

    pub fn set_max_y(&self, max_y: f64) -> AABB {
        AABB::new(self.min_x, self.min_y, self.min_z, self.max_x, max_y, self.max_z)
    }

    pub fn contract(&self, xa: f64, ya: f64, za: f64) -> AABB {
        let (mut x0, mut y0, mut z0, mut x1, mut y1, mut z1) = (self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z);
        if xa < 0.0 {
            x0 -= xa;
        } else if xa > 0.0 {
            x1 -= xa;
        }
        if ya < 0.0 {
            y0 -= ya;
        } else if ya > 0.0 {
            y1 -= ya;
        }
        if za < 0.0 {
            z0 -= za;
        } else if za > 0.0 {
            z1 -= za;
        }
        AABB::new(x0, y0, z0, x1, y1, z1)
    }

    pub fn expand_towards(&self, xa: f64, ya: f64, za: f64) -> AABB {
        let (mut x0, mut y0, mut z0, mut x1, mut y1, mut z1) = (self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z);
        if xa < 0.0 {
            x0 += xa;
        } else if xa > 0.0 {
            x1 += xa;
        }
        if ya < 0.0 {
            y0 += ya;
        } else if ya > 0.0 {
            y1 += ya;
        }
        if za < 0.0 {
            z0 += za;
        } else if za > 0.0 {
            z1 += za;
        }
        AABB::new(x0, y0, z0, x1, y1, z1)
    }

    pub fn expand_towards_vec(&self, v: Vec3) -> AABB {
        self.expand_towards(v.x, v.y, v.z)
    }

    pub fn inflate(&self, x: f64, y: f64, z: f64) -> AABB {
        AABB::new(self.min_x - x, self.min_y - y, self.min_z - z, self.max_x + x, self.max_y + y, self.max_z + z)
    }

    pub fn inflate_all(&self, amount: f64) -> AABB {
        self.inflate(amount, amount, amount)
    }

    pub fn deflate(&self, x: f64, y: f64, z: f64) -> AABB {
        self.inflate(-x, -y, -z)
    }

    pub fn deflate_all(&self, amount: f64) -> AABB {
        self.inflate_all(-amount)
    }

    pub fn intersect(&self, o: &AABB) -> AABB {
        AABB::new(
            jmax(self.min_x, o.min_x),
            jmax(self.min_y, o.min_y),
            jmax(self.min_z, o.min_z),
            jmin(self.max_x, o.max_x),
            jmin(self.max_y, o.max_y),
            jmin(self.max_z, o.max_z),
        )
    }

    pub fn minmax(&self, o: &AABB) -> AABB {
        AABB::new(
            jmin(self.min_x, o.min_x),
            jmin(self.min_y, o.min_y),
            jmin(self.min_z, o.min_z),
            jmax(self.max_x, o.max_x),
            jmax(self.max_y, o.max_y),
            jmax(self.max_z, o.max_z),
        )
    }

    pub fn move_by(&self, x: f64, y: f64, z: f64) -> AABB {
        AABB::new(self.min_x + x, self.min_y + y, self.min_z + z, self.max_x + x, self.max_y + y, self.max_z + z)
    }

    pub fn move_vec(&self, v: Vec3) -> AABB {
        self.move_by(v.x, v.y, v.z)
    }

    pub fn move_to_block(&self, pos: BlockPos) -> AABB {
        self.move_by(pos.x as f64, pos.y as f64, pos.z as f64)
    }

    pub fn intersects(&self, o: &AABB) -> bool {
        self.intersects_bounds(o.min_x, o.min_y, o.min_z, o.max_x, o.max_y, o.max_z)
    }

    pub fn intersects_bounds(&self, x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> bool {
        self.min_x < x1 && self.max_x > x0 && self.min_y < y1 && self.max_y > y0 && self.min_z < z1 && self.max_z > z0
    }

    pub fn intersects_block(&self, pos: BlockPos) -> bool {
        self.intersects_bounds(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64)
    }

    pub fn contains(&self, x: f64, y: f64, z: f64) -> bool {
        x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y && z >= self.min_z && z < self.max_z
    }

    pub fn size(&self) -> f64 {
        (self.x_size() + self.y_size() + self.z_size()) / 3.0
    }

    pub fn x_size(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn y_size(&self) -> f64 {
        self.max_y - self.min_y
    }

    pub fn z_size(&self) -> f64 {
        self.max_z - self.min_z
    }

    pub fn center(&self) -> Vec3 {
        Vec3::new(mth::lerp(0.5, self.min_x, self.max_x), mth::lerp(0.5, self.min_y, self.max_y), mth::lerp(0.5, self.min_z, self.max_z))
    }

    pub fn bottom_center(&self) -> Vec3 {
        Vec3::new(mth::lerp(0.5, self.min_x, self.max_x), self.min_y, mth::lerp(0.5, self.min_z, self.max_z))
    }

    pub fn has_nan(&self) -> bool {
        [self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z].iter().any(|v| v.is_nan())
    }

    pub fn distance_to_sqr(&self, p: Vec3) -> f64 {
        let dx = jmax(jmax(self.min_x - p.x, p.x - self.max_x), 0.0);
        let dy = jmax(jmax(self.min_y - p.y, p.y - self.max_y), 0.0);
        let dz = jmax(jmax(self.min_z - p.z, p.z - self.max_z), 0.0);
        mth::length_squared(dx, dy, dz)
    }

    pub fn contains_vec(&self, p: Vec3) -> bool {
        self.contains(p.x, p.y, p.z)
    }

    pub fn collided_along_vector(&self, vector: Vec3, boxes: &[AABB]) -> bool {
        let from = self.center();
        let to = from.add_vec(vector);
        for part in boxes {
            let inflated = part.inflate(self.x_size() * 0.5 - 1.0e-7, self.y_size() * 0.5 - 1.0e-7, self.z_size() * 0.5 - 1.0e-7);
            if inflated.contains_vec(to) || inflated.contains_vec(from) {
                return true;
            }
            if inflated.clip(from, to).is_some() {
                return true;
            }
        }
        false
    }

    pub fn clip(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        let mut scale = 1.0;
        let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
        self.clip_direction(from, &mut scale, None, dx, dy, dz)?;
        Some(from.add(scale * dx, scale * dy, scale * dz))
    }

    pub fn clip_direction(&self, from: Vec3, scale: &mut f64, mut direction: Option<Direction>, dx: f64, dy: f64, dz: f64) -> Option<Direction> {
        let b = self;
        if dx > 1.0e-7 {
            direction = clip_point(scale, direction, dx, dy, dz, b.min_x, b.min_y, b.max_y, b.min_z, b.max_z, Direction::West, from.x, from.y, from.z);
        } else if dx < -1.0e-7 {
            direction = clip_point(scale, direction, dx, dy, dz, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z, Direction::East, from.x, from.y, from.z);
        }
        if dy > 1.0e-7 {
            direction = clip_point(scale, direction, dy, dz, dx, b.min_y, b.min_z, b.max_z, b.min_x, b.max_x, Direction::Down, from.y, from.z, from.x);
        } else if dy < -1.0e-7 {
            direction = clip_point(scale, direction, dy, dz, dx, b.max_y, b.min_z, b.max_z, b.min_x, b.max_x, Direction::Up, from.y, from.z, from.x);
        }
        if dz > 1.0e-7 {
            direction = clip_point(scale, direction, dz, dx, dy, b.min_z, b.min_x, b.max_x, b.min_y, b.max_y, Direction::North, from.z, from.x, from.y);
        } else if dz < -1.0e-7 {
            direction = clip_point(scale, direction, dz, dx, dy, b.max_z, b.min_x, b.max_x, b.min_y, b.max_y, Direction::South, from.z, from.x, from.y);
        }
        direction
    }
}

#[allow(clippy::too_many_arguments)]
fn clip_point(
    scale: &mut f64,
    direction: Option<Direction>,
    da: f64,
    db: f64,
    dc: f64,
    point: f64,
    min_b: f64,
    max_b: f64,
    min_c: f64,
    max_c: f64,
    new_direction: Direction,
    from_a: f64,
    from_b: f64,
    from_c: f64,
) -> Option<Direction> {
    let s = (point - from_a) / da;
    let pb = from_b + s * db;
    let pc = from_c + s * dc;
    if 0.0 < s && s < *scale && min_b - 1.0e-7 < pb && pb < max_b + 1.0e-7 && min_c - 1.0e-7 < pc && pc < max_c + 1.0e-7 {
        *scale = s;
        Some(new_direction)
    } else {
        direction
    }
}
