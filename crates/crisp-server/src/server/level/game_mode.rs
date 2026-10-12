use crisp_core::{AABB, Axis, BlockPos, Direction, Vec3};
use crisp_data::BlockState;
use crisp_data::packets::play::clientbound;

use super::ServerLevel;
use super::events::{PARTICLES_AND_SOUND_DESTROY_PROGRESS, PARTICLES_DESTROY_PROGRESS};
use crate::advancements::triggers::criteria_triggers;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::player::abilities::Abilities;
use crate::world::item::ItemStack;
use crate::world::level::collision::shapes_collide;
use crate::world::level::block::{self, BlockHitResult};
use crate::world::level::{BlockGetter, update_flags};
use crate::world::{InteractionHand, InteractionResult, item};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GameType {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameType {
    pub const VALUES: [GameType; 4] = [GameType::Survival, GameType::Creative, GameType::Adventure, GameType::Spectator];

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> GameType {
        Self::VALUES.get(id as usize).copied().unwrap_or(GameType::Survival)
    }

    pub fn name(self) -> &'static str {
        match self {
            GameType::Survival => "survival",
            GameType::Creative => "creative",
            GameType::Adventure => "adventure",
            GameType::Spectator => "spectator",
        }
    }

    pub fn by_name(name: &str) -> Option<GameType> {
        Self::VALUES.into_iter().find(|g| g.name() == name)
    }

    pub fn long_display_name(self) -> crate::network::chat::Component {
        crate::network::chat::Component::translatable(&format!("gameMode.{}", self.name()))
    }

    pub fn update_player_abilities(self, abilities: &mut Abilities) {
        match self {
            GameType::Creative => {
                abilities.mayfly = true;
                abilities.instabuild = true;
                abilities.invulnerable = true;
            }
            GameType::Spectator => {
                abilities.mayfly = true;
                abilities.instabuild = false;
                abilities.invulnerable = true;
                abilities.flying = true;
            }
            GameType::Survival | GameType::Adventure => {
                abilities.mayfly = false;
                abilities.instabuild = false;
                abilities.invulnerable = false;
                abilities.flying = false;
            }
        }
        abilities.may_build = !self.is_block_placing_restricted();
    }

    pub fn is_block_placing_restricted(self) -> bool {
        matches!(self, GameType::Adventure | GameType::Spectator)
    }

    pub fn is_creative(self) -> bool {
        self == GameType::Creative
    }

    pub fn is_survival(self) -> bool {
        matches!(self, GameType::Survival | GameType::Adventure)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockBreakAction {
    StartDestroyBlock,
    AbortDestroyBlock,
    StopDestroyBlock,
}

#[derive(Default)]
pub struct ServerPlayerGameMode {
    pub game_type: GameType,
    pub previous_game_type: Option<GameType>,
    is_destroying_block: bool,
    destroy_progress_start: i32,
    destroy_pos: BlockPos,
    destroy_direction: Option<Direction>,
    game_ticks: i32,
    has_delayed_destroy: bool,
    delayed_destroy_pos: BlockPos,
    delayed_tick_start: i32,
    last_sent_state: i32,
}

pub const BLOCK_INTERACTION_RANGE: f64 = 4.5;

const GAME_EVENT_CHANGE_GAME_MODE: u8 = 3;
const INFO_UPDATE_GAME_MODE: u8 = 1 << 2;

impl ServerLevel {
    fn change_game_mode_for_player(&mut self, player: EntityId, game_type: GameType) -> bool {
        let p = self.player(player);
        if game_type == p.game_mode.game_type {
            return false;
        }
        let previous = p.game_mode.game_type;
        self.player_mut(player).set_game_mode_for_player(game_type, Some(previous));
        if self.player(player).abilities.flying && game_type != GameType::Spectator && self.is_in_range_of_ground(player) {
            self.player_mut(player).abilities.flying = false;
        }
        self.on_update_abilities(player);
        let p = self.player(player);
        let uuid = p.profile.uuid;
        for other in self.players.values() {
            other.connection.send(clientbound::PLAYER_INFO_UPDATE, |w| {
                w.u8(INFO_UPDATE_GAME_MODE);
                w.varint(1);
                w.uuid(uuid);
                w.varint(game_type.id());
            });
        }
        if game_type == GameType::Creative {
            self.player_mut(player).living.reset_current_impulse_context();
        }
        true
    }

    fn is_in_range_of_ground(&self, player: EntityId) -> bool {
        let bb = self.player(player).entity.bounding_box();
        if !self.colliders(player, &bb).is_empty() {
            return false;
        }
        let below = bb.set_min_y(bb.min_y - 1.0).set_max_y(bb.min_y);
        let colliders = self.colliders(player, &below);
        let space = if colliders.is_empty() { 1.0 } else { -shapes_collide(Axis::Y, &bb, &colliders, -1.0) };
        space < 1.0
    }

    pub fn on_update_abilities(&self, player: EntityId) {
        let p = self.player(player);
        p.connection.send(clientbound::PLAYER_ABILITIES, |w| p.abilities.write(w));
    }

    pub fn set_game_mode(&mut self, player: EntityId, game_type: GameType) -> bool {
        if !self.change_game_mode_for_player(player, game_type) {
            return false;
        }
        self.player(player).connection.send(clientbound::GAME_EVENT, |w| {
            w.u8(GAME_EVENT_CHANGE_GAME_MODE);
            w.f32(game_type.id() as f32);
        });
        if game_type == GameType::Spectator {
            self.stop_using_item(player);
        } else {
            self.set_camera(player, Some(player));
        }
        self.on_update_abilities(player);
        self.update_effect_visibility(player);
        true
    }

    pub fn is_within_block_interaction_range(&self, player: EntityId, pos: BlockPos, buffer: f64) -> bool {
        let p = self.player(player);
        let max = p.living.attributes.value(Attribute::BlockInteractionRange) + buffer;
        AABB::of_block(pos).distance_to_sqr(p.entity.eye_position()) < max * max
    }

    pub fn is_within_entity_interaction_range(&self, player: EntityId, target: EntityId, buffer: f64) -> bool {
        let p = self.player(player);
        let max = p.living.attributes.value(Attribute::EntityInteractionRange) + buffer;
        self.entity(target).bounding_box().distance_to_sqr(p.entity.eye_position()) < max * max
    }

    fn player_destroy_speed(&self, player: EntityId, state: BlockState) -> f32 {
        let p = self.player(player);
        let mut speed = p.inventory.held().map_or(1.0, |s| s.destroy_speed(state));
        if !p.entity.on_ground {
            speed /= 5.0;
        }
        speed
    }

    pub fn has_correct_tool_for_drops(&self, player: EntityId, state: BlockState) -> bool {
        !state.requires_correct_tool_for_drops() || self.player(player).inventory.held().is_some_and(|s| s.is_correct_tool_for_drops(state))
    }

    fn destroy_progress_at(&mut self, player: EntityId, state: BlockState, pos: BlockPos) -> f32 {
        match block::of(state).destroy_progress(state, self, player, pos) {
            Some(progress) => progress,
            None => self.destroy_progress(player, state),
        }
    }

    fn destroy_progress(&self, player: EntityId, state: BlockState) -> f32 {
        let destroy_speed = state.destroy_speed();
        if destroy_speed == -1.0 {
            return 0.0;
        }
        let modifier = if self.has_correct_tool_for_drops(player, state) { 30.0 } else { 100.0 };
        self.player_destroy_speed(player, state) / destroy_speed / modifier
    }

    pub fn tick_game_mode(&mut self, player: EntityId) {
        let gm = &mut self.player_mut(player).game_mode;
        gm.game_ticks += 1;
        if gm.has_delayed_destroy {
            let pos = gm.delayed_destroy_pos;
            let start = gm.delayed_tick_start;
            let state = self.block_state(pos);
            if state.is_air() {
                self.player_mut(player).game_mode.has_delayed_destroy = false;
            } else {
                let progress = self.increment_destroy_progress(player, state, pos, start);
                if progress >= 1.0 {
                    self.player_mut(player).game_mode.has_delayed_destroy = false;
                    self.destroy_block_by_player(player, pos);
                }
            }
        } else if gm.is_destroying_block {
            let pos = gm.destroy_pos;
            let state = self.block_state(pos);
            if state.is_air() {
                self.destroy_block_progress(player, pos, -1);
                let gm = &mut self.player_mut(player).game_mode;
                gm.last_sent_state = -1;
                gm.is_destroying_block = false;
            } else {
                let gm = &self.player(player).game_mode;
                let ticks = gm.game_ticks - gm.destroy_progress_start;
                let direction = gm.destroy_direction.unwrap_or(Direction::Down);
                let event = if ticks % 4 == 0 { PARTICLES_AND_SOUND_DESTROY_PROGRESS } else { PARTICLES_DESTROY_PROGRESS };
                self.increment_destroy_progress(player, state, pos, ticks);
                self.level_event(None, event, pos, direction.ordinal() as i32);
            }
        }
    }

    fn increment_destroy_progress(&mut self, player: EntityId, state: BlockState, pos: BlockPos, ticks: i32) -> f32 {
        let progress = self.destroy_progress_at(player, state, pos) * (ticks + 1) as f32;
        let sent = (progress * 10.0) as i32;
        if sent != self.player(player).game_mode.last_sent_state {
            self.destroy_block_progress(player, pos, sent);
            self.player_mut(player).game_mode.last_sent_state = sent;
        }
        progress
    }

    pub fn handle_block_break_action(&mut self, player: EntityId, pos: BlockPos, action: BlockBreakAction, direction: Direction, max_y: i32) {
        if !self.is_within_block_interaction_range(player, pos, 1.0) {
            if self.player(player).game_mode.is_destroying_block && action == BlockBreakAction::AbortDestroyBlock {
                self.abort_destroy_block(player, pos);
            }
            return;
        }
        if pos.y > max_y {
            self.send_block_update(player, pos);
            return;
        }
        match action {
            BlockBreakAction::StartDestroyBlock => {
                if self.is_under_spawn_protection(pos, player) {
                    self.send_spawn_protection_message(player, pos);
                    return;
                }
                if !self.may_interact(player, pos) {
                    self.send_block_update(player, pos);
                    return;
                }
                if self.player(player).game_mode.game_type == GameType::Creative {
                    self.destroy_and_ack(player, pos);
                    return;
                }
                let ticks = self.player(player).game_mode.game_ticks;
                self.player_mut(player).game_mode.destroy_progress_start = ticks;
                let state = self.block_state(pos);
                let mut progress = 1.0;
                if !state.is_air() {
                    block::of(state).attack(state, self, pos, player);
                    progress = self.destroy_progress_at(player, state, pos);
                }
                if !state.is_air() && progress >= 1.0 {
                    self.destroy_and_ack(player, pos);
                } else {
                    let gm = &self.player(player).game_mode;
                    if gm.is_destroying_block {
                        let old = gm.destroy_pos;
                        self.send_block_update(player, old);
                    }
                    let sent = (progress * 10.0) as i32;
                    let gm = &mut self.player_mut(player).game_mode;
                    gm.is_destroying_block = true;
                    gm.destroy_pos = pos;
                    gm.destroy_direction = Some(direction);
                    gm.last_sent_state = sent;
                    self.destroy_block_progress(player, pos, sent);
                }
            }
            BlockBreakAction::StopDestroyBlock => {
                let gm = &self.player(player).game_mode;
                if pos == gm.destroy_pos {
                    let ticks = gm.game_ticks - gm.destroy_progress_start;
                    let state = self.block_state(pos);
                    if !state.is_air() {
                        let progress = self.destroy_progress_at(player, state, pos) * (ticks + 1) as f32;
                        if progress >= 0.7 {
                            self.player_mut(player).game_mode.is_destroying_block = false;
                            self.destroy_block_progress(player, pos, -1);
                            self.destroy_and_ack(player, pos);
                            return;
                        }
                        let gm = &mut self.player_mut(player).game_mode;
                        if !gm.has_delayed_destroy {
                            gm.is_destroying_block = false;
                            gm.has_delayed_destroy = true;
                            gm.delayed_destroy_pos = pos;
                            gm.delayed_tick_start = gm.destroy_progress_start;
                        }
                    }
                }
            }
            BlockBreakAction::AbortDestroyBlock => self.abort_destroy_block(player, pos),
        }
    }

    pub fn change_destroy_direction(&mut self, player: EntityId, direction: Direction) {
        self.player_mut(player).game_mode.destroy_direction = Some(direction);
    }

    fn abort_destroy_block(&mut self, player: EntityId, pos: BlockPos) {
        let gm = &mut self.player_mut(player).game_mode;
        gm.is_destroying_block = false;
        let current = gm.destroy_pos;
        if current != pos {
            warn!("Mismatch in destroy block pos: {current:?} {pos:?}");
            self.destroy_block_progress(player, current, -1);
        }
        self.destroy_block_progress(player, pos, -1);
    }

    fn destroy_and_ack(&mut self, player: EntityId, pos: BlockPos) {
        if !self.destroy_block_by_player(player, pos) {
            self.send_block_update(player, pos);
        }
    }

    fn send_block_update(&mut self, player: EntityId, pos: BlockPos) {
        let state = self.block_state(pos);
        self.player(player).connection.send(clientbound::BLOCK_UPDATE, |p| {
            p.position(pos.x, pos.y, pos.z);
            p.varint(state.id() as i32);
        });
    }

    pub fn destroy_block_by_player(&mut self, player: EntityId, pos: BlockPos) -> bool {
        let state = self.block_state(pos);
        let behaviour = block::of(state);
        let adjusted = behaviour.player_will_destroy(self, pos, state, player);
        let mut block_entity = None;
        let fluid = crate::world::level::BlockGetter::fluid_state(self, pos);
        let changed = self.set_block_removing(pos, super::legacy_block(fluid), update_flags::UPDATE_ALL, update_flags::UPDATE_LIMIT, &mut block_entity);
        if changed {
            behaviour.destroy(self, pos, adjusted);
        }
        if self.player(player).game_mode.game_type == GameType::Creative {
            return true;
        }
        let destroyed_with = self.player(player).inventory.held().cloned();
        let can_destroy = self.has_correct_tool_for_drops(player, adjusted);
        self.mine_block_with_held_item(player, adjusted);
        if changed && can_destroy {
            self.award_stat(player, crate::stats::stats::BLOCK_MINED.get(adjusted.block().0), 1);
            self.cause_food_exhaustion(player, crate::world::food::constants::EXHAUSTION_MINE);
            block::behaviour(adjusted.block()).player_destroy(self, player, pos, adjusted, block_entity.as_ref(), destroyed_with);
        }
        true
    }

    fn mine_block_with_held_item(&mut self, player: EntityId, state: BlockState) {
        let Some(held) = self.item_by_slot(player, crisp_data::EquipmentSlot::Mainhand) else { return };
        let used_item = held.item;
        let Some(tool) = held.tool() else { return };
        let damages_tool = if used_item == crisp_data::items::SHEARS { !state.is(&crisp_data::block_tags::FIRE) } else { state.destroy_speed() != 0.0 };
        if damages_tool && tool.damage_per_block > 0 {
            self.hurt_and_break(player, crisp_data::EquipmentSlot::Mainhand, tool.damage_per_block);
        }
        self.award_stat(player, crate::stats::stats::ITEM_USED.get(used_item.0), 1);
    }

    pub fn use_item_on(&mut self, player: EntityId, hand: InteractionHand, hit: BlockHitResult) -> InteractionResult {
        let pos = hit.block_pos;
        let state = self.block_state(pos);
        let p = self.player(player);
        let item = p.item_in_hand(hand);
        let something_in_hands = p.item_in_hand(InteractionHand::MainHand).is_some() || p.item_in_hand(InteractionHand::OffHand).is_some();
        let suppress_using_block = p.is_secondary_use_active() && something_in_hands;
        let used_item_stack = item.clone().unwrap_or_else(ItemStack::empty);
        if !suppress_using_block {
            let behaviour = block::of(state);
            let held = item.clone().unwrap_or_else(|| ItemStack::new(crisp_data::items::AIR, 0));
            let result = behaviour.use_item_on(held, state, self, pos, player, hand, &hit);
            if result.consumes_action() {
                criteria_triggers::ITEM_USED_ON_BLOCK.trigger(self, player, pos, &used_item_stack);
                return result;
            }
            if result == InteractionResult::TryWithEmptyHand && hand == InteractionHand::MainHand {
                let result = behaviour.use_without_item(state, self, pos, player, &hit);
                if result.consumes_action() {
                    criteria_triggers::DEFAULT_BLOCK_USE.trigger(self, player, pos);
                    return result;
                }
            }
        }
        match item {
            Some(stack) if stack.count > 0 => {
                let used_item = stack.item;
                let result = item::use_on(self, player, hand, hit, stack);
                if let InteractionResult::Success { was_item_interaction: true, .. } = result {
                    self.award_stat(player, crate::stats::stats::ITEM_USED.get(used_item.0), 1);
                }
                if result.consumes_action() {
                    criteria_triggers::ITEM_USED_ON_BLOCK.trigger(self, player, pos, &used_item_stack);
                }
                result
            }
            _ => InteractionResult::Pass,
        }
    }

    pub fn use_item(&mut self, player: EntityId, hand: InteractionHand) -> InteractionResult {
        if self.player(player).game_mode.game_type == GameType::Spectator {
            return InteractionResult::Pass;
        }
        let Some(item_stack) = self.player(player).item_in_hand(hand) else { return InteractionResult::Pass };
        let (result, transformed) = item::use_item(self, player, hand, item_stack.clone());
        let held = self.player(player).item_in_hand(hand);
        let result_stack = match result {
            InteractionResult::Success { .. } => transformed.or(held),
            _ => held,
        };
        let duration = result_stack.as_ref().map_or(0, crate::world::entity::using_item::use_duration);
        if result_stack.as_ref() == Some(&item_stack) && duration <= 0 {
            return result;
        }
        if result == InteractionResult::Fail && duration > 0 && !self.is_using_item(player) {
            return result;
        }
        if result_stack.as_ref() != Some(&item_stack) {
            self.player_mut(player).set_item_in_hand(hand, result_stack);
        }
        if !self.is_using_item(player)
            && let Some(mut inventory_menu) = self.player_mut(player).inventory_menu.take()
        {
            self.menu_send_all_data_to_remote(&mut inventory_menu, player);
            self.restore_container_menu(player, inventory_menu);
        }
        result
    }

    pub fn destroy_block_progress(&self, breaker: EntityId, pos: BlockPos, progress: i32) {
        for p in self.players.values() {
            if p.entity.id == breaker {
                continue;
            }
            let d = Vec3::new(pos.x as f64 - p.entity.x(), pos.y as f64 - p.entity.y(), pos.z as f64 - p.entity.z());
            if d.length_sqr() < 1024.0 {
                p.connection.send(clientbound::BLOCK_DESTRUCTION, |w| {
                    w.varint(breaker);
                    w.position(pos.x, pos.y, pos.z);
                    w.u8(progress as u8);
                });
            }
        }
    }

    pub fn set_block_simple(&mut self, pos: BlockPos, state: BlockState) -> bool {
        self.set_block(pos, state, update_flags::UPDATE_ALL)
    }
}

#[cfg(test)]
mod spawn_protection_tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::blocks;

    use super::{BlockBreakAction, GameType};
    use crate::server::level::testing;
    use crate::world::level::BlockGetter;

    #[test]
    fn non_operators_cannot_break_near_spawn() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 60.0);
        level.player_mut(player).game_mode.game_type = GameType::Creative;
        let pos = BlockPos::new(0, -61, 0);
        level.handle_block_break_action(player, pos, BlockBreakAction::StartDestroyBlock, Direction::Up, 320);
        assert!(level.block_state(pos).is_air());
        let pos = BlockPos::new(0, -61, 1);
        level.spawn_protection.radius = 16;
        level.spawn_protection.dimension = crate::server::OVERWORLD.into();
        level.spawn_protection.ops = vec![12345];
        level.handle_block_break_action(player, pos, BlockBreakAction::StartDestroyBlock, Direction::Up, 320);
        assert_eq!(level.block_state(pos), blocks::GRASS_BLOCK.default_state());
        level.spawn_protection.ops.push(level.player(player).profile.uuid);
        level.handle_block_break_action(player, pos, BlockBreakAction::StartDestroyBlock, Direction::Up, 320);
        assert!(level.block_state(pos).is_air());
    }
}
