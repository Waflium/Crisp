use crisp_core::random::RandomSource;
use crisp_core::{AABB, BlockPos};
use crisp_data::{BlockState, block_tags, damage_types, mob_effects, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::level::BlockGetter;

const MIN_ACTIVE_SIZE: usize = 16;
const MIN_KILL_SIZE: usize = 42;
const KILL_RANGE: f64 = 8.0;

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

#[derive(Clone, Default)]
pub struct ConduitBlockEntity {
    pub tick_count: i32,
    is_active: bool,
    is_hunting: bool,
    effect_blocks: Vec<BlockPos>,
    destroy_target: Option<EntityReference>,
    next_ambient_sound_activation: i64,
}

impl ConduitBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.destroy_target = EntityReference::read(input, "Target");
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        EntityReference::store(self.destroy_target, output, "Target");
    }

    fn update_hunting(&mut self, effect_blocks: &[BlockPos]) {
        self.set_hunting(effect_blocks.len() >= MIN_KILL_SIZE);
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn is_hunting(&self) -> bool {
        self.is_hunting
    }

    fn set_hunting(&mut self, hunting: bool) {
        self.is_hunting = hunting;
    }
}

fn get_destroy_range_aabb(world_position: BlockPos) -> AABB {
    AABB::of_block(world_position).inflate_all(KILL_RANGE)
}

impl ServerLevel {
    fn conduit(&mut self, pos: BlockPos) -> Option<&mut ConduitBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Conduit(conduit) => Some(conduit),
            _ => None,
        }
    }

    pub fn conduit_server_tick(&mut self, pos: BlockPos, state: BlockState) {
        let game_time = self.game_time;
        let Some(entity) = self.conduit(pos) else { return };
        entity.tick_count += 1;
        if game_time % 40 == 0 {
            let mut effect_blocks = std::mem::take(&mut entity.effect_blocks);
            let active = self.conduit_update_shape(pos, &mut effect_blocks);
            let Some(entity) = self.conduit(pos) else { return };
            if active != entity.is_active {
                let event = if active { sound("minecraft:block.conduit.activate") } else { sound("minecraft:block.conduit.deactivate") };
                self.play_sound(None, pos, event, SoundSource::Blocks, 1.0, 1.0);
            }
            let Some(entity) = self.conduit(pos) else { return };
            entity.is_active = active;
            entity.update_hunting(&effect_blocks);
            let active_size = effect_blocks.len();
            if active {
                self.conduit_apply_effects(pos, &effect_blocks);
            }
            let Some(entity) = self.conduit(pos) else { return };
            entity.effect_blocks = effect_blocks;
            if active {
                self.conduit_update_and_attack_target(pos, state, active_size >= MIN_KILL_SIZE);
            }
        }
        let Some(entity) = self.conduit(pos) else { return };
        if entity.is_active() {
            let next_ambient_sound_activation = entity.next_ambient_sound_activation;
            if game_time % 80 == 0 {
                self.play_sound(None, pos, sound("minecraft:block.conduit.ambient"), SoundSource::Blocks, 1.0, 1.0);
            }
            if game_time > next_ambient_sound_activation {
                let next = game_time + 60 + self.random.next_int_bounded(40) as i64;
                if let Some(entity) = self.conduit(pos) {
                    entity.next_ambient_sound_activation = next;
                }
                self.play_sound(None, pos, sound("minecraft:block.conduit.ambient.short"), SoundSource::Blocks, 1.0, 1.0);
            }
        }
    }

    fn conduit_update_shape(&mut self, world_position: BlockPos, effect_blocks: &mut Vec<BlockPos>) -> bool {
        effect_blocks.clear();
        for ox in -1..=1 {
            for oy in -1..=1 {
                for oz in -1..=1 {
                    let test_pos = world_position.offset(ox, oy, oz);
                    if !self.is_water_at(test_pos) {
                        return false;
                    }
                }
            }
        }
        for ox in -2i32..=2 {
            for oy in -2i32..=2 {
                for oz in -2i32..=2 {
                    let (ax, ay, az) = (ox.abs(), oy.abs(), oz.abs());
                    if (ax > 1 || ay > 1 || az > 1) && (ox == 0 && (ay == 2 || az == 2) || oy == 0 && (ax == 2 || az == 2) || oz == 0 && (ax == 2 || ay == 2)) {
                        let test_pos = world_position.offset(ox, oy, oz);
                        let test_block = self.block_state(test_pos);
                        if test_block.is(&block_tags::CONDUIT_EFFECT_BLOCK) {
                            effect_blocks.push(test_pos);
                        }
                    }
                }
            }
        }
        effect_blocks.len() >= MIN_ACTIVE_SIZE
    }

    fn conduit_apply_effects(&mut self, world_position: BlockPos, effect_blocks: &[BlockPos]) {
        let active_size = effect_blocks.len() as i32;
        let effect_range = active_size / 7 * 16;
        let bb = AABB::of_block(world_position).inflate_all(effect_range as f64).expand_towards(0.0, self.chunks.height() as f64, 0.0);
        let players: Vec<_> = self.get_entities(None, &bb).into_iter().filter(|&id| self.is_player(id)).collect();
        for player in players {
            if world_position.closer_than(self.entity(player).block_position(), effect_range as f64) && self.is_in_water_or_rain(player) {
                self.add_effect(player, MobEffectInstance::with(mob_effects::CONDUIT_POWER, 260, 0, true, true, true), None);
            }
        }
    }

    fn conduit_update_and_attack_target(&mut self, world_position: BlockPos, block_state: BlockState, is_active: bool) {
        let Some(entity) = self.conduit(world_position) else { return };
        let destroy_target = entity.destroy_target;
        let new_destroy_target = self.conduit_update_destroy_target(destroy_target, world_position, is_active);
        if let Some(target_entity) = EntityReference::get_living_entity(new_destroy_target, self) {
            let position = self.entity(target_entity).position();
            self.play_sound_at(None, position.x, position.y, position.z, sound("minecraft:block.conduit.attack.target"), SoundSource::Blocks, 1.0, 1.0);
            self.hurt_server(target_entity, DamageSource::new(damage_types::MAGIC), 4.0);
        }
        if new_destroy_target != destroy_target
            && let Some(entity) = self.conduit(world_position)
        {
            entity.destroy_target = new_destroy_target;
            self.send_block_updated(world_position, block_state, block_state);
        }
    }

    fn conduit_update_destroy_target(&mut self, target: Option<EntityReference>, pos: BlockPos, is_active: bool) -> Option<EntityReference> {
        if !is_active {
            return None;
        }
        let Some(target) = target else { return self.conduit_select_new_target(pos) };
        match EntityReference::get_living_entity(Some(target), self) {
            Some(target_entity) if self.is_alive(target_entity) && pos.closer_than(self.entity(target_entity).block_position(), KILL_RANGE) => Some(target),
            _ => None,
        }
    }

    fn conduit_select_new_target(&mut self, pos: BlockPos) -> Option<EntityReference> {
        let mut candidates = Vec::new();
        for id in self.get_entities(None, &get_destroy_range_aabb(pos)) {
            if self.is_living(id) && self.is_enemy(id) && self.is_in_water_or_rain(id) {
                candidates.push(id);
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let selected = candidates[self.random.next_int_bounded(candidates.len() as i32) as usize];
        Some(EntityReference::of(self.entity(selected).uuid))
    }
}
