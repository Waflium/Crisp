use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::RandomSource;
use crisp_core::{AABB, BlockPos, Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_data::{BlockState, Fluid, damage_types, game_rule, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::movement::sound_event;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, base_fire_block};
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::storage::loot::{LootContext, generate};
use crate::world::level::update_flags::UPDATE_ALL;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExplosionInteraction {
    None,
    Block,
    Mob,
    Tnt,
    Trigger,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockInteraction {
    Keep,
    Destroy,
    DestroyWithDecay,
    TriggerBlock,
}

const MAX_DROPS_PER_COMBINED_STACK: i32 = 16;
const LARGE_EXPLOSION_RADIUS: f32 = 2.0;

pub struct ServerExplosion {
    pub source: Option<EntityId>,
    pub indirect_source: Option<EntityId>,
    pub damage_source: DamageSource,
    pub center: Vec3,
    pub radius: f32,
    pub fire: bool,
    pub block_interaction: BlockInteraction,
    hit_players: Vec<(EntityId, Vec3)>,
}

fn fluid_explosion_resistance(fluid: Fluid) -> f32 {
    if fluid == Fluid::Empty { 0.0 } else { 100.0 }
}

impl ServerExplosion {
    pub fn is_small(&self) -> bool {
        self.radius < LARGE_EXPLOSION_RADIUS || !self.interacts_with_blocks()
    }

    fn interacts_with_blocks(&self) -> bool {
        self.block_interaction != BlockInteraction::Keep
    }
}

impl ServerLevel {
    fn destroy_type(&self, rule: crisp_data::GameRule) -> BlockInteraction {
        if self.game_rules.get_bool(rule) { BlockInteraction::DestroyWithDecay } else { BlockInteraction::Destroy }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn explode(
        &mut self,
        source: Option<EntityId>,
        indirect_source: Option<EntityId>,
        damage_source: Option<DamageSource>,
        center: Vec3,
        radius: f32,
        fire: bool,
        interaction: ExplosionInteraction,
    ) {
        let block_interaction = match interaction {
            ExplosionInteraction::None => BlockInteraction::Keep,
            ExplosionInteraction::Block => self.destroy_type(game_rule::BLOCK_EXPLOSION_DROP_DECAY),
            ExplosionInteraction::Mob => {
                if self.game_rules.get_bool(game_rule::MOB_GRIEFING) { self.destroy_type(game_rule::MOB_EXPLOSION_DROP_DECAY) } else { BlockInteraction::Keep }
            }
            ExplosionInteraction::Tnt => self.destroy_type(game_rule::TNT_EXPLOSION_DROP_DECAY),
            ExplosionInteraction::Trigger => BlockInteraction::TriggerBlock,
        };
        let damage_source = damage_source.unwrap_or_else(|| default_damage_source(source, indirect_source));
        let mut explosion = ServerExplosion { source, indirect_source, damage_source, center, radius, fire, block_interaction, hit_players: Vec::new() };
        let block_count = self.run_explosion(&mut explosion);
        let particle = registry_id(registry::PARTICLE_TYPE, if explosion.is_small() { "minecraft:explosion" } else { "minecraft:explosion_emitter" });
        let play_sound = source.is_none_or(|s| !self.entity(s).is_silent());
        let ids: Vec<EntityId> = self.players.keys().copied().collect();
        for id in ids {
            if self.entity(id).position().distance_to_sqr(center) >= 4096.0 {
                continue;
            }
            let knockback = explosion.hit_players.iter().find(|(p, _)| *p == id).map(|(_, k)| *k);
            self.player(id).connection.send(clientbound::EXPLODE, |w| {
                w.f64(center.x);
                w.f64(center.y);
                w.f64(center.z);
                w.f32(radius);
                w.i32(block_count);
                w.bool(knockback.is_some());
                if let Some(k) = knockback {
                    w.f64(k.x);
                    w.f64(k.y);
                    w.f64(k.z);
                }
                w.varint(particle);
                w.varint(sound_event("minecraft:entity.generic.explode") + 1);
                w.varint(2);
                for (name, scaling) in [("minecraft:poof", 0.5f32), ("minecraft:smoke", 1.0)] {
                    w.varint(registry_id(registry::PARTICLE_TYPE, name));
                    w.f32(scaling);
                    w.f32(1.0);
                    w.varint(1);
                }
                w.bool(play_sound);
            });
        }
    }

    fn run_explosion(&mut self, explosion: &mut ServerExplosion) -> i32 {
        let to_blow = self.calculate_exploded_positions(explosion);
        self.hurt_entities(explosion);
        if explosion.interacts_with_blocks() {
            self.interact_with_blocks(explosion, to_blow.clone());
        }
        if explosion.fire {
            self.create_explosion_fire(&to_blow);
        }
        to_blow.len() as i32
    }

    fn calculate_exploded_positions(&mut self, explosion: &ServerExplosion) -> Vec<BlockPos> {
        let mut to_blow: Vec<BlockPos> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for xx in 0..16 {
            for yy in 0..16 {
                for zz in 0..16 {
                    if xx != 0 && xx != 15 && yy != 0 && yy != 15 && zz != 0 && zz != 15 {
                        continue;
                    }
                    let mut xd = (xx as f32 / 15.0 * 2.0 - 1.0) as f64;
                    let mut yd = (yy as f32 / 15.0 * 2.0 - 1.0) as f64;
                    let mut zd = (zz as f32 / 15.0 * 2.0 - 1.0) as f64;
                    let d = (xd * xd + yd * yd + zd * zd).sqrt();
                    xd /= d;
                    yd /= d;
                    zd /= d;
                    let mut remaining_power = explosion.radius * (0.7 + self.random.next_float() * 0.6);
                    let (mut xp, mut yp, mut zp) = (explosion.center.x, explosion.center.y, explosion.center.z);
                    while remaining_power > 0.0 {
                        let pos = BlockPos::containing(xp, yp, zp);
                        let state = self.block_state(pos);
                        let fluid = state.fluid_state();
                        if !self.is_in_world_bounds(pos) {
                            break;
                        }
                        if !(state.is_air() && fluid.fluid == Fluid::Empty) {
                            let resistance = state.block().info().explosion_resistance.max(fluid_explosion_resistance(fluid.fluid));
                            remaining_power -= (resistance + 0.3) * 0.3;
                        }
                        if remaining_power > 0.0 && seen.insert(pos) {
                            to_blow.push(pos);
                        }
                        xp += xd * 0.3f32 as f64;
                        yp += yd * 0.3f32 as f64;
                        zp += zd * 0.3f32 as f64;
                        remaining_power -= 0.22500001;
                    }
                }
            }
        }
        let hashes: Vec<i32> = to_blow.iter().map(|p| p.y.wrapping_add(p.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(p.x)).collect();
        java_hash_map_order(&hashes).into_iter().map(|i| to_blow[i]).collect()
    }

    fn seen_percent(&mut self, center: Vec3, id: EntityId) -> f32 {
        let bb = self.entity(id).bounding_box();
        let xs = 1.0 / ((bb.max_x - bb.min_x) * 2.0 + 1.0);
        let ys = 1.0 / ((bb.max_y - bb.min_y) * 2.0 + 1.0);
        let zs = 1.0 / ((bb.max_z - bb.min_z) * 2.0 + 1.0);
        let x_offset = (1.0 - (1.0 / xs).floor() * xs) / 2.0;
        let z_offset = (1.0 - (1.0 / zs).floor() * zs) / 2.0;
        if xs < 0.0 || ys < 0.0 || zs < 0.0 {
            return 0.0;
        }
        let is_player = self.is_player(id);
        let (mut hits, mut count) = (0, 0);
        let mut xx = 0.0;
        while xx <= 1.0 {
            let mut yy = 0.0;
            while yy <= 1.0 {
                let mut zz = 0.0;
                while zz <= 1.0 {
                    let x = mth::lerp(xx, bb.min_x, bb.max_x);
                    let y = mth::lerp(yy, bb.min_y, bb.max_y);
                    let z = mth::lerp(zz, bb.min_z, bb.max_z);
                    let from = Vec3::new(x + x_offset, y, z + z_offset);
                    let ctx = ClipContext { from, to: center, block: ClipBlock::Collider, fluid: ClipFluid::None, is_player };
                    if self.clip(&ctx).miss {
                        hits += 1;
                    }
                    count += 1;
                    zz += zs;
                }
                yy += ys;
            }
            xx += xs;
        }
        hits as f32 / count as f32
    }

    fn hurt_entities(&mut self, explosion: &mut ServerExplosion) {
        if explosion.radius < 1.0e-5 {
            return;
        }
        let double_radius = explosion.radius * 2.0;
        let c = explosion.center;
        let r = double_radius as f64 + 1.0;
        let area = AABB::new(
            mth::floor(c.x - r) as f64,
            mth::floor(c.y - r) as f64,
            mth::floor(c.z - r) as f64,
            mth::floor(c.x + r) as f64,
            mth::floor(c.y + r) as f64,
            mth::floor(c.z + r) as f64,
        );
        for id in self.get_entities(explosion.source, &area) {
            if self.entity_ref(id).is_none() {
                continue;
            }
            let dist = self.entity(id).position().distance_to_sqr(c).sqrt() / double_radius as f64;
            if dist > 1.0 {
                continue;
            }
            let is_tnt = matches!(self.entities.get(&id), Some(LevelEntity::PrimedTnt(_)));
            let origin = if is_tnt { self.entity(id).position() } else { self.entity(id).eye_position() };
            let direction = origin.subtract(c).normalize();
            let exposure = self.seen_percent(c, id);
            let pow = (1.0 - dist) * exposure as f64;
            let damage = ((pow * pow + pow) / 2.0 * 7.0 * double_radius as f64 + 1.0) as f32;
            self.hurt_server(id, explosion.damage_source.clone(), damage);
            if self.entity_ref(id).is_none() {
                continue;
            }
            let knockback = direction.scale((1.0 - dist) * exposure as f64);
            let e = self.entity_mut(id);
            if knockback.x.is_finite() && knockback.y.is_finite() && knockback.z.is_finite() {
                let delta = e.delta_movement.add(knockback.x, knockback.y, knockback.z);
                e.set_delta_movement(delta);
                e.needs_sync = true;
            }
            if let Some(player) = self.players.get(&id)
                && !self.is_spectator(id)
                && !(player.game_mode.game_type.is_creative() && player.abilities.flying)
            {
                explosion.hit_players.push((id, knockback));
            }
        }
    }

    fn interact_with_blocks(&mut self, explosion: &ServerExplosion, mut targets: Vec<BlockPos>) {
        let mut stacks: Vec<(BlockPos, ItemStack)> = Vec::new();
        crate::util::shuffle(&mut targets, &mut self.random);
        for pos in targets {
            let state = self.block_state(pos);
            self.on_explosion_hit(state, pos, explosion, &mut stacks);
        }
        for (pos, stack) in stacks {
            self.pop_resource(pos, stack);
        }
    }

    fn on_explosion_hit(&mut self, state: BlockState, pos: BlockPos, explosion: &ServerExplosion, stacks: &mut Vec<(BlockPos, ItemStack)>) {
        if state.is_air() || explosion.block_interaction == BlockInteraction::TriggerBlock {
            return;
        }
        let behaviour = block::of(state);
        if behaviour.drop_from_explosion() {
            let drop_experience = explosion.indirect_source.is_some_and(|e| self.is_player(e));
            let block_entity = if state.has_block_entity() { self.get_block_entity(pos).cloned() } else { None };
            behaviour.spawn_after_break(state, self, pos, None, drop_experience);
            if let Some(table) = state.block().info().loot_table {
                let radius = (explosion.block_interaction == BlockInteraction::DestroyWithDecay).then_some(explosion.radius);
                let ctx = LootContext { block_state: Some(state), tool: None, explosion_radius: radius, block_entity: block_entity.as_ref() };
                for stack in generate(table, &ctx, &mut rand::rng()) {
                    add_or_append_stack(stacks, stack, pos);
                }
            }
        }
        self.set_block(pos, crisp_data::blocks::AIR.default_state(), UPDATE_ALL);
        behaviour.was_exploded(self, pos, explosion);
    }

    fn create_explosion_fire(&mut self, targets: &[BlockPos]) {
        for &pos in targets {
            if self.random.next_int_bounded(3) == 0 && self.block_state(pos).is_air() && self.block_state(pos.below()).is_solid_render() {
                let fire = base_fire_block::get_state(self, pos);
                self.set_block(pos, fire, UPDATE_ALL);
            }
        }
    }

    pub fn is_in_world_bounds(&self, pos: BlockPos) -> bool {
        !self.chunks.is_outside_build_height(pos.y) && pos.x >= -30_000_000 && pos.z >= -30_000_000 && pos.x < 30_000_000 && pos.z < 30_000_000
    }
}

fn default_damage_source(source: Option<EntityId>, causing: Option<EntityId>) -> DamageSource {
    let ty = if source.is_some() && causing.is_some() { damage_types::PLAYER_EXPLOSION } else { damage_types::EXPLOSION };
    DamageSource { ty, causing_entity: causing, direct_entity: source, damage_source_position: None }
}

fn add_or_append_stack(stacks: &mut Vec<(BlockPos, ItemStack)>, mut stack: ItemStack, pos: BlockPos) {
    for (_, collected) in stacks.iter_mut() {
        if stack.count + collected.count <= collected.max_stack_size() && ItemStack::is_same_item_same_components(collected, &stack) {
            let delta = (collected.max_stack_size().min(MAX_DROPS_PER_COMBINED_STACK) - collected.count).min(stack.count);
            *collected = collected.copy_with_count(collected.count + delta);
            stack.count -= delta;
        }
        if stack.is_empty() {
            return;
        }
    }
    stacks.push((pos, stack));
}
