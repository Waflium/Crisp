use crisp_core::hash_map::FastMap;
use crisp_core::{AABB, Axis, BlockPos, Direction, Vec3, mth};
use crisp_data::properties::OPEN;
use crisp_data::{block_tags, blocks, fluid_tags};

use super::node::Node;
use super::node_evaluator::{NodeEvaluator, is_burning_block};
use super::path_type::PathType;
use super::pathfinding_context::PathfindingContext;
use super::target::Target;
use crate::world::entity::mob::Mob;
use crate::world::level::block::door_block;
use crate::world::level::path_navigation_region::PathNavigationRegion;

#[derive(Default)]
pub struct WalkNodeEvaluator {
    pub node_evaluator: NodeEvaluator,
    path_types_by_pos_cache_by_mob: FastMap<i64, PathType>,
    collision_cache: FastMap<[u64; 6], bool>,
    reusable_neighbors: [Option<u32>; 4],
}

impl WalkNodeEvaluator {
    pub fn prepare(&mut self, context: &PathfindingContext, mob: crate::world::entity::EntityId) {
        self.node_evaluator.prepare(context, mob);
    }

    pub fn done(&mut self) {
        self.path_types_by_pos_cache_by_mob.clear();
        self.collision_cache.clear();
    }

    fn mob<'b>(&self, context: &'b PathfindingContext) -> &'b Mob {
        context.level().level.mob(self.node_evaluator.mob).expect("a pathfinding mob")
    }

    pub fn get_start(&mut self, context: &PathfindingContext) -> Option<u32> {
        let mob = self.mob(context);
        let e = &mob.entity;
        let mut start_y = e.block_position().y;
        let mut block_state = context.get_block_state(BlockPos::containing(e.x(), start_y as f64, e.z()));
        if self.node_evaluator.can_float && e.fluid_interaction.is_in_fluid(&fluid_tags::ENTITY_FLOATABLE) {
            while block_state.fluid_state().is(&fluid_tags::ENTITY_FLOATABLE) {
                start_y += 1;
                block_state = context.get_block_state(BlockPos::containing(e.x(), start_y as f64, e.z()));
            }
            start_y -= 1;
        } else if e.on_ground {
            start_y = mth::floor(e.y() + 0.5);
        } else {
            let mut reusable_pos = BlockPos::containing(e.x(), e.y() + 1.0, e.z());
            while reusable_pos.y > context.level().get_min_y() {
                start_y = reusable_pos.y;
                reusable_pos.y -= 1;
                let below_block_state = context.get_block_state(reusable_pos);
                if !below_block_state.is_air() && !below_block_state.is_pathfindable_land() {
                    break;
                }
            }
        }
        let start_pos = e.block_position();
        if !self.can_start_at(context, BlockPos::new(start_pos.x, start_y, start_pos.z)) {
            let mob_bb = e.bounding_box();
            for (x, z) in [(mob_bb.min_x, mob_bb.min_z), (mob_bb.min_x, mob_bb.max_z), (mob_bb.max_x, mob_bb.min_z), (mob_bb.max_x, mob_bb.max_z)] {
                let pos = BlockPos::containing(x, start_y as f64, z);
                if self.can_start_at(context, pos) {
                    return Some(self.get_start_node(context, pos));
                }
            }
        }
        Some(self.get_start_node(context, BlockPos::new(start_pos.x, start_y, start_pos.z)))
    }

    fn get_start_node(&mut self, context: &PathfindingContext, pos: BlockPos) -> u32 {
        let node = self.node_evaluator.get_node_at(pos);
        let path_type = self.get_cached_path_type(context, pos.x, pos.y, pos.z);
        let cost_malus = self.mob(context).get_pathfinding_malus(path_type);
        let n = &mut self.node_evaluator.nodes[node as usize];
        n.r#type = path_type;
        n.cost_malus = cost_malus;
        node
    }

    fn can_start_at(&mut self, context: &PathfindingContext, pos: BlockPos) -> bool {
        let block_path_type = self.get_cached_path_type(context, pos.x, pos.y, pos.z);
        block_path_type != PathType::Open && self.mob(context).get_pathfinding_malus(block_path_type) >= 0.0
    }

    pub fn get_target(&mut self, x: f64, y: f64, z: f64) -> Target {
        self.node_evaluator.get_target_node_at(x, y, z)
    }

    pub fn get_neighbors(&mut self, context: &PathfindingContext, neighbors: &mut [u32; 32], pos: u32) -> usize {
        let mut p = 0;
        let mut jump_size = 0;
        let Node { x, y, z, .. } = self.node_evaluator.nodes[pos as usize];
        let block_path_type_above = self.get_cached_path_type(context, x, y + 1, z);
        let block_path_type_current = self.get_cached_path_type(context, x, y, z);
        let max_up_step = context.level().level.max_up_step(self.node_evaluator.mob);
        if self.mob(context).get_pathfinding_malus(block_path_type_above) >= 0.0 && block_path_type_current != PathType::StickyHoney {
            jump_size = mth::floor_f(1.0f32.max(max_up_step));
        }
        let pos_height = self.get_floor_level(context, BlockPos::new(x, y, z));
        for direction in Direction::PLANE_HORIZONTAL {
            let node = self.find_accepted_node(context, x + direction.step_x(), y, z + direction.step_z(), jump_size, pos_height, direction, block_path_type_current);
            self.reusable_neighbors[direction.get_2d_data_value() as usize] = node;
            if self.is_neighbor_valid(node, pos) {
                neighbors[p] = node.expect("a valid neighbor");
                p += 1;
            }
        }
        for direction in Direction::PLANE_HORIZONTAL {
            let second_direction = direction.clockwise();
            let ew = self.reusable_neighbors[direction.get_2d_data_value() as usize];
            let ns = self.reusable_neighbors[second_direction.get_2d_data_value() as usize];
            if self.is_diagonal_valid(context, pos, ew, ns) {
                let diagonal_node = self.find_accepted_node(
                    context,
                    x + direction.step_x() + second_direction.step_x(),
                    y,
                    z + direction.step_z() + second_direction.step_z(),
                    jump_size,
                    pos_height,
                    direction,
                    block_path_type_current,
                );
                if self.is_diagonal_node_valid(diagonal_node) {
                    neighbors[p] = diagonal_node.expect("a valid diagonal");
                    p += 1;
                }
            }
        }
        p
    }

    fn is_neighbor_valid(&self, neighbor: Option<u32>, current: u32) -> bool {
        let nodes = &self.node_evaluator.nodes;
        neighbor.is_some_and(|neighbor| {
            let neighbor = &nodes[neighbor as usize];
            !neighbor.closed && (neighbor.cost_malus >= 0.0 || nodes[current as usize].cost_malus < 0.0)
        })
    }

    fn is_diagonal_valid(&self, context: &PathfindingContext, pos: u32, ew: Option<u32>, ns: Option<u32>) -> bool {
        let nodes = &self.node_evaluator.nodes;
        let (Some(ns), Some(ew)) = (ns, ew) else { return false };
        let (pos, ew, ns) = (&nodes[pos as usize], &nodes[ew as usize], &nodes[ns as usize]);
        if ns.y > pos.y || ew.y > pos.y {
            return false;
        }
        if ew.r#type == PathType::WalkableDoor || ns.r#type == PathType::WalkableDoor {
            return false;
        }
        let width = self.mob(context).entity.dimensions.width;
        if width > 1.0 && (ew.cost_malus > 0.0 || ns.cost_malus > 0.0) {
            return false;
        }
        let can_pass_between_posts = ns.r#type == PathType::Fence && ew.r#type == PathType::Fence && (width as f64) < 0.5;
        (ns.y < pos.y || ns.cost_malus >= 0.0 || can_pass_between_posts) && (ew.y < pos.y || ew.cost_malus >= 0.0 || can_pass_between_posts)
    }

    fn is_diagonal_node_valid(&self, diagonal: Option<u32>) -> bool {
        let Some(diagonal) = diagonal else { return false };
        let diagonal = &self.node_evaluator.nodes[diagonal as usize];
        if diagonal.closed {
            return false;
        }
        if diagonal.r#type == PathType::WalkableDoor { false } else { diagonal.cost_malus >= 0.0 }
    }

    fn can_reach_without_collision(&mut self, context: &PathfindingContext, pos_to: u32) -> bool {
        let e = &self.mob(context).entity;
        let mut bb = e.bounding_box();
        let to = &self.node_evaluator.nodes[pos_to as usize];
        let mut delta = Vec3::new(
            to.x as f64 - e.x() + bb.x_size() / 2.0,
            to.y as f64 - e.y() + bb.y_size() / 2.0,
            to.z as f64 - e.z() + bb.z_size() / 2.0,
        );
        let steps = mth::ceil(delta.length() / bb.size());
        delta = delta.scale((1.0f32 / steps as f32) as f64);
        for _ in 1..=steps {
            bb = bb.move_vec(delta);
            if self.has_collisions(context, &bb) {
                return false;
            }
        }
        true
    }

    fn get_floor_level(&self, context: &PathfindingContext, pos: BlockPos) -> f64 {
        let level = context.level();
        if self.node_evaluator.can_float && level.get_fluid_state(pos).is(&fluid_tags::ENTITY_FLOATABLE) {
            pos.y as f64 + 0.5
        } else {
            get_floor_level(level, pos)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn find_accepted_node(
        &mut self,
        context: &PathfindingContext,
        x: i32,
        y: i32,
        z: i32,
        jump_size: i32,
        node_height: f64,
        travel_direction: Direction,
        block_path_type_current: PathType,
    ) -> Option<u32> {
        let mut best = None;
        let max_y_target = self.get_floor_level(context, BlockPos::new(x, y, z));
        if max_y_target - node_height > self.get_mob_jump_height(context) {
            return None;
        }
        let path_type = self.get_cached_path_type(context, x, y, z);
        let path_cost = self.mob(context).get_pathfinding_malus(path_type);
        if path_cost >= 0.0 {
            best = Some(self.get_node_and_update_cost_to_max(x, y, z, path_type, path_cost));
        }
        if does_block_have_partial_collision(block_path_type_current)
            && let Some(node) = best
            && self.node_evaluator.nodes[node as usize].cost_malus >= 0.0
            && !self.can_reach_without_collision(context, node)
        {
            best = None;
        }
        if path_type == PathType::Walkable {
            return best;
        }
        if (best.is_none_or(|best| self.node_evaluator.nodes[best as usize].cost_malus < 0.0))
            && jump_size > 0
            && (path_type != PathType::Fence || self.node_evaluator.can_walk_over_fences)
            && path_type != PathType::UnpassableRail
            && path_type != PathType::Trapdoor
            && path_type != PathType::PowderSnow
        {
            best = self.try_jump_on(context, x, y, z, jump_size, node_height, travel_direction, block_path_type_current);
        } else if path_type == PathType::Water && !self.node_evaluator.can_float {
            best = self.try_find_first_non_water_below(context, x, y, z, best);
        } else if path_type == PathType::Open {
            best = Some(self.try_find_first_ground_node_below(context, x, y, z));
        } else if does_block_have_partial_collision(path_type) && best.is_none() {
            best = Some(self.get_closed_node(x, y, z, path_type));
        }
        best
    }

    fn get_mob_jump_height(&self, context: &PathfindingContext) -> f64 {
        1.125f64.max(context.level().level.max_up_step(self.node_evaluator.mob) as f64)
    }

    fn get_node_and_update_cost_to_max(&mut self, x: i32, y: i32, z: i32, path_type: PathType, cost: f32) -> u32 {
        let node = self.node_evaluator.get_node(x, y, z);
        let n = &mut self.node_evaluator.nodes[node as usize];
        n.r#type = path_type;
        n.cost_malus = n.cost_malus.max(cost);
        node
    }

    fn get_blocked_node(&mut self, x: i32, y: i32, z: i32) -> u32 {
        let node = self.node_evaluator.get_node(x, y, z);
        let n = &mut self.node_evaluator.nodes[node as usize];
        n.r#type = PathType::Blocked;
        n.cost_malus = -1.0;
        node
    }

    fn get_closed_node(&mut self, x: i32, y: i32, z: i32, path_type: PathType) -> u32 {
        let node = self.node_evaluator.get_node(x, y, z);
        let n = &mut self.node_evaluator.nodes[node as usize];
        n.closed = true;
        n.r#type = path_type;
        n.cost_malus = path_type.get_malus();
        node
    }

    #[allow(clippy::too_many_arguments)]
    fn try_jump_on(
        &mut self,
        context: &PathfindingContext,
        x: i32,
        y: i32,
        z: i32,
        jump_size: i32,
        node_height: f64,
        travel_direction: Direction,
        block_path_type_current: PathType,
    ) -> Option<u32> {
        let node_above = self.find_accepted_node(context, x, y + 1, z, jump_size - 1, node_height, travel_direction, block_path_type_current)?;
        let dimensions = self.mob(context).entity.dimensions;
        if dimensions.width >= 1.0 {
            return Some(node_above);
        }
        let above = self.node_evaluator.nodes[node_above as usize];
        if above.r#type != PathType::Open && above.r#type != PathType::Walkable {
            return Some(node_above);
        }
        let center_x = (x - travel_direction.step_x()) as f64 + 0.5;
        let center_z = (z - travel_direction.step_z()) as f64 + 0.5;
        let half_width = dimensions.width as f64 / 2.0;
        let grow = AABB::new(
            center_x - half_width,
            self.get_floor_level(context, BlockPos::containing(center_x, (y + 1) as f64, center_z)) + 0.001,
            center_z - half_width,
            center_x + half_width,
            dimensions.height as f64 + self.get_floor_level(context, BlockPos::new(above.x, above.y, above.z)) - 0.002,
            center_z + half_width,
        );
        if self.has_collisions(context, &grow) { None } else { Some(node_above) }
    }

    fn try_find_first_non_water_below(&mut self, context: &PathfindingContext, x: i32, mut y: i32, z: i32, mut best: Option<u32>) -> Option<u32> {
        y -= 1;
        while y > context.level().get_min_y() {
            let path_type_local = self.get_cached_path_type(context, x, y, z);
            if path_type_local != PathType::Water {
                return best;
            }
            let malus = self.mob(context).get_pathfinding_malus(path_type_local);
            best = Some(self.get_node_and_update_cost_to_max(x, y, z, path_type_local, malus));
            y -= 1;
        }
        best
    }

    fn try_find_first_ground_node_below(&mut self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> u32 {
        let max_fall_distance = context.level().level.get_max_fall_distance(self.node_evaluator.mob);
        let mut current_y = y - 1;
        while current_y >= context.level().get_min_y() {
            if y - current_y > max_fall_distance {
                return self.get_blocked_node(x, current_y, z);
            }
            let path_type = self.get_cached_path_type(context, x, current_y, z);
            let path_cost = self.mob(context).get_pathfinding_malus(path_type);
            if path_type != PathType::Open {
                if path_cost >= 0.0 {
                    return self.get_node_and_update_cost_to_max(x, current_y, z, path_type, path_cost);
                }
                return self.get_blocked_node(x, current_y, z);
            }
            current_y -= 1;
        }
        self.get_blocked_node(x, y, z)
    }

    fn has_collisions(&mut self, context: &PathfindingContext, aabb: &AABB) -> bool {
        let key = [aabb.min_x.to_bits(), aabb.min_y.to_bits(), aabb.min_z.to_bits(), aabb.max_x.to_bits(), aabb.max_y.to_bits(), aabb.max_z.to_bits()];
        let mob = self.node_evaluator.mob;
        *self.collision_cache.entry(key).or_insert_with(|| !context.level().no_collision(mob, aabb))
    }

    pub fn get_cached_path_type(&mut self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let key = BlockPos::new(x, y, z).as_long();
        if let Some(&path_type) = self.path_types_by_pos_cache_by_mob.get(&key) {
            return path_type;
        }
        let path_type = self.get_path_type_of_mob(context, x, y, z);
        self.path_types_by_pos_cache_by_mob.insert(key, path_type);
        path_type
    }

    pub fn get_path_type_of_mob(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let mob = self.mob(context);
        let block_types = self.get_path_type_within_mob_bb(context, x, y, z);
        if block_types.count_ones() == 1 {
            return PathType::VALUES[block_types.trailing_zeros() as usize];
        }
        if block_types & 1 << PathType::Fence as u32 != 0 {
            return PathType::Fence;
        }
        if block_types & 1 << PathType::UnpassableRail as u32 != 0 {
            return PathType::UnpassableRail;
        }
        let mut highest_malus_path_type_within_bb = PathType::Blocked;
        let mut highest_malus_within_bb = mob.get_pathfinding_malus(highest_malus_path_type_within_bb);
        for path_type in PathType::VALUES {
            if block_types & 1 << path_type as u32 == 0 {
                continue;
            }
            let malus_for_path_type = mob.get_pathfinding_malus(path_type);
            if malus_for_path_type < 0.0 {
                return path_type;
            }
            if malus_for_path_type >= highest_malus_within_bb {
                highest_malus_within_bb = malus_for_path_type;
                highest_malus_path_type_within_bb = path_type;
            }
        }
        let current_node_path_type = get_path_type_static(context, x, y, z);
        if self.node_evaluator.entity_width > 1 {
            let is_current_node_cheaper = mob.get_pathfinding_malus(current_node_path_type) < highest_malus_within_bb;
            let cap_malus_due_to_cheap_node = is_current_node_cheaper && mob.get_pathfinding_malus(PathType::BigMobsCloseToDanger) < highest_malus_within_bb;
            if cap_malus_due_to_cheap_node { PathType::BigMobsCloseToDanger } else { highest_malus_path_type_within_bb }
        } else if current_node_path_type == PathType::Open && highest_malus_path_type_within_bb != PathType::Open && highest_malus_within_bb == 0.0 {
            PathType::Open
        } else {
            highest_malus_path_type_within_bb
        }
    }

    fn get_path_type_within_mob_bb(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> u32 {
        let mut block_types = 0u32;
        let evaluator = &self.node_evaluator;
        for dx in 0..evaluator.entity_width {
            for dy in 0..evaluator.entity_height {
                for dz in 0..evaluator.entity_depth {
                    let mut block_type = get_path_type_static(context, dx + x, dy + y, dz + z);
                    let mob_position = self.mob(context).entity.block_position();
                    let can_pass_doors = evaluator.can_pass_doors;
                    if block_type == PathType::DoorWoodClosed && evaluator.can_open_doors && can_pass_doors {
                        block_type = PathType::WalkableDoor;
                    }
                    if block_type == PathType::DoorOpen && !can_pass_doors {
                        block_type = PathType::Blocked;
                    }
                    if block_type == PathType::Rail
                        && get_path_type_static(context, mob_position.x, mob_position.y, mob_position.z) != PathType::Rail
                        && get_path_type_static(context, mob_position.x, mob_position.y - 1, mob_position.z) != PathType::Rail
                    {
                        block_type = PathType::UnpassableRail;
                    }
                    block_types |= 1 << block_type as u32;
                }
            }
        }
        block_types
    }
}

fn does_block_have_partial_collision(path_type: PathType) -> bool {
    path_type == PathType::Fence || path_type == PathType::DoorWoodClosed || path_type == PathType::DoorIronClosed
}

pub fn get_floor_level(level: &PathNavigationRegion, pos: BlockPos) -> f64 {
    let target = pos.below();
    let shape = level.get_block_state(target).collision_voxel_shape();
    target.y as f64 + if shape.is_empty() { 0.0 } else { shape.max(Axis::Y) }
}

pub fn get_path_type_static(context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
    let block_path_type = context.get_path_type_from_state(x, y, z);
    if block_path_type == PathType::Open && y >= context.level().get_min_y() + 1 {
        match context.get_path_type_from_state(x, y - 1, z) {
            PathType::Open | PathType::Water | PathType::Lava | PathType::Walkable => PathType::Open,
            PathType::Fire => PathType::Fire,
            PathType::Damaging => PathType::Damaging,
            PathType::StickyHoney => PathType::StickyHoney,
            PathType::PowderSnow => PathType::OnTopOfPowderSnow,
            PathType::DamageCautious => PathType::DamageCautious,
            PathType::Trapdoor => PathType::OnTopOfTrapdoor,
            _ => check_neighbour_blocks(context, x, y, z, PathType::Walkable),
        }
    } else {
        block_path_type
    }
}

pub fn check_neighbour_blocks(context: &PathfindingContext, x: i32, y: i32, z: i32, block_path_type: PathType) -> PathType {
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if dx != 0 || dz != 0 {
                    let path_type = context.get_path_type_from_state(x + dx, y + dy, z + dz);
                    if path_type == PathType::Damaging {
                        return PathType::DamagingInNeighbor;
                    }
                    if path_type == PathType::Fire || path_type == PathType::Lava {
                        return PathType::FireInNeighbor;
                    }
                    if path_type == PathType::Water {
                        return PathType::WaterBorder;
                    }
                    if path_type == PathType::DamageCautious {
                        return PathType::DamageCautious;
                    }
                }
            }
        }
    }
    block_path_type
}

pub fn get_path_type_from_state(level: &PathNavigationRegion, pos: BlockPos) -> PathType {
    let block_state = level.get_block_state(pos);
    let block = block_state.block();
    if block_state.is_air() {
        return PathType::Open;
    }
    if block_state.is(&block_tags::TRAPDOORS) || block_state.is_block(blocks::LILY_PAD) || block_state.is_block(blocks::BIG_DRIPLEAF) {
        return PathType::Trapdoor;
    }
    if block_state.is_block(blocks::POWDER_SNOW) {
        return PathType::PowderSnow;
    }
    if block_state.is_block(blocks::CACTUS) || block_state.is_block(blocks::SWEET_BERRY_BUSH) {
        return PathType::Damaging;
    }
    if block_state.is_block(blocks::HONEY_BLOCK) {
        return PathType::StickyHoney;
    }
    if block_state.is_block(blocks::COCOA) {
        return PathType::Cocoa;
    }
    if block_state.is_block(blocks::WITHER_ROSE) || block_state.is(&block_tags::SPELEOTHEMS) {
        return PathType::DamageCautious;
    }
    let fluid_state = block_state.fluid_state();
    if fluid_state.is(&fluid_tags::LAVA) {
        return PathType::Lava;
    }
    if is_burning_block(block_state) {
        return PathType::Fire;
    }
    if block.is_instance_of("DoorBlock") {
        return if block_state.get(OPEN) {
            PathType::DoorOpen
        } else if door_block::type_can_open_by_hand(block) {
            PathType::DoorWoodClosed
        } else {
            PathType::DoorIronClosed
        };
    }
    if block.is_instance_of("BaseRailBlock") {
        return PathType::Rail;
    }
    if block.is_instance_of("LeavesBlock") {
        return PathType::Leaves;
    }
    if block_state.is(&block_tags::FENCES) || block_state.is(&block_tags::WALLS) || block.is_instance_of("FenceGateBlock") && !block_state.get(OPEN) {
        return PathType::Fence;
    }
    if !block_state.is_pathfindable_land() {
        PathType::Blocked
    } else if fluid_state.is(&fluid_tags::WATER) {
        PathType::Water
    } else {
        PathType::Open
    }
}
