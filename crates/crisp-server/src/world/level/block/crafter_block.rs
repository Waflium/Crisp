use std::sync::{LazyLock, Mutex};

use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::{CRAFTING, FrontAndTop, ORIENTATION, TRIGGERED};
use crisp_data::{Block, BlockState};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::core::dispenser::default_dispense_item_behavior::spawn_item;
use crate::server::level::ServerLevel;
use crate::server::level::events::{PARTICLES_SHOOT_WHITE_SMOKE, SOUND_CRAFTER_CRAFT, SOUND_CRAFTER_FAIL};
use crate::world::InteractionResult;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::item::crafting::crafting_input::CraftingInput;
use crate::world::item::crafting::recipe_cache::RecipeCache;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::level::update_flags::UPDATE_CLIENTS;

const MAX_CRAFTING_TICKS: i32 = 6;
const CRAFTING_TICK_DELAY: i32 = 4;
static RECIPE_CACHE: LazyLock<Mutex<RecipeCache>> = LazyLock::new(|| Mutex::new(RecipeCache::new(10)));

pub struct CrafterBlock;

pub fn get_potential_results(level: &mut ServerLevel, input: &mut CraftingInput) -> Option<&'static RecipeHolder> {
    RECIPE_CACHE.lock().expect("recipe cache lock").get(level, input)
}

fn set_block_entity_triggered(level: &mut ServerLevel, pos: BlockPos, triggered: bool) {
    if let Some(crafter_block_entity) = level.crafter(pos) {
        crafter_block_entity.set_triggered(triggered);
    }
}

fn dispense_from(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let Some(block_entity) = level.crafter(pos) else { return };
    let mut craft_input = block_entity.as_craft_input();
    let Some(picked_recipe) = get_potential_results(level, &mut craft_input) else {
        level.level_event(None, SOUND_CRAFTER_FAIL, pos, 0);
        return;
    };
    let crafting_recipe = picked_recipe.crafting().expect("a crafting recipe");
    let results = crafting_recipe.assemble(&craft_input);
    if results.is_empty() {
        level.level_event(None, SOUND_CRAFTER_FAIL, pos, 0);
        return;
    }
    if let Some(block_entity) = level.crafter(pos) {
        block_entity.set_crafting_ticks_remaining(MAX_CRAFTING_TICKS);
    }
    level.set_block(pos, state.set(CRAFTING, true), UPDATE_CLIENTS);
    dispense_item(level, pos, &results, state);
    for remaining_item in crafting_recipe.get_remaining_items(&mut craft_input) {
        if !remaining_item.is_empty() {
            dispense_item(level, pos, &remaining_item, state);
        }
    }
    if let Some(block_entity) = level.crafter(pos) {
        block_entity.get_items_mut().iter_mut().filter(|it| !it.is_empty()).for_each(|it| it.shrink(1));
    }
    level.block_entity_set_changed(pos);
}

fn dispense_item(level: &mut ServerLevel, pos: BlockPos, results: &ItemStack, block_state: BlockState) {
    let direction = block_state.get(ORIENTATION).front();
    let into = level.get_container_at(pos.relative(direction));
    let block_entity = ContainerRef::block(level, pos);
    let mut remaining = results.copy();
    if let Some(into) = into
        && (into.single_block().is_some_and(|into_pos| level.crafter(into_pos).is_some()) || results.count > level.container_max_stack_size(into, results))
    {
        while !remaining.is_empty() {
            let copy = remaining.copy_with_count(1);
            let item_stack = level.add_item(Some(block_entity), into, copy, Some(direction.opposite()));
            if !item_stack.is_empty() {
                break;
            }
            remaining.shrink(1);
        }
    } else if let Some(into) = into {
        while !remaining.is_empty() {
            let old_size = remaining.count;
            remaining = level.add_item(Some(block_entity), into, remaining, Some(direction.opposite()));
            if old_size == remaining.count {
                break;
            }
        }
    }
    if !remaining.is_empty() {
        let center_pos = Vec3::at_center_of(pos);
        let item_spawn_offset = center_pos.relative(direction, 0.7);
        spawn_item(level, remaining, 6, direction, item_spawn_offset);
        level.level_event(None, SOUND_CRAFTER_CRAFT, pos, 0);
        level.level_event(None, PARTICLES_SHOOT_WHITE_SMOKE, pos, direction.ordinal() as i32);
    }
}

impl BlockBehaviour for CrafterBlock {
    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.crafter(pos).map_or(0, |crafter_block_entity| crafter_block_entity.get_redstone_signal())
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let should_trigger = level.has_neighbor_signal(pos);
        let is_triggered = state.get(TRIGGERED);
        if should_trigger && !is_triggered {
            level.schedule_tick(pos, state.block(), CRAFTING_TICK_DELAY);
            level.set_block(pos, state.set(TRIGGERED, true), UPDATE_CLIENTS);
            set_block_entity_triggered(level, pos, true);
        } else if !should_trigger && is_triggered {
            level.set_block(pos, state.set(TRIGGERED, false).set(CRAFTING, false), UPDATE_CLIENTS);
            set_block_entity_triggered(level, pos, false);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        dispense_from(state, level, pos);
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let nearest_looking_direction = ctx.nearest_looking_direction().opposite();
        let vertical_direction = match nearest_looking_direction {
            Direction::Down => ctx.horizontal_direction().opposite(),
            Direction::Up => ctx.horizontal_direction(),
            _ => Direction::Up,
        };
        Some(
            block
                .default_state()
                .set(ORIENTATION, FrontAndTop::from_front_and_top(nearest_looking_direction, vertical_direction))
                .set(TRIGGERED, level.has_neighbor_signal(ctx.clicked_pos())),
        )
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        if state.get(TRIGGERED) {
            level.schedule_tick(pos, state.block(), CRAFTING_TICK_DELAY);
        }
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.crafter(pos).is_some() {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
        InteractionResult::SUCCESS
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ORIENTATION, state.get(ORIENTATION).rotate(rotation.rotation()))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ORIENTATION, state.get(ORIENTATION).rotate(mirror.rotation()))
    }
}
