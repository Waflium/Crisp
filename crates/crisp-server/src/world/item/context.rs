use crisp_core::{Axis, BlockPos, Direction, Vec3};
use crisp_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, BlockHitResult};

#[derive(Clone, Copy, Debug)]
pub struct PlayerView {
    pub id: EntityId,
    pub x_rot: f32,
    pub y_rot: f32,
    pub secondary_use_active: bool,
    pub position: Vec3,
}

#[derive(Clone, Debug)]
pub struct BlockPlaceContext {
    pub player: Option<PlayerView>,
    pub hand: InteractionHand,
    pub item: ItemStack,
    pub hit: BlockHitResult,
    relative_pos: BlockPos,
    replace_clicked: bool,
}

impl BlockPlaceContext {
    pub fn new(level: &mut ServerLevel, player: Option<EntityId>, hand: InteractionHand, item: ItemStack, hit: BlockHitResult) -> BlockPlaceContext {
        let player = player.map(|id| {
            let p = level.player(id);
            PlayerView { id, x_rot: p.entity.x_rot, y_rot: p.entity.y_rot, secondary_use_active: p.is_secondary_use_active(), position: p.entity.position() }
        });
        let mut ctx = BlockPlaceContext { player, hand, item, hit, relative_pos: hit.block_pos.relative(hit.direction), replace_clicked: true };
        let clicked = level.block_state(hit.block_pos);
        ctx.replace_clicked = block::of(clicked).can_be_replaced(clicked, &ctx);
        ctx
    }

    pub fn at(&self, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> BlockPlaceContext {
        let location = Vec3::new(
            pos.x as f64 + 0.5 + direction.step_x() as f64 * 0.5,
            pos.y as f64 + 0.5 + direction.step_y() as f64 * 0.5,
            pos.z as f64 + 0.5 + direction.step_z() as f64 * 0.5,
        );
        let hit = BlockHitResult { location, direction, block_pos: pos, inside: false, world_border_hit: false };
        let mut ctx = BlockPlaceContext { player: self.player, hand: self.hand, item: self.item.clone(), hit, relative_pos: pos.relative(direction), replace_clicked: true };
        let clicked = level.block_state(pos);
        ctx.replace_clicked = block::of(clicked).can_be_replaced(clicked, &ctx);
        ctx
    }

    pub fn clicked_pos(&self) -> BlockPos {
        if self.replace_clicked { self.hit.block_pos } else { self.relative_pos }
    }

    pub fn clicked_face(&self) -> Direction {
        self.hit.direction
    }

    pub fn click_location(&self) -> Vec3 {
        self.hit.location
    }

    pub fn is_inside(&self) -> bool {
        self.hit.inside
    }

    pub fn replacing_clicked_on_block(&self) -> bool {
        self.replace_clicked
    }

    pub fn can_place(&self, level: &mut ServerLevel) -> bool {
        if self.replace_clicked {
            return true;
        }
        let state = level.block_state(self.clicked_pos());
        block::of(state).can_be_replaced(state, self)
    }

    pub fn horizontal_direction(&self) -> Direction {
        self.player.map_or(Direction::North, |p| Direction::from_y_rot(p.y_rot as f64))
    }

    pub fn is_secondary_use_active(&self) -> bool {
        self.player.is_some_and(|p| p.secondary_use_active)
    }

    pub fn rotation(&self) -> f32 {
        self.player.map_or(0.0, |p| p.y_rot)
    }

    fn ordered_by_nearest(&self) -> [Direction; 6] {
        let (x_rot, y_rot) = self.player.map_or((0.0, 0.0), |p| (p.x_rot, p.y_rot));
        Direction::ordered_by_nearest(x_rot, y_rot)
    }

    pub fn nearest_looking_direction(&self) -> Direction {
        self.ordered_by_nearest()[0]
    }

    pub fn nearest_looking_vertical_direction(&self) -> Direction {
        let (x_rot, y_rot) = self.player.map_or((0.0, 0.0), |p| (p.x_rot, p.y_rot));
        Direction::facing_axis(x_rot, y_rot, Axis::Y)
    }

    pub fn nearest_looking_directions(&self) -> [Direction; 6] {
        let mut directions = self.ordered_by_nearest();
        if self.replace_clicked {
            return directions;
        }
        let opposite = self.clicked_face().opposite();
        let index = directions.iter().position(|&d| d == opposite).unwrap_or(directions.len());
        if index > 0 && index < directions.len() {
            directions.copy_within(0..index, 1);
            directions[0] = opposite;
        }
        directions
    }

    pub fn neighbour(&self, level: &mut ServerLevel, direction: Direction) -> BlockState {
        level.block_state(self.clicked_pos().relative(direction))
    }
}
