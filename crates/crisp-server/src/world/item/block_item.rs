use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, Item, blocks};

use super::context::BlockPlaceContext;
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::level::block;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL_IMMEDIATE;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};
use crisp_protocol::nbt::Nbt;

pub trait BlockItemBehaviour: Send + Sync {
    fn block(&self) -> Block;

    fn update_placement_context(&self, _level: &mut ServerLevel, ctx: BlockPlaceContext) -> Option<BlockPlaceContext> {
        Some(ctx)
    }

    fn placement_state(&self, level: &mut ServerLevel, ctx: &BlockPlaceContext) -> Option<BlockState> {
        let block = self.block();
        let state = block::behaviour(block).get_state_for_placement(block, ctx, level)?;
        self.can_place(level, ctx, state).then_some(state)
    }

    fn can_place(&self, level: &mut ServerLevel, ctx: &BlockPlaceContext, state: BlockState) -> bool {
        let pos = ctx.clicked_pos();
        (!self.must_survive() || block::of(state).can_survive(state, level, pos)) && level.is_unobstructed(state, pos)
    }

    fn must_survive(&self) -> bool {
        true
    }

    fn place_block(&self, level: &mut ServerLevel, ctx: &BlockPlaceContext, state: BlockState) -> bool {
        level.set_block(ctx.clicked_pos(), state, UPDATE_ALL_IMMEDIATE)
    }
}

pub fn place(item: &dyn BlockItemBehaviour, level: &mut ServerLevel, ctx: BlockPlaceContext) -> InteractionResult {
    if !ctx.can_place(level) {
        return InteractionResult::Fail;
    }
    let Some(ctx) = item.update_placement_context(level, ctx) else { return InteractionResult::Fail };
    let Some(state) = item.placement_state(level, &ctx) else { return InteractionResult::Fail };
    if !item.place_block(level, &ctx, state) {
        return InteractionResult::Fail;
    }
    let pos = ctx.clicked_pos();
    let player = ctx.player.map(|p| p.id);
    let placed = level.block_state(pos);
    if placed.is_block(state.block()) {
        update_custom_block_entity_tag(level, player, pos, &ctx.item);
        update_block_entity_components(level, pos, &ctx.item);
        block::of(placed).set_placed_by(level, pos, placed, player, ctx.item);
    }
    let sound = placed.sound_type();
    level.play_sound(player, pos, sound.place_sound, SoundSource::Blocks, (sound.volume + 1.0) / 2.0, sound.pitch * 0.8);
    if let Some(player) = player {
        let p = level.player_mut(player);
        if p.game_mode.game_type != GameType::Creative {
            let slot = p.hand_slot(ctx.hand);
            if let Some(held) = &mut p.inventory.slots[slot] {
                held.count -= 1;
                if held.count <= 0 {
                    p.inventory.slots[slot] = None;
                }
            }
        }
    }
    InteractionResult::SUCCESS
}

pub fn update_custom_block_entity_tag(level: &mut ServerLevel, player: Option<EntityId>, pos: BlockPos, stack: &ItemStack) -> bool {
    let Some(custom_data) = stack.get(dc::BLOCK_ENTITY_DATA).cloned() else { return false };
    let (id, data) = values::typed_entity_data(&custom_data);
    let may_set = |level: &ServerLevel, ty: crisp_data::BlockEntityType| {
        !ty.only_op_can_set_nbt()
            || player.is_some_and(|p| level.can_use_game_master_blocks(p))
    };
    let Some(entity) = level.get_block_entity(pos) else { return false };
    let ty = entity.ty();
    if Some(ty) != crate::commands::arguments::parse_identifier(&id).and_then(|id| crisp_data::BlockEntityType::by_name(&id)) || !may_set(level, ty) {
        return false;
    }
    let entity = level.get_block_entity(pos).expect("block entity");
    let Nbt::Compound(mut tag) = entity.save_custom_only() else { return false };
    let old = Nbt::Compound(tag.clone());
    if let Nbt::Compound(fields) = &data {
        crate::nbt::merge(&mut tag, fields);
    }
    let tag = Nbt::Compound(tag);
    if tag == old {
        return false;
    }
    entity.load_custom_only(&tag);
    level.block_entity_set_changed(pos);
    true
}

fn update_block_entity_components(level: &mut ServerLevel, pos: BlockPos, stack: &ItemStack) {
    if let Some(entity) = level.get_block_entity(pos) {
        entity.apply_components_from_item_stack(stack);
        level.block_entity_set_changed(pos);
    }
}

pub struct BlockItem {
    pub block: Block,
}

impl BlockItemBehaviour for BlockItem {
    fn block(&self) -> Block {
        self.block
    }
}

pub struct StandingAndWallBlockItem {
    pub block: Block,
    pub wall_block: Block,
    pub attachment_direction: Direction,
}

impl BlockItemBehaviour for StandingAndWallBlockItem {
    fn block(&self) -> Block {
        self.block
    }

    fn placement_state(&self, level: &mut ServerLevel, ctx: &BlockPlaceContext) -> Option<BlockState> {
        let wall_state = block::behaviour(self.wall_block).get_state_for_placement(self.wall_block, ctx, level);
        let pos = ctx.clicked_pos();
        let mut chosen = None;
        for direction in ctx.nearest_looking_directions() {
            if direction == self.attachment_direction.opposite() {
                continue;
            }
            let possible = if direction == self.attachment_direction {
                block::behaviour(self.block).get_state_for_placement(self.block, ctx, level)
            } else {
                wall_state
            };
            if let Some(state) = possible
                && block::of(state).can_survive(state, level, pos)
            {
                chosen = Some(state);
                break;
            }
        }
        chosen.filter(|&state| level.is_unobstructed(state, pos))
    }
}

pub struct DoubleHighBlockItem {
    pub block: Block,
}

impl BlockItemBehaviour for DoubleHighBlockItem {
    fn block(&self) -> Block {
        self.block
    }

    fn place_block(&self, level: &mut ServerLevel, ctx: &BlockPlaceContext, state: BlockState) -> bool {
        let above: BlockPos = ctx.clicked_pos().above();
        let above_state = if level.is_water_at(above) { blocks::WATER.default_state() } else { blocks::AIR.default_state() };
        level.set_block(above, above_state, 27);
        level.set_block(ctx.clicked_pos(), state, UPDATE_ALL_IMMEDIATE)
    }
}

pub fn for_item(item: Item) -> Option<Box<dyn BlockItemBehaviour>> {
    let info = item.info();
    let block = info.block?;
    let fields: serde_json::Value = serde_json::from_str(info.fields).unwrap_or_default();
    let block_field = |name: &str| fields[name].as_str().and_then(Block::by_name);
    for class in info.classes {
        match *class {
            "StandingAndWallBlockItem" => {
                let attachment = fields["attachmentDirection"].as_str().and_then(Direction::by_name).unwrap_or(Direction::Down);
                return Some(Box::new(StandingAndWallBlockItem { block, wall_block: block_field("wallBlock")?, attachment_direction: attachment }));
            }
            "DoubleHighBlockItem" => return Some(Box::new(DoubleHighBlockItem { block })),
            "ScaffoldingBlockItem" => return Some(Box::new(super::scaffolding_block_item::ScaffoldingBlockItem { block })),
            "BlockItem" => return Some(Box::new(BlockItem { block })),
            _ => {}
        }
    }
    None
}

impl ServerLevel {
    pub fn is_unobstructed(&self, state: BlockState, pos: BlockPos) -> bool {
        let shape = crate::world::phys::shapes::collision_context::CollisionContext::placement_context(self, None).get_collision_shape(state, pos);
        if shape.is_empty() {
            return true;
        }
        let boxes: Vec<crisp_core::AABB> = shape.to_aabbs().iter().map(|b| b.move_by(pos.x as f64, pos.y as f64, pos.z as f64)).collect();
        let bounds = boxes.iter().skip(1).fold(boxes[0], |a, b| a.minmax(b));
        !self.get_entities(None, &bounds).into_iter().any(|id| {
            let bb = self.entity(id).bounding_box();
            self.blocks_building(id) && boxes.iter().any(|b| b.intersects(&bb))
        })
    }

    pub fn is_water_at(&mut self, pos: BlockPos) -> bool {
        self.fluid_state(pos).is(&crisp_data::fluid_tags::WATER)
    }
}
