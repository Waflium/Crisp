use crisp_core::{AABB, BlockPos, ChunkPos, Direction, Vec3, mth};
use crisp_data::{Fluid, FluidState, Tag, block_tags, fluid_tags};

use super::EntityId;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

struct Tracker {
    fluid: Fluid,
    height: f64,
    eyes_inside: bool,
}

impl Tracker {
    fn reset(&mut self) -> bool {
        if self.height == 0.0 && !self.eyes_inside {
            return true;
        }
        self.height = 0.0;
        self.eyes_inside = false;
        false
    }
}

#[derive(Default)]
struct CurrentAccumulator {
    height: f64,
    accumulated: Vec3,
    count: i32,
}

impl CurrentAccumulator {
    fn reset(&mut self) {
        *self = CurrentAccumulator::default();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CurrentFluid {
    Water,
    Lava,
}

#[derive(Default)]
pub struct EntityFluidInteraction {
    trackers: Vec<Tracker>,
    water_current: CurrentAccumulator,
    lava_current: CurrentAccumulator,
}

impl EntityFluidInteraction {
    fn tracker_index(&mut self, fluid: Fluid) -> usize {
        match self.trackers.iter().position(|t| t.fluid == fluid) {
            Some(i) => i,
            None => {
                self.trackers.push(Tracker { fluid, height: 0.0, eyes_inside: false });
                self.trackers.len() - 1
            }
        }
    }

    fn current(&mut self, which: CurrentFluid) -> &mut CurrentAccumulator {
        match which {
            CurrentFluid::Water => &mut self.water_current,
            CurrentFluid::Lava => &mut self.lava_current,
        }
    }

    pub fn fluid_height(&self, fluid: &Tag) -> f64 {
        let mut height = 0.0;
        for tracker in &self.trackers {
            if tracker.height > height && tracker.fluid.is(fluid) {
                height = tracker.height;
            }
        }
        height
    }

    pub fn is_in_fluid(&self, fluid: &Tag) -> bool {
        self.fluid_height(fluid) > 0.0
    }

    pub fn is_eye_in_fluid(&self, fluid: &Tag) -> bool {
        self.trackers.iter().any(|t| t.eyes_inside && t.fluid.is(fluid))
    }
}

fn current_fluid(fluid: Fluid) -> Option<CurrentFluid> {
    if fluid.is(&fluid_tags::WATER) {
        Some(CurrentFluid::Water)
    } else if fluid.is(&fluid_tags::LAVA) {
        Some(CurrentFluid::Lava)
    } else {
        None
    }
}

impl ServerLevel {
    pub fn update_fluid_trackers(&mut self, id: EntityId, ignore_current: bool) -> bool {
        let fluid_interaction_box = self.fluid_interaction_box(id);
        let e = self.entity_mut(id);
        let mut fi = std::mem::take(&mut e.fluid_interaction);
        let entity_y = e.bounding_box().min_y;
        let (eye_block_x, eye_y, eye_block_z) = (mth::floor(e.x()), e.eye_y(), mth::floor(e.z()));
        fi.trackers.retain_mut(|t| !t.reset());
        fi.water_current.reset();
        fi.lava_current.reset();
        let result = match fluid_interaction_box {
            Some(box_) => self.scan_fluids(&mut fi, box_, entity_y, eye_block_x, eye_y, eye_block_z, ignore_current),
            None => false,
        };
        self.entity_mut(id).fluid_interaction = fi;
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_fluids(
        &mut self,
        fi: &mut EntityFluidInteraction,
        box_: AABB,
        entity_y: f64,
        eye_block_x: i32,
        eye_y: f64,
        eye_block_z: i32,
        ignore_current: bool,
    ) -> bool {
        let (x0, y0, z0) = (mth::floor(box_.min_x), mth::floor(box_.min_y), mth::floor(box_.min_z));
        let (x1, y1, z1) = (mth::ceil(box_.max_x) - 1, mth::ceil(box_.max_y) - 1, mth::ceil(box_.max_z) - 1);
        if !self.has_fluid_and_loaded(x0 - 1, y0, z0 - 1, x1 + 1, y1, z1 + 1) {
            return false;
        }
        let mut last_fluid: Option<Fluid> = None;
        let mut tracker = 0;
        let mut current: Option<CurrentFluid> = None;
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    let pos = BlockPos::new(x, y, z);
                    let fluid_state = self.fluid_state(pos);
                    if fluid_state.is_empty() {
                        continue;
                    }
                    let fluid_bottom = pos.y as f64;
                    let fluid_top = fluid_bottom + self.fluid_height(fluid_state, pos) as f64;
                    if fluid_top < box_.min_y {
                        continue;
                    }
                    if last_fluid != Some(fluid_state.fluid) {
                        last_fluid = Some(fluid_state.fluid);
                        tracker = fi.tracker_index(fluid_state.fluid);
                        if !ignore_current {
                            current = current_fluid(fluid_state.fluid);
                        }
                    }
                    if x == eye_block_x && z == eye_block_z && eye_y >= fluid_bottom {
                        let fluid_top_for_camera = fluid_bottom + self.fluid_height_for_camera(fluid_state, pos) as f64;
                        if eye_y <= fluid_top_for_camera {
                            fi.trackers[tracker].eyes_inside = true;
                        }
                    }
                    let height = (fluid_top - entity_y).max(fi.trackers[tracker].height);
                    fi.trackers[tracker].height = height;
                    if let Some(which) = current {
                        let mut flow = self.fluid_flow(fluid_state, pos);
                        let accumulator = fi.current(which);
                        accumulator.height = height.max(accumulator.height);
                        if accumulator.height < 0.4 {
                            flow = flow.scale(accumulator.height);
                        }
                        accumulator.accumulated = accumulator.accumulated.add_vec(flow);
                        accumulator.count += 1;
                    }
                }
            }
        }
        last_fluid.is_some()
    }

    fn has_fluid_and_loaded(&self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> bool {
        let mut has_fluid = false;
        for chunk_z in z0 >> 4..=z1 >> 4 {
            for chunk_x in x0 >> 4..=x1 >> 4 {
                let Some(chunk) = self.chunks.loaded_chunk(ChunkPos::new(chunk_x, chunk_z)) else { return false };
                let min_section = chunk.min_y() >> 4;
                for section_y in y0 >> 4..=y1 >> 4 {
                    let index = section_y - min_section;
                    if index >= 0 && (index as usize) < chunk.sections.len() {
                        has_fluid |= chunk.sections[index as usize].has_fluid();
                    }
                }
            }
        }
        has_fluid
    }

    pub fn apply_fluid_current(&mut self, id: EntityId, which: CurrentFluid, scale: f64) {
        let is_player = self.players.contains_key(&id);
        let e = self.entity_mut(id);
        let current = e.fluid_interaction.current(which);
        if current.count == 0 || current.accumulated.length_sqr() < 1.0e-5f32 as f64 {
            return;
        }
        let impulse = if is_player { current.accumulated.scale(1.0 / current.count as f64) } else { current.accumulated.normalize() };
        let old_movement = e.delta_movement;
        let mut impulse = impulse.scale(scale);
        if old_movement.x.abs() < 0.003 && old_movement.z.abs() < 0.003 && impulse.length() < 0.0045000000000000005 {
            impulse = impulse.normalize().scale(0.0045000000000000005);
        }
        e.add_delta_movement(impulse);
    }

    pub fn fluid_height(&mut self, fluid: FluidState, pos: BlockPos) -> f32 {
        if fluid.is_empty() {
            return 0.0;
        }
        if fluid.fluid.is_same(self.fluid_state(pos.above()).fluid) { 1.0 } else { fluid.own_height() }
    }

    pub fn fluid_height_for_camera(&mut self, fluid: FluidState, pos: BlockPos) -> f32 {
        if fluid.is_source() && self.block_state(pos.above()).is_face_sturdy_full(Direction::Down) {
            return 1.0;
        }
        self.fluid_height(fluid, pos)
    }

    pub fn fluid_aabb(&mut self, fluid: FluidState, pos: BlockPos) -> Option<AABB> {
        if fluid.is_empty() {
            return None;
        }
        let height = self.fluid_height(fluid, pos) as f64;
        Some(AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, pos.x as f64 + 1.0, pos.y as f64 + height, pos.z as f64 + 1.0))
    }

    pub fn fluid_flow(&mut self, fluid: FluidState, pos: BlockPos) -> Vec3 {
        if fluid.is_empty() {
            return Vec3::ZERO;
        }
        let affects_flow = |neighbour: FluidState| neighbour.is_empty() || neighbour.fluid.is_same(fluid.fluid);
        let (mut flow_x, mut flow_z) = (0.0f64, 0.0f64);
        for direction in Direction::PLANE_HORIZONTAL {
            let neighbour_pos = pos.relative(direction);
            let neighbour = self.fluid_state(neighbour_pos);
            if !affects_flow(neighbour) {
                continue;
            }
            let mut neighbour_height = neighbour.own_height();
            let mut distance = 0.0f32;
            if neighbour_height == 0.0 {
                if !self.block_state(neighbour_pos).is(&block_tags::BLOCKS_FLUID_FLOW) {
                    let below = self.fluid_state(neighbour_pos.below());
                    if affects_flow(below) {
                        neighbour_height = below.own_height();
                        if neighbour_height > 0.0 {
                            distance = fluid.own_height() - (neighbour_height - 0.8888889);
                        }
                    }
                }
            } else if neighbour_height > 0.0 {
                distance = fluid.own_height() - neighbour_height;
            }
            if distance != 0.0 {
                flow_x += (direction.step_x() as f32 * distance) as f64;
                flow_z += (direction.step_z() as f32 * distance) as f64;
            }
        }
        let mut flow = Vec3::new(flow_x, 0.0, flow_z);
        if fluid.falling {
            for direction in Direction::PLANE_HORIZONTAL {
                let neighbour_pos = pos.relative(direction);
                if self.is_solid_face(fluid, neighbour_pos, direction) || self.is_solid_face(fluid, neighbour_pos.above(), direction) {
                    flow = flow.normalize().add(0.0, -6.0, 0.0);
                    break;
                }
            }
        }
        flow.normalize()
    }

    fn is_solid_face(&mut self, fluid: FluidState, pos: BlockPos, direction: Direction) -> bool {
        let state = self.block_state(pos);
        if state.fluid_state().fluid.is_same(fluid.fluid) {
            false
        } else if direction == Direction::Up {
            true
        } else if state.block().is_instance_of("IceBlock") {
            false
        } else {
            state.is_face_sturdy_full(direction)
        }
    }
}
