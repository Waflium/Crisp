use std::sync::LazyLock;

use crisp_core::shapes::VoxelShape;
use crisp_core::{AABB, Axis, BlockPos};
use crisp_data::properties::{BOTTOM, STABILITY_DISTANCE};
use crisp_data::{BlockState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::{Entity, EntityId, LevelEntity};

static EMPTY: LazyLock<VoxelShape> = LazyLock::new(VoxelShape::empty);
static BLOCK: LazyLock<VoxelShape> = LazyLock::new(VoxelShape::block);
static SCAFFOLDING_SHAPE_UNSTABLE_BOTTOM: LazyLock<VoxelShape> = LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(0.0, 0.0, 0.0, 1.0, 0.125, 1.0)]));
static POWDER_SNOW_FALLING_COLLISION_SHAPE: LazyLock<VoxelShape> =
    LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(0.0, 0.0, 0.0, 1.0, 0.9f32 as f64, 1.0)]));

#[derive(Clone, Copy, Debug)]
struct EntityProperties {
    fall_distance: f64,
    is_falling_block: bool,
    can_walk_on_powder_snow: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CollisionContext {
    descending: bool,
    placement: bool,
    entity_bottom: f64,
    entity: Option<EntityProperties>,
}

impl CollisionContext {
    pub const EMPTY: CollisionContext = CollisionContext { descending: false, placement: false, entity_bottom: -f64::MAX, entity: None };

    pub fn of(level: &ServerLevel, entity: EntityId) -> CollisionContext {
        CollisionContext::new(level, entity, false, level.entity(entity).y())
    }

    pub fn of_detached(entity: &Entity) -> CollisionContext {
        let properties = EntityProperties { fall_distance: entity.fall_distance, is_falling_block: false, can_walk_on_powder_snow: false };
        CollisionContext { descending: entity.is_shift_key_down(), placement: false, entity_bottom: entity.y(), entity: Some(properties) }
    }

    pub fn placement_context(level: &ServerLevel, player: Option<EntityId>) -> CollisionContext {
        match player {
            Some(player) => CollisionContext::new(level, player, true, level.entity(player).y()),
            None => CollisionContext { placement: true, ..CollisionContext::EMPTY },
        }
    }

    fn new(level: &ServerLevel, entity: EntityId, placement: bool, entity_bottom: f64) -> CollisionContext {
        let e = level.entity(entity);
        let properties = EntityProperties {
            fall_distance: e.fall_distance,
            is_falling_block: matches!(level.entities.get(&entity), Some(LevelEntity::FallingBlock(_))),
            can_walk_on_powder_snow: level.can_entity_walk_on_powder_snow(entity),
        };
        CollisionContext { descending: e.is_shift_key_down(), placement, entity_bottom, entity: Some(properties) }
    }

    pub fn is_descending(&self) -> bool {
        self.descending
    }

    pub fn is_placement(&self) -> bool {
        self.placement
    }

    pub fn is_above(&self, shape: &VoxelShape, pos: BlockPos, default_value: bool) -> bool {
        self.is_above_max_y(shape.max(Axis::Y), pos, default_value)
    }

    fn is_above_max_y(&self, max_y: f64, pos: BlockPos, default_value: bool) -> bool {
        if self.entity.is_none() && !self.placement {
            return default_value;
        }
        self.entity_bottom > pos.y as f64 + max_y - 1.0e-5f32 as f64
    }

    pub fn get_collision_shape(&self, state: BlockState, pos: BlockPos) -> &'static VoxelShape {
        if state.is_block(blocks::SCAFFOLDING) {
            self.scaffolding_collision_shape(state, pos)
        } else if state.is_block(blocks::POWDER_SNOW) {
            self.powder_snow_collision_shape(pos)
        } else {
            state.collision_voxel_shape()
        }
    }

    fn scaffolding_collision_shape(&self, state: BlockState, pos: BlockPos) -> &'static VoxelShape {
        if self.is_placement() {
            &EMPTY
        } else if self.is_above(&BLOCK, pos, true) && !self.is_descending() {
            state.collision_voxel_shape()
        } else if state.get(STABILITY_DISTANCE) != 0 && state.get(BOTTOM) && self.is_above_max_y(0.0, pos, true) {
            &SCAFFOLDING_SHAPE_UNSTABLE_BOTTOM
        } else {
            &EMPTY
        }
    }

    fn powder_snow_collision_shape(&self, pos: BlockPos) -> &'static VoxelShape {
        if !self.is_placement()
            && let Some(entity) = self.entity
        {
            if entity.fall_distance > 2.5 {
                return &POWDER_SNOW_FALLING_COLLISION_SHAPE;
            }
            if entity.is_falling_block || entity.can_walk_on_powder_snow && self.is_above(&BLOCK, pos, false) && !self.is_descending() {
                return &BLOCK;
            }
        }
        &EMPTY
    }
}
