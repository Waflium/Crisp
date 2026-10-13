use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, Vec3, mth};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{BlockState, Fluid, block_tags, blocks, fluid_tags, game_rule, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::super::data::{DataValue, entity_defaults};
use super::super::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType};
use crate::network::connection::encode;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::{self, anvil_block, falling_block};
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::BlockGetter;
use crisp_data::packets::play::clientbound;

const DEFAULT_TIME: i32 = 0;
const DEFAULT_FALL_DAMAGE_PER_DISTANCE: f32 = 0.0;
const DEFAULT_MAX_FALL_DAMAGE: i32 = 40;
const DEFAULT_DROP_ITEM: bool = true;
const DEFAULT_CANCEL_DROP: bool = false;
const DATA_START_POS: usize = 8;

pub struct FallingBlockEntity {
    pub entity: Entity,
    block_state: BlockState,
    pub time: i32,
    pub drop_item: bool,
    cancel_drop: bool,
    hurt_entities: bool,
    fall_damage_max: i32,
    fall_damage_per_distance: f32,
    pub block_data: Option<Nbt>,
    pub force_tick_after_teleport_to_duplicate: bool,
}

impl FallingBlockEntity {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.98, 0.98);

    pub fn of_type() -> FallingBlockEntity {
        let mut data = entity_defaults();
        data.push(DataValue::BlockPos(BlockPos::ZERO));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:falling_block"), Self::DIMENSIONS, data);
        FallingBlockEntity {
            entity,
            block_state: blocks::SAND.default_state(),
            time: DEFAULT_TIME,
            drop_item: DEFAULT_DROP_ITEM,
            cancel_drop: DEFAULT_CANCEL_DROP,
            hurt_entities: false,
            fall_damage_max: DEFAULT_MAX_FALL_DAMAGE,
            fall_damage_per_distance: DEFAULT_FALL_DAMAGE_PER_DISTANCE,
            block_data: None,
            force_tick_after_teleport_to_duplicate: false,
        }
    }

    fn new(x: f64, y: f64, z: f64, block_state: BlockState) -> FallingBlockEntity {
        let mut falling = FallingBlockEntity::of_type();
        falling.block_state = block_state;
        let entity = &mut falling.entity;
        entity.set_pos(Vec3::new(x, y, z));
        entity.set_delta_movement(Vec3::ZERO);
        entity.xo = Vec3::new(x, y, z);
        let start_pos = entity.block_position();
        falling.set_start_pos(start_pos);
        falling
    }

    pub fn set_start_pos(&mut self, pos: BlockPos) {
        self.entity.data.set(DATA_START_POS, DataValue::BlockPos(pos));
    }

    pub fn get_start_pos(&self) -> BlockPos {
        match self.entity.data.get(DATA_START_POS) {
            DataValue::BlockPos(pos) => *pos,
            _ => unreachable!("the start pos is a block pos"),
        }
    }

    pub fn set_hurts_entities(&mut self, damage_per_distance: f32, damage_max: i32) {
        self.hurt_entities = true;
        self.fall_damage_per_distance = damage_per_distance;
        self.fall_damage_max = damage_max;
    }

    pub fn disable_drop(&mut self) {
        self.cancel_drop = true;
    }

    pub fn get_block_state(&self) -> BlockState {
        self.block_state
    }

    pub fn add_additional_save_data(&self, out: &mut Vec<(String, Nbt)>) {
        out.push(("BlockState".into(), crate::world::level::chunk::serialization::state_to_nbt(self.block_state)));
        out.push(("Time".into(), Nbt::Int(self.time)));
        out.push(("DropItem".into(), Nbt::Byte(self.drop_item as i8)));
        out.push(("HurtEntities".into(), Nbt::Byte(self.hurt_entities as i8)));
        out.push(("FallHurtAmount".into(), Nbt::Float(self.fall_damage_per_distance)));
        out.push(("FallHurtMax".into(), Nbt::Int(self.fall_damage_max)));
        if let Some(block_data) = &self.block_data {
            out.push(("TileEntityData".into(), block_data.clone()));
        }
        out.push(("CancelDrop".into(), Nbt::Byte(self.cancel_drop as i8)));
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        use crate::world::level::block::entity::{float_value, int_value};
        let flag = |key: &str| input.get(key).and_then(Nbt::as_i64).map(|v| v != 0);
        self.block_state = input.get("BlockState").and_then(crate::world::level::chunk::serialization::state_from_nbt).unwrap_or(blocks::SAND.default_state());
        self.time = input.get("Time").and_then(int_value).unwrap_or(DEFAULT_TIME);
        let default_hurt_entities = self.block_state.is(&block_tags::ANVIL);
        self.hurt_entities = flag("HurtEntities").unwrap_or(default_hurt_entities);
        self.fall_damage_per_distance = input.get("FallHurtAmount").and_then(float_value).unwrap_or(DEFAULT_FALL_DAMAGE_PER_DISTANCE);
        self.fall_damage_max = input.get("FallHurtMax").and_then(int_value).unwrap_or(DEFAULT_MAX_FALL_DAMAGE);
        self.drop_item = flag("DropItem").unwrap_or(DEFAULT_DROP_ITEM);
        self.block_data = input.get("TileEntityData").filter(|tag| matches!(tag, Nbt::Compound(_))).cloned();
        self.cancel_drop = flag("CancelDrop").unwrap_or(DEFAULT_CANCEL_DROP);
    }
}

pub fn fall(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> EntityId {
    let state = if state.has(WATERLOGGED) { state.set(WATERLOGGED, false) } else { state };
    let entity = FallingBlockEntity::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5, state);
    let id = entity.entity.id;
    let fluid = level.fluid_state(pos);
    level.set_block(pos, crate::server::level::legacy_block(fluid), crate::world::level::update_flags::UPDATE_ALL);
    level.add_fresh_entity(LevelEntity::FallingBlock(entity));
    id
}

impl ServerLevel {
    pub fn falling_block(&mut self, id: EntityId) -> &mut FallingBlockEntity {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::FallingBlock(falling)) => falling,
            _ => unreachable!("not a falling block"),
        }
    }

    pub fn tick_falling_block(&mut self, id: EntityId) {
        let falling = self.falling_block(id);
        if falling.block_state.is_air() {
            falling.entity.discard();
            return;
        }
        let block = falling.block_state.block();
        falling.time += 1;
        self.apply_gravity(id);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        self.apply_effects_from_blocks(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        self.handle_portal(id);
        let falling = self.falling_block(id);
        if !falling.entity.is_removed() || falling.force_tick_after_teleport_to_duplicate {
            let mut pos = falling.entity.block_position();
            let is_concrete = falling.block_state.block().is_instance_of("ConcretePowderBlock");
            let mut is_stuck_in_water = is_concrete && self.fluid_state(pos).is(&fluid_tags::WATER);
            let e = &self.falling_block(id).entity;
            let move_vec = e.delta_movement.length_sqr();
            if is_concrete && move_vec > 1.0 {
                let ctx = ClipContext { from: e.xo, to: e.position(), block: ClipBlock::Collider, fluid: ClipFluid::SourceOnly, is_player: false };
                let clip = self.clip(&ctx);
                if !clip.miss && self.fluid_state(clip.hit.block_pos).is(&fluid_tags::WATER) {
                    pos = clip.hit.block_pos;
                    is_stuck_in_water = true;
                }
            }
            if !self.entity(id).on_ground && !is_stuck_in_water {
                let time = self.falling_block(id).time;
                if time > 100 && (pos.y <= self.chunks.min_y() || pos.y > self.chunks.max_y()) || time > 600 {
                    if self.falling_block(id).drop_item && self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
                        self.spawn_at_location(id, ItemStack::new(block.item(), 1), 0.0);
                    }
                    self.falling_block(id).entity.discard();
                }
            } else {
                let current_state = self.block_state(pos);
                let e = self.entity_mut(id);
                e.set_delta_movement(e.delta_movement.multiply(0.7, -0.5, 0.7));
                if !current_state.is_block(blocks::MOVING_PISTON) {
                    if !self.falling_block(id).cancel_drop {
                        let ctx = BlockPlaceContext::directional(self, pos, Direction::Down, ItemStack::empty(), Direction::Up);
                        let may_replace = block::of(current_state).can_be_replaced(current_state, &ctx);
                        let would_continue_falling = falling_block::is_free(self.block_state(pos.below())) && (!is_concrete || !is_stuck_in_water);
                        let block_state = self.falling_block(id).block_state;
                        let would_survive = block::of(block_state).can_survive(block_state, self, pos) && !would_continue_falling;
                        if may_replace && would_survive {
                            if block_state.has(WATERLOGGED) && self.fluid_state(pos).fluid == Fluid::Water {
                                self.falling_block(id).block_state = block_state.set(WATERLOGGED, true);
                            }
                            let block_state = self.falling_block(id).block_state;
                            if self.set_block(pos, block_state, crate::world::level::update_flags::UPDATE_ALL) {
                                let placed = self.block_state(pos);
                                self.send_to_tracking_players(id, &encode(clientbound::BLOCK_UPDATE, |p| {
                                    p.position(pos.x, pos.y, pos.z);
                                    p.varint(placed.id() as i32);
                                }));
                                let placed = self.block_state(pos);
                                self.send_to_tracking_players(id, &encode(clientbound::ADD_TRANSIENT_BLOCK, |p| {
                                    p.position(pos.x, pos.y, pos.z);
                                    p.varint(placed.id() as i32);
                                }));
                                self.falling_block(id).entity.discard();
                                block::behaviour(block).on_land(self, pos, block_state, current_state, id);
                                let falling = self.falling_block(id);
                                if let Some(Nbt::Compound(block_data)) = falling.block_data.clone()
                                    && falling.block_state.has_block_entity()
                                    && let Some(block_entity) = self.get_block_entity(pos)
                                {
                                    let Nbt::Compound(mut merged) = block_entity.save_without_metadata() else { unreachable!("block entities save compounds") };
                                    for (name, tag) in block_data {
                                        match merged.iter_mut().find(|(k, _)| *k == name) {
                                            Some(slot) => slot.1 = tag,
                                            None => merged.push((name, tag)),
                                        }
                                    }
                                    block_entity.load_with_components(&Nbt::Compound(merged));
                                    self.block_entity_set_changed(pos);
                                }
                            } else if self.falling_block(id).drop_item && self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
                                self.falling_block(id).entity.discard();
                                self.call_on_broken_after_fall(id, block, pos);
                                self.spawn_at_location(id, ItemStack::new(block.item(), 1), 0.0);
                            }
                        } else {
                            self.falling_block(id).entity.discard();
                            if self.falling_block(id).drop_item && self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
                                self.call_on_broken_after_fall(id, block, pos);
                                self.spawn_at_location(id, ItemStack::new(block.item(), 1), 0.0);
                            }
                        }
                    } else {
                        self.falling_block(id).entity.discard();
                        self.call_on_broken_after_fall(id, block, pos);
                    }
                }
            }
        }
        let drag = self.air_drag(id) as f64;
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(drag));
        e.requires_precise_position = e.horizontal_collision || e.vertical_collision;
    }

    fn call_on_broken_after_fall(&mut self, id: EntityId, block: crisp_data::Block, pos: BlockPos) {
        block::behaviour(block).on_broken_after_fall(self, pos, id);
    }

    pub fn falling_block_hurt_server(&mut self, id: EntityId, source: DamageSource) -> bool {
        if !self.is_invulnerable_to_base(id, &source) {
            self.entity_mut(id).sync_velocity = true;
        }
        false
    }

    pub fn falling_block_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64) -> bool {
        let falling = self.falling_block(id);
        if !falling.hurt_entities {
            return false;
        }
        let fall_distance_int = mth::ceil(fall_distance - 1.0);
        if fall_distance_int < 0 {
            return false;
        }
        let block_state = falling.block_state;
        let actual_damage_source = block::of(block_state).get_fall_damage_source(id);
        let damage = mth::floor_f(fall_distance_int as f32 * falling.fall_damage_per_distance).min(falling.fall_damage_max) as f32;
        let bb = falling.entity.bounding_box();
        let targets: Vec<EntityId> = self
            .get_entities(Some(id), &bb)
            .into_iter()
            .filter(|&e| self.is_living(e) && self.is_alive(e) && !self.players.get(&e).is_some_and(|p| p.game_mode.game_type.is_creative()))
            .collect();
        for target in targets {
            self.hurt(target, actual_damage_source, damage);
        }
        let is_anvil = block_state.is(&block_tags::ANVIL);
        let falling = self.falling_block(id);
        if is_anvil && damage > 0.0 && falling.entity.random.next_float() < 0.05 + fall_distance_int as f32 * 0.05 {
            match anvil_block::damage(falling.block_state) {
                None => falling.cancel_drop = true,
                Some(new_block_state) => falling.block_state = new_block_state,
            }
        }
        false
    }
}
