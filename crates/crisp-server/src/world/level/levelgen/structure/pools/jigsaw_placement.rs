use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty, join_unoptimized};
use crisp_core::{AABB, BlockPos};

use super::alias::PoolAliasLookup;
use super::jigsaw_junction::JigsawJunction;
use super::structure_pool_element::StructurePoolElement;
use super::structure_template_pool::{EMPTY, Projection, TemplatePoolHolder};
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pool_element_structure_piece::PoolElementStructurePiece;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};
use crate::world::level::levelgen::structure::structures::Piece;
use crate::world::level::levelgen::structure::templatesystem::structure_template::{can_attach, get_front_facing};

#[derive(Clone, Copy)]
pub struct MaxDistance {
    pub horizontal: i32,
    pub vertical: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DimensionPadding {
    pub bottom: i32,
    pub top: i32,
}

impl DimensionPadding {
    pub const ZERO: DimensionPadding = DimensionPadding { bottom: 0, top: 0 };
}

type SharedShape = Rc<RefCell<Option<VoxelShape>>>;

fn aabb_of(bounding_box: &BoundingBox) -> AABB {
    AABB::new(
        bounding_box.min.x as f64,
        bounding_box.min.y as f64,
        bounding_box.min.z as f64,
        (bounding_box.max.x + 1) as f64,
        (bounding_box.max.y + 1) as f64,
        (bounding_box.max.z + 1) as f64,
    )
}

fn create(aabb: AABB) -> VoxelShape {
    if aabb.max_x - aabb.min_x < 1.0E-7 || aabb.max_y - aabb.min_y < 1.0E-7 || aabb.max_z - aabb.min_z < 1.0E-7 {
        VoxelShape::empty()
    } else {
        VoxelShape::from_boxes(&[aabb])
    }
}

pub struct JigsawSettings<'a> {
    pub start_pool: TemplatePoolHolder,
    pub start_jigsaw_name: Option<&'a str>,
    pub max_depth: i32,
    pub use_expansion_hack: bool,
    pub project_start_to_heightmap: Option<HeightmapType>,
    pub max_distance_from_center: MaxDistance,
    pub dimension_padding: DimensionPadding,
    pub apply_waterlogging: bool,
}

pub fn add_pieces(context: &mut GenerationContext, settings: JigsawSettings, position: BlockPos, pool_alias_lookup: PoolAliasLookup) -> Option<GenerationStub<'static>> {
    let center_rotation = Rotation::get_random(&mut context.random);
    let center_pool = TemplatePoolHolder::by_name(pool_alias_lookup.lookup(settings.start_pool.key())).unwrap_or(settings.start_pool);
    let center_element = center_pool.value().get_random_template(&mut context.random);
    if center_element.is_empty() {
        return None;
    }
    let anchored_position = match settings.start_jigsaw_name {
        Some(target_jigsaw_id) => {
            let anchor = center_element
                .get_shuffled_jigsaw_blocks(position, center_rotation, &mut context.random)
                .into_iter()
                .find(|jigsaw| jigsaw.name.as_deref() == Some(target_jigsaw_id))
                .map(|jigsaw| jigsaw.pos);
            match anchor {
                Some(anchor) => anchor,
                None => {
                    error!("No starting jigsaw {target_jigsaw_id} found in start pool {}", settings.start_pool.key());
                    return None;
                }
            }
        }
        None => position,
    };
    let local_anchor_position = BlockPos::new(anchored_position.x - position.x, anchored_position.y - position.y, anchored_position.z - position.z);
    let adjusted_position = BlockPos::new(position.x - local_anchor_position.x, position.y - local_anchor_position.y, position.z - local_anchor_position.z);
    let center_box = center_element.get_bounding_box(adjusted_position, center_rotation);
    let mut center_piece =
        PoolElementStructurePiece::new(center_element.clone(), adjusted_position, center_element.get_ground_level_delta(), center_rotation, center_box, settings.apply_waterlogging);
    let center_x = (center_box.max.x + center_box.min.x) / 2;
    let center_z = (center_box.max.z + center_box.min.z) / 2;
    let bottom_y = match settings.project_start_to_heightmap {
        None => adjusted_position.y,
        Some(heightmap) => {
            if !context.could_structure_exist_in_column(center_x, center_z, context.min_y, context.max_y()) {
                return None;
            }
            position.y + context.chunk_generator.get_base_height(center_x, center_z, heightmap, context.min_y, context.height)
        }
    };
    let old_absolute_ground_y = center_box.min.y + center_piece.get_ground_level_delta();
    center_piece.move_by(0, bottom_y - old_absolute_ground_y, 0);
    if is_start_too_close_to_world_height_limits(context, settings.dimension_padding, &center_piece.piece.bounding_box) {
        return None;
    }
    let center_box = center_piece.piece.bounding_box;
    let center_y = bottom_y + local_anchor_position.y;
    let max_depth = settings.max_depth;
    let use_expansion_hack = settings.use_expansion_hack;
    let max_distance_from_center = settings.max_distance_from_center;
    let dimension_padding = settings.dimension_padding;
    let apply_waterlogging = settings.apply_waterlogging;
    Some(GenerationStub {
        position: BlockPos::new(center_x, center_y, center_z),
        generator: Box::new(move |builder, context| {
            let mut pieces = vec![center_piece];
            if max_depth > 0 {
                let aabb = AABB::new(
                    (center_x - max_distance_from_center.horizontal) as f64,
                    (center_y - max_distance_from_center.vertical).max(context.min_y + dimension_padding.bottom) as f64,
                    (center_z - max_distance_from_center.horizontal) as f64,
                    (center_x + max_distance_from_center.horizontal + 1) as f64,
                    (center_y + max_distance_from_center.vertical + 1).min(context.max_y() + 1 - dimension_padding.top) as f64,
                    (center_z + max_distance_from_center.horizontal + 1) as f64,
                );
                let shape = join_unoptimized(&create(aabb), &create(aabb_of(&center_box)), BooleanOp::OnlyFirst);
                let mut placer = Placer { max_depth, pieces: &mut pieces, placing: SequencedPriorityIterator::default() };
                placer.try_placing_children(0, Rc::new(RefCell::new(Some(shape))), 0, use_expansion_hack, context, &pool_alias_lookup, apply_waterlogging);
                while let Some(state) = placer.placing.next() {
                    placer.try_placing_children(state.piece, state.free, state.depth, use_expansion_hack, context, &pool_alias_lookup, apply_waterlogging);
                }
                for piece in pieces {
                    builder.add_piece(Piece::Jigsaw(piece));
                }
            }
        }),
    })
}

fn is_start_too_close_to_world_height_limits(context: &GenerationContext, dimension_padding: DimensionPadding, center_piece_bb: &BoundingBox) -> bool {
    if dimension_padding == DimensionPadding::ZERO {
        return false;
    }
    let min_y_with_padding = context.min_y + dimension_padding.bottom;
    let max_y_with_padding = context.max_y() - dimension_padding.top;
    center_piece_bb.min.y < min_y_with_padding || center_piece_bb.max.y > max_y_with_padding
}

struct PieceState {
    piece: usize,
    free: SharedShape,
    depth: i32,
}

#[derive(Default)]
struct SequencedPriorityIterator {
    highest_prio_queue: Option<i32>,
    highest_prio: i32,
    queues_by_priority: BTreeMap<i32, VecDeque<PieceState>>,
}

impl SequencedPriorityIterator {
    fn add(&mut self, data: PieceState, priority: i32) {
        if self.highest_prio_queue.is_some() && priority == self.highest_prio {
            self.queues_by_priority.get_mut(&priority).expect("the highest queue").push_back(data);
        } else {
            self.queues_by_priority.entry(priority).or_default().push_back(data);
            if self.highest_prio_queue.is_none() || priority >= self.highest_prio {
                self.highest_prio_queue = Some(priority);
                self.highest_prio = priority;
            }
        }
    }

    fn next(&mut self) -> Option<PieceState> {
        let priority = self.highest_prio_queue?;
        let queue = self.queues_by_priority.get_mut(&priority).expect("the highest queue");
        let result = queue.pop_front()?;
        if queue.is_empty() {
            match self.queues_by_priority.iter().rev().find(|(_, queue)| !queue.is_empty()) {
                Some((&priority, _)) => {
                    self.highest_prio = priority;
                    self.highest_prio_queue = Some(priority);
                }
                None => {
                    self.highest_prio = i32::MIN;
                    self.highest_prio_queue = None;
                }
            }
        }
        Some(result)
    }
}

struct Placer<'a> {
    max_depth: i32,
    pieces: &'a mut Vec<PoolElementStructurePiece>,
    placing: SequencedPriorityIterator,
}

fn get_first_free_height(context: &GenerationContext, x: i32, z: i32) -> i32 {
    context.chunk_generator.get_base_height(x, z, HeightmapType::WorldSurfaceWg, context.min_y, context.height)
}

impl Placer<'_> {
    #[allow(clippy::too_many_arguments)]
    fn try_placing_children(
        &mut self,
        source_index: usize,
        context_free: SharedShape,
        depth: i32,
        do_expansion_hack: bool,
        context: &mut GenerationContext,
        pool_alias_lookup: &PoolAliasLookup,
        apply_waterlogging: bool,
    ) {
        let source_piece = &self.pieces[source_index];
        let source_element = source_piece.element.clone();
        let source_box_position = source_piece.position;
        let source_rotation = source_piece.rotation;
        let source_projection = source_element.get_projection();
        let source_rigid = source_projection == Projection::Rigid;
        let source_free: SharedShape = Rc::new(RefCell::new(None));
        let source_bb = source_piece.piece.bounding_box;
        let source_box_y = source_bb.min.y;
        let source_ground_level_delta = source_piece.get_ground_level_delta();
        'jigsaws: for source_jigsaw in source_element.get_shuffled_jigsaw_blocks(source_box_position, source_rotation, &mut context.random) {
            let source_direction = get_front_facing(source_jigsaw.state);
            let source_jigsaw_pos = source_jigsaw.pos;
            let target_jigsaw_pos = source_jigsaw_pos.relative(source_direction);
            let source_jigsaw_local_y = source_jigsaw_pos.y - source_box_y;
            let mut source_jigsaw_base_height = i32::MIN;
            let pool_name = pool_alias_lookup.lookup(&source_jigsaw.pool);
            let Some(target_pool) = TemplatePoolHolder::by_name(pool_name) else {
                warn!("Empty or non-existent pool: {pool_name}");
                continue;
            };
            if target_pool.value().size() == 0 && !target_pool.is(EMPTY) {
                warn!("Empty or non-existent pool: {pool_name}");
                continue;
            }
            let fallback = target_pool.value().get_fallback();
            if fallback.value().size() == 0 && !fallback.is(EMPTY) {
                warn!("Empty or non-existent fallback pool: {}", fallback.key());
                continue;
            }
            let attach_inside_source = source_bb.is_inside(target_jigsaw_pos);
            let children_free = if attach_inside_source {
                if source_free.borrow().is_none() {
                    *source_free.borrow_mut() = Some(create(aabb_of(&source_bb)));
                }
                source_free.clone()
            } else {
                context_free.clone()
            };
            let mut target_pieces: Vec<Arc<StructurePoolElement>> = Vec::new();
            if depth != self.max_depth {
                target_pieces.extend(target_pool.value().get_shuffled_templates(&mut context.random));
            }
            target_pieces.extend(fallback.value().get_shuffled_templates(&mut context.random));
            let placement_priority = source_jigsaw.placement_priority;
            for target_element in target_pieces {
                if target_element.is_empty() {
                    break;
                }
                let mut rotations = Rotation::VALUES;
                super::structure_template_pool::shuffle(&mut rotations, &mut context.random);
                for target_rotation in rotations {
                    let target_jigsaws = target_element.get_shuffled_jigsaw_blocks(BlockPos::ZERO, target_rotation, &mut context.random);
                    let hack_box = target_element.get_bounding_box(BlockPos::ZERO, target_rotation);
                    let expand_to = if do_expansion_hack && hack_box.y_span() <= 16 {
                        target_jigsaws
                            .iter()
                            .map(|target_jigsaw| {
                                if !hack_box.is_inside(target_jigsaw.pos.relative(get_front_facing(target_jigsaw.state))) {
                                    return 0;
                                }
                                let child_pool = TemplatePoolHolder::by_name(pool_alias_lookup.lookup(&target_jigsaw.pool));
                                let child_pool_size = child_pool.map_or(0, |pool| pool.value().get_max_size());
                                let child_fallback_size = child_pool.map_or(0, |pool| pool.value().get_fallback().value().get_max_size());
                                child_pool_size.max(child_fallback_size)
                            })
                            .max()
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    for target_jigsaw in &target_jigsaws {
                        if !can_attach(&source_jigsaw, target_jigsaw) {
                            continue;
                        }
                        let target_jigsaw_local_pos = target_jigsaw.pos;
                        let raw_target_box_pos = BlockPos::new(
                            target_jigsaw_pos.x - target_jigsaw_local_pos.x,
                            target_jigsaw_pos.y - target_jigsaw_local_pos.y,
                            target_jigsaw_pos.z - target_jigsaw_local_pos.z,
                        );
                        let raw_target_bb = target_element.get_bounding_box(raw_target_box_pos, target_rotation);
                        let raw_target_y = raw_target_bb.min.y;
                        let target_projection = target_element.get_projection();
                        let target_rigid = target_projection == Projection::Rigid;
                        let target_jigsaw_local_y = target_jigsaw_local_pos.y;
                        let delta_y = source_jigsaw_local_y - target_jigsaw_local_y + get_front_facing(source_jigsaw.state).step_y();
                        let target_box_y = if source_rigid && target_rigid {
                            source_box_y + delta_y
                        } else {
                            if source_jigsaw_base_height == i32::MIN {
                                source_jigsaw_base_height = get_first_free_height(context, source_jigsaw_pos.x, source_jigsaw_pos.z);
                            }
                            source_jigsaw_base_height - target_jigsaw_local_y
                        };
                        let y_offset = target_box_y - raw_target_y;
                        let mut target_bb = raw_target_bb;
                        target_bb.move_in_place(0, y_offset, 0);
                        let target_box_position = raw_target_box_pos.offset(0, y_offset, 0);
                        if expand_to > 0 {
                            let new_size = (expand_to + 1).max(target_bb.max.y - target_bb.min.y);
                            target_bb = BoundingBox::encapsulating(&target_bb, &BoundingBox::of(BlockPos::new(target_bb.min.x, target_bb.min.y + new_size, target_bb.min.z)));
                        }
                        let new_free = {
                            let free = children_free.borrow();
                            let free = free.as_ref().expect("a free shape");
                            if join_is_not_empty(free, &create(aabb_of(&target_bb).deflate_all(0.25)), BooleanOp::OnlySecond) {
                                continue;
                            }
                            join_unoptimized(free, &create(aabb_of(&target_bb)), BooleanOp::OnlyFirst)
                        };
                        *children_free.borrow_mut() = Some(new_free);
                        let target_ground_level_delta = if target_rigid { source_ground_level_delta - delta_y } else { target_element.get_ground_level_delta() };
                        let mut target_piece =
                            PoolElementStructurePiece::new(target_element.clone(), target_box_position, target_ground_level_delta, target_rotation, target_bb, apply_waterlogging);
                        let junction_y = if source_rigid {
                            source_box_y + source_jigsaw_local_y
                        } else if target_rigid {
                            target_box_y + target_jigsaw_local_y
                        } else {
                            if source_jigsaw_base_height == i32::MIN {
                                source_jigsaw_base_height = get_first_free_height(context, source_jigsaw_pos.x, source_jigsaw_pos.z);
                            }
                            source_jigsaw_base_height + delta_y / 2
                        };
                        self.pieces[source_index].junctions.push(JigsawJunction {
                            source_x: target_jigsaw_pos.x,
                            source_ground_y: junction_y - source_jigsaw_local_y + source_ground_level_delta,
                            source_z: target_jigsaw_pos.z,
                            delta_y,
                            dest_projection: target_projection,
                        });
                        target_piece.junctions.push(JigsawJunction {
                            source_x: source_jigsaw_pos.x,
                            source_ground_y: junction_y - target_jigsaw_local_y + target_ground_level_delta,
                            source_z: source_jigsaw_pos.z,
                            delta_y: -delta_y,
                            dest_projection: source_projection,
                        });
                        self.pieces.push(target_piece);
                        if depth < self.max_depth {
                            self.placing.add(PieceState { piece: self.pieces.len() - 1, free: children_free.clone(), depth: depth + 1 }, placement_priority);
                        }
                        continue 'jigsaws;
                    }
                }
            }
        }
    }
}
