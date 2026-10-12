use crisp_core::{BlockPos, Direction, Vec3, mth};
use crisp_data::properties::RESPAWN_ANCHOR_CHARGES;
use crisp_data::{BlockState, damage_types, fluid_tags, items};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::server::level::server_player::RespawnConfig;
use crate::world::attribute::environment_attributes;
use crate::world::damagesource::DamageSource;
use crate::world::entity::vehicle::dismount_helper::{self, DismountType};
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::server_explosion::{ExplosionDamageCalculator, ExplosionInteraction};
use crate::world::level::storage::RespawnData;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub const CHARGE: crisp_data::properties::IntegerProperty = RESPAWN_ANCHOR_CHARGES;

const RESPAWN_HORIZONTAL_OFFSETS: [(i32, i32, i32); 8] = [(0, 0, -1), (-1, 0, 0), (0, 0, 1), (1, 0, 0), (-1, 0, -1), (1, 0, -1), (-1, 0, 1), (1, 0, 1)];

fn respawn_offsets() -> impl Iterator<Item = (i32, i32, i32)> {
    RESPAWN_HORIZONTAL_OFFSETS
        .into_iter()
        .chain(RESPAWN_HORIZONTAL_OFFSETS.into_iter().map(|(x, y, z)| (x, y - 1, z)))
        .chain(RESPAWN_HORIZONTAL_OFFSETS.into_iter().map(|(x, y, z)| (x, y + 1, z)))
        .chain(std::iter::once((0, 1, 0)))
}

pub struct RespawnAnchorBlock;

fn is_respawn_fuel(item_in_hand: &ItemStack) -> bool {
    item_in_hand.is(items::GLOWSTONE)
}

fn can_be_charged(state: BlockState) -> bool {
    state.get(CHARGE) < 4
}

fn is_water_that_would_flow(pos: BlockPos, level: &mut ServerLevel) -> bool {
    let fluid = level.fluid_state(pos);
    if !fluid.is(&fluid_tags::WATER) {
        return false;
    }
    if fluid.is_source() {
        return true;
    }
    let amount = fluid.amount as f32;
    if amount < 2.0 {
        return false;
    }
    !level.fluid_state(pos.below()).is(&fluid_tags::WATER)
}

fn explode(level: &mut ServerLevel, pos: BlockPos) {
    level.remove_block(pos, false);
    let any_water_neighbors = Direction::HORIZONTAL.into_iter().any(|direction| is_water_that_would_flow(pos.relative(direction), level));
    let in_water = any_water_neighbors || level.fluid_state(pos.above()).is(&fluid_tags::WATER);
    let boom_pos = Vec3::at_center_of(pos);
    let source = DamageSource { damage_source_position: Some(boom_pos), ..DamageSource::new(damage_types::BAD_RESPAWN_POINT) };
    level.explode_with_calculator(None, None, Some(source), ExplosionDamageCalculator::RespawnAnchor { pos, in_water }, boom_pos, 5.0, true, ExplosionInteraction::Block);
}

pub fn can_set_spawn(level: &ServerLevel, pos: BlockPos) -> bool {
    level.environment_attributes().get_value_at(environment_attributes::RESPAWN_ANCHOR_WORKS, pos)
}

pub fn charge(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    let new_state = state.set(CHARGE, state.get(CHARGE) + 1);
    level.set_block(pos, new_state, UPDATE_ALL);
    level.play_sound_at(None, pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5, sound_event("minecraft:block.respawn_anchor.charge"), SoundSource::Blocks, 1.0, 1.0);
}

pub fn get_scaled_charge_level(state: BlockState, maximum: i32) -> i32 {
    mth::floor_f((state.get(CHARGE) - 0) as f32 / 4.0 * maximum as f32)
}

pub fn find_stand_up_position(entity_type: DismountType, level: &mut ServerLevel, pos: BlockPos) -> Option<Vec3> {
    find_stand_up_position_checked(entity_type, level, pos, true).or_else(|| find_stand_up_position_checked(entity_type, level, pos, false))
}

fn find_stand_up_position_checked(entity_type: DismountType, level: &mut ServerLevel, pos: BlockPos, check_dangerous: bool) -> Option<Vec3> {
    respawn_offsets().find_map(|(x, y, z)| dismount_helper::find_safe_dismount_location(entity_type, level, pos.offset(x, y, z), check_dangerous))
}

impl BlockBehaviour for RespawnAnchorBlock {
    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if is_respawn_fuel(&item) && can_be_charged(state) {
            charge(level, pos, state);
            let mut item = item;
            item.consume(1, level, Some(player));
            level.player_mut(player).update_item_in_hand(hand, &item);
            return InteractionResult::SUCCESS;
        }
        let off_hand = level.player(player).item_in_hand(InteractionHand::OffHand).unwrap_or_else(ItemStack::empty);
        if hand == InteractionHand::MainHand && is_respawn_fuel(&off_hand) && can_be_charged(state) {
            InteractionResult::Pass
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if state.get(CHARGE) == 0 {
            return InteractionResult::Pass;
        }
        if !can_set_spawn(level, pos) {
            explode(level, pos);
            return InteractionResult::SUCCESS_SERVER;
        }
        let respawn_data = RespawnData { dimension: level.dimension.to_string(), pos, yaw: 0.0, pitch: 0.0 };
        let same_position = level
            .player(player)
            .respawn_config
            .as_ref()
            .is_some_and(|config| config.respawn_data.dimension == respawn_data.dimension && config.respawn_data.pos == respawn_data.pos);
        if !same_position {
            level.set_respawn_position(player, Some(RespawnConfig { respawn_data, forced: false }), true);
            level.play_sound_at(None, pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5, sound_event("minecraft:block.respawn_anchor.set_spawn"), SoundSource::Blocks, 1.0, 1.0);
            return InteractionResult::SUCCESS_SERVER;
        }
        InteractionResult::CONSUME
    }

    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        get_scaled_charge_level(state, 15)
    }
}
