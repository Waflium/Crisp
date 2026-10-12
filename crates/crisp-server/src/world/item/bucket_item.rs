use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::{Fluid, Item, fluid_tags, items, registry, registry_id};

use super::ItemStack;
use super::item_utils::{self, has_infinite_materials};
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, BlockHitResult};
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::material;
use crate::world::level::update_flags::UPDATE_LIMIT;
use crate::world::{InteractionHand, InteractionResult};

const DEFAULT_BLOCK_INTERACTION_RANGE: f64 = 4.5;
const CREATIVE_BLOCK_INTERACTION_RANGE_BONUS: f64 = 0.5;

pub struct BucketItem {
    content: Fluid,
}

impl BucketItem {
    pub fn for_item(item: Item) -> Option<BucketItem> {
        let info = item.info();
        if info.classes.first() != Some(&"BucketItem") {
            return None;
        }
        let fields: serde_json::Value = serde_json::from_str(info.fields).unwrap_or_default();
        let content = match fields["content"].as_str() {
            Some("minecraft:water") => Fluid::Water,
            Some("minecraft:lava") => Fluid::Lava,
            _ => Fluid::Empty,
        };
        Some(BucketItem { content })
    }

    pub fn use_item(&self, level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
        let Some(item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
        let fluid = if self.content == Fluid::Empty { ClipFluid::SourceOnly } else { ClipFluid::None };
        let Some(hit) = player_pov_hit_result(level, player, fluid) else { return (InteractionResult::Pass, None) };
        let pos = hit.block_pos;
        let direction = hit.direction;
        let direction_offset_pos = pos.relative(direction);
        if !(level.may_interact(player, pos) && may_use_item_at(level, player)) {
            return (InteractionResult::Fail, None);
        }
        let clicked = level.block_state(pos);
        let place_pos =
            if block::of(clicked).is_liquid_block_container(clicked) && self.content == Fluid::Water { pos } else { direction_offset_pos };
        if self.empty_contents(level, Some(player), place_pos, Some(hit)) {
            let empty_result = if has_infinite_materials(level, player) { item_stack.clone() } else { ItemStack::new(items::BUCKET, 1) };
            let result = item_utils::create_filled_result(level, player, hand, item_stack, empty_result, true);
            return (InteractionResult::SUCCESS, Some(result));
        }
        if self.content == Fluid::Empty {
            let state = level.block_state(pos);
            let behaviour = block::of(state);
            if behaviour.is_bucket_pickup(state)
                && let Some(taken) = behaviour.pickup_block(Some(player), level, pos, state)
            {
                if let Some(sound) = behaviour.pickup_sound() {
                    let position = level.player(player).entity.position();
                    level.play_sound_at(Some(player), position.x, position.y, position.z, sound, SoundSource::Players, 1.0, 1.0);
                }
                let result = item_utils::create_filled_result(level, player, hand, item_stack, taken, true);
                return (InteractionResult::SUCCESS, Some(result));
            }
        }
        (InteractionResult::Fail, None)
    }

    pub fn empty_contents(&self, level: &mut ServerLevel, user: Option<EntityId>, pos: BlockPos, hit: Option<BlockHitResult>) -> bool {
        let Some(flowing) = material::flowing(self.content) else { return false };
        let state = level.block_state(pos);
        let behaviour = block::of(state);
        let may_replace = behaviour.can_be_replaced_by_fluid(state, self.content);
        let shift_key_down = user.is_some_and(|p| level.player(p).entity.is_shift_key_down());
        let is_container = behaviour.is_liquid_block_container(state);
        let place_liquid = may_replace || is_container && behaviour.can_place_liquid(user, level, pos, state, self.content);
        let can_place_fluid_inside_block = state.is_air() || place_liquid && (!shift_key_down || hit.is_none());
        if !can_place_fluid_inside_block {
            return hit.is_some_and(|hit| self.empty_contents(level, user, hit.block_pos.relative(hit.direction), None));
        }
        if level.environment_attributes().get_value_at(environment_attributes::WATER_EVAPORATES, pos) && self.content.is(&fluid_tags::WATER) {
            let pitch = 2.6 + (level.random.next_float() - level.random.next_float()) * 0.8;
            let sound = registry_id(registry::SOUND_EVENT, "minecraft:block.fire.extinguish");
            level.play_sound(user, pos, sound, SoundSource::Blocks, 0.5, pitch);
            return true;
        }
        if is_container && self.content == Fluid::Water {
            behaviour.place_liquid(level, pos, state, flowing.source_state(false));
            self.play_empty_sound(level, user, pos);
            return true;
        }
        if may_replace && !state.liquid() {
            level.destroy_block(pos, true, None, UPDATE_LIMIT);
        }
        let fluid_block = material::create_legacy_block(flowing.source_state(false));
        if !level.set_block(pos, fluid_block, 11) && !state.fluid_state().is_source() {
            return false;
        }
        self.play_empty_sound(level, user, pos);
        true
    }

    fn play_empty_sound(&self, level: &mut ServerLevel, user: Option<EntityId>, pos: BlockPos) {
        let name = if self.content.is(&fluid_tags::LAVA) { "minecraft:item.bucket.empty_lava" } else { "minecraft:item.bucket.empty" };
        level.play_sound(user, pos, registry_id(registry::SOUND_EVENT, name), SoundSource::Blocks, 1.0, 1.0);
    }
}

fn calculate_view_vector(x_rot: f32, y_rot: f32) -> Vec3 {
    let real_x_rot = x_rot * (std::f64::consts::PI / 180.0) as f32;
    let real_y_rot = -y_rot * (std::f64::consts::PI / 180.0) as f32;
    let y_cos = mth::cos(real_y_rot as f64);
    let y_sin = mth::sin(real_y_rot as f64);
    let x_cos = mth::cos(real_x_rot as f64);
    let x_sin = mth::sin(real_x_rot as f64);
    Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
}

pub fn player_pov_hit_result(level: &mut ServerLevel, player: EntityId, fluid: ClipFluid) -> Option<BlockHitResult> {
    let p = level.player(player);
    let from = p.entity.eye_position();
    let range = DEFAULT_BLOCK_INTERACTION_RANGE + if p.game_mode.game_type == GameType::Creative { CREATIVE_BLOCK_INTERACTION_RANGE_BONUS } else { 0.0 };
    let to = from.add_vec(calculate_view_vector(p.entity.x_rot, p.entity.y_rot).scale(range));
    let result = level.clip(&ClipContext { from, to, block: ClipBlock::Outline, fluid, is_player: true });
    (!result.miss).then_some(result.hit)
}

pub fn may_use_item_at(level: &ServerLevel, player: EntityId) -> bool {
    !matches!(level.player(player).game_mode.game_type, GameType::Adventure | GameType::Spectator)
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::properties::WATERLOGGED;
    use crisp_data::{blocks, items};

    use crate::server::level::game_mode::GameType;
    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;
    use crate::world::{InteractionHand, InteractionResult};

    fn setup() -> (crate::server::level::ServerLevel, crate::world::entity::EntityId) {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 45.0);
        (level, player)
    }

    #[test]
    fn buckets_pick_up_and_place_water() {
        let (mut level, player) = setup();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::WATER.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::BUCKET, 1));
        assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
        assert!(level.block_state(pos).is_air());
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::WATER_BUCKET, 1)));

        assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
        assert_eq!(level.block_state(pos), blocks::WATER.default_state());
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::BUCKET, 1)));
    }

    #[test]
    fn empty_bucket_ignores_flowing_water_and_stacks_split() {
        let (mut level, player) = setup();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::WATER.default_state().set(crisp_data::properties::LEVEL, 1), UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::BUCKET, 3));
        assert_eq!(level.use_item(player, InteractionHand::MainHand), InteractionResult::Fail);
        level.set_block(pos, blocks::WATER.default_state(), UPDATE_ALL);
        assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
        let inventory = &level.player(player).inventory;
        assert_eq!(inventory.slots[36], Some(ItemStack::new(items::BUCKET, 2)));
        assert_eq!(inventory.slots[37], Some(ItemStack::new(items::WATER_BUCKET, 1)));
    }

    #[test]
    fn water_bucket_waterlogs_and_empty_bucket_drains() {
        let (mut level, player) = setup();
        let pos = BlockPos::new(0, -60, 0);
        let stairs = blocks::OAK_STAIRS.default_state();
        level.set_block(pos, stairs, UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::WATER_BUCKET, 1));
        assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
        assert_eq!(level.block_state(pos), stairs.set(WATERLOGGED, true));
        assert!(level.chunks.fluid_ticks.has_scheduled_tick(pos, crisp_data::Fluid::Water));
        assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
        assert_eq!(level.block_state(pos), stairs);
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::WATER_BUCKET, 1)));
    }

    #[test]
    fn creative_buckets_stay_as_they_are() {
        let (mut level, player) = setup();
        level.player_mut(player).game_mode.game_type = GameType::Creative;
        let pos = BlockPos::new(0, -60, 0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::LAVA_BUCKET, 1));
        assert!(level.use_item(player, InteractionHand::MainHand).consumes_action());
        assert_eq!(level.block_state(pos), blocks::LAVA.default_state());
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::LAVA_BUCKET, 1)));
    }
}
