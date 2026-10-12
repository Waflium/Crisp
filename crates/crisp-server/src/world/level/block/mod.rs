mod abstract_cauldron_block;
mod abstract_furnace_block;
pub mod anvil_block;
mod bonemealable_feature_placer_block;
mod dragon_egg_block;
mod barrier_block;
mod budding_amethyst_block;
mod grass_block;
mod hay_block;
mod mangrove_roots_block;
mod path_block;
mod pumpkin_block;
mod waterlogged_transparent_block;
mod weathering_copper_full_block;
mod weathering_copper_grate_block;
mod honey_block;
mod magma_block;
mod netherrack_block;
mod powder_snow_block;
pub mod respawn_anchor_block;
mod rooted_dirt_block;
mod slime_block;
mod web_block;
pub mod falling_block;
mod bamboo_sapling_block;
mod bamboo_stalk_block;
mod barrel_block;
pub mod beehive_block;
pub mod chest_block;
mod dispenser_block;
pub mod entity;
mod hopper_block;
pub mod shulker_box_block;
pub mod base_fire_block;
mod base_pressure_plate_block;
mod base_rail_block;
mod base_torch_block;
mod block_set_type;
mod bubble_column_block;
mod button_block;
pub mod campfire_block;
pub mod abstract_bed_block;
pub mod abstract_candle_block;
mod cake_block;
mod comparator_block;
pub mod composter_block;
pub mod beacon_block;
mod conduit_block;
mod daylight_detector_block;
mod copper_bulb_block;
mod jukebox_block;
pub mod candle_block;
pub mod candle_cake_block;
mod carpet_block;
mod cauldron_block;
mod cartography_table_block;
mod carved_pumpkin_block;
mod chain_block;
mod concrete_powder_block;
mod cross_collision_block;
mod detector_rail_block;
mod diode_block;
pub mod door_block;
mod drop_experience_block;
mod end_rod_block;
mod face_attached_horizontal_directional_block;
mod fence_block;
mod fence_gate_block;
mod frosted_ice_block;
mod glazed_terracotta_block;
mod grindstone_block;
mod ice_block;
mod iron_bars_block;
mod ladder_block;
mod lantern_block;
mod lever_block;
mod light_block;
mod lightning_rod_block;
pub mod liquid_block;
mod loom_block;
mod note_block;
mod observer_block;
mod piston_head_block;
mod powered_block;
mod powered_rail_block;
mod pressure_plate_block;
mod rail_block;
mod rail_state;
mod red_stone_ore_block;
mod redstone_lamp_block;
#[cfg(test)]
mod redstone_tests;
#[cfg(test)]
mod rotation_tests;
mod redstone_torch_block;
mod redstone_wall_torch_block;
pub mod redstone_wire_block;
mod repeater_block;
mod rotated_pillar_block;
pub mod scaffolding_block;
mod sculk_catalyst_block;
mod sculk_sensor_block;
mod sculk_shrieker_block;
pub mod simple_waterlogged_block;
mod slab_block;
mod smithing_table_block;
mod snow_layer_block;
mod soul_fire_block;
mod spawner_block;
mod sponge_block;
mod stair_block;
mod stonecutter_block;
mod target_block;
pub mod tnt_block;
mod torch_block;
mod trap_door_block;
mod trip_wire_block;
mod trip_wire_hook_block;
mod wall_block;
mod wall_torch_block;
mod weathering_copper_bars_block;
mod weathering_copper_chain_block;
mod weathering_copper_door_block;
mod weathering_copper_slab_block;
mod weathering_copper_stair_block;
mod weathering_copper_trap_door_block;
pub mod weathering_copper;
mod weathering_lantern_block;
mod weathering_lightning_rod_block;
mod weighted_pressure_plate_block;
mod wet_sponge_block;
mod attached_stem_block;
mod azalea_block;
mod beetroot_block;
mod big_dripleaf_block;
mod big_dripleaf_stem_block;
mod bonemealable_block;
mod bush_block;
mod cactus_block;
mod cactus_flower_block;
mod cave_vines;
mod cave_vines_block;
mod cave_vines_plant_block;
mod cocoa_block;
mod crop_block;
pub mod double_plant_block;
mod dry_vegetation_block;
mod farmland_block;
mod fire_block;
mod firefly_bush_block;
mod flower_bed_block;
mod flower_block;
mod flower_pot_block;
mod growing_plant_block;
mod growing_plant_body_block;
mod growing_plant_head_block;
mod glow_lichen_block;
mod hanging_moss_block;
mod hanging_roots_block;
mod kelp_block;
mod kelp_plant_block;
mod leaf_litter_block;
pub mod leaves_block;
mod lily_pad_block;
mod mangrove_leaves_block;
mod mangrove_propagule_block;
pub mod mirror;
pub mod mossy_carpet_block;
pub mod rotation;
pub mod multiface_block;
pub mod multiface_spreader;
mod mushroom_block;
mod nether_fungus_block;
mod nether_roots_block;
mod nether_sprouts_block;
mod nether_vines;
mod nether_wart_block;
mod nylium_block;
mod pitcher_crop_block;
mod sapling_block;
pub mod grower;
pub mod sculk_behaviour;
pub mod sculk_spreader;
mod sculk_vein_block;
mod sea_pickle_block;
mod seagrass_block;
mod segmentable_block;
mod short_dry_grass_block;
mod small_dripleaf_block;
mod snowy_block;
mod spore_blossom_block;
mod spreading_snowy_block;
mod stem_block;
mod sugar_cane_block;
mod sweet_berry_bush_block;
mod tall_dry_grass_block;
mod tall_flower_block;
mod tall_grass_block;
mod tall_seagrass_block;
mod torchflower_crop_block;
mod twisting_vines_block;
mod twisting_vines_plant_block;
mod vegetation_block;
pub mod vine_block;
mod weeping_vines_block;
mod weeping_vines_plant_block;
mod wither_rose_block;
mod amethyst_cluster_block;
mod banner_block;
mod base_coral_plant_type_block;
mod base_coral_wall_fan_block;
mod coral_block;
mod frogspawn_block;
mod sniffer_egg_block;
mod turtle_egg_block;
mod chorus_flower_block;
mod chorus_plant_block;
mod pipe_block;
mod coral_fan_block;
mod coral_plant_block;
mod coral_wall_fan_block;
mod bell_block;
mod calibrated_sculk_sensor_block;
mod ceiling_hanging_sign_block;
mod chiseled_book_shelf_block;
mod command_block;
mod copper_golem_statue_block;
mod crafter_block;
mod creaking_heart_block;
mod decorated_pot_block;
mod end_portal_frame_block;
mod ender_chest_block;
mod horizontal_directional_block;
mod huge_mushroom_block;
mod infested_rotated_pillar_block;
mod jigsaw_block;
mod lectern_block;
mod moving_piston_block;
mod nether_portal_block;
mod piston_base_block;
mod pointed_dripstone_block;
mod rod_block;
mod shelf_block;
mod speleothem_block;
#[cfg(test)]
mod speleothem_tests;
mod sulfur_spike_block;
mod skull_block;
mod standing_sign_block;
mod vault_block;
mod wall_banner_block;
mod wall_hanging_sign_block;
mod wall_sign_block;
mod wall_skull_block;

use std::sync::LazyLock;

use growing_plant_body_block::GrowingPlantBodyBlock;
use growing_plant_head_block::GrowingPlantHeadBlock;
use mirror::Mirror;
use rotation::Rotation;

use crisp_core::{AABB, BlockPos, Direction, Vec3};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState, Fluid, FluidState};

use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::biome::Precipitation;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::{InteractionHand, InteractionResult};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHitResult {
    pub location: Vec3,
    pub direction: Direction,
    pub block_pos: BlockPos,
    pub inside: bool,
    pub world_border_hit: bool,
}

#[allow(unused_variables)]
pub trait BlockBehaviour: Send + Sync {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state())
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        state.can_be_replaced() && (ctx.item.is_empty() || ctx.item.item != state.block().item())
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, placer: Option<EntityId>, item: ItemStack) {}

    fn get_clone_item_stack(&self, level: &ServerLevel, pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        ItemStack::new(state.block().item(), 1)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        state
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state
    }

    fn update_indirect_neighbour_shapes(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, flags: i32, update_limit: i32) {}

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {}

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {}

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {}

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hit: &BlockHitResult) -> InteractionResult {
        InteractionResult::Pass
    }

    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        hit: &BlockHitResult,
    ) -> InteractionResult {
        InteractionResult::TryWithEmptyHand
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        true
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {}

    fn drop_from_explosion(&self) -> bool {
        true
    }

    fn was_exploded(&self, level: &mut ServerLevel, pos: BlockPos, explosion: &crate::world::level::server_explosion::ServerExplosion) {}

    fn handle_precipitation(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, precipitation: Precipitation) {}

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {}

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        block_player_will_destroy(level, pos, state, player)
    }

    fn destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {}

    fn should_changed_state_keep_block_entity(&self, old_state: BlockState) -> bool {
        false
    }

    fn trigger_event(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        false
    }

    fn get_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        self.own_signal(state, level, pos)
    }

    fn own_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> i32 {
        0
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        0
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        0
    }

    fn attack(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId) {}

    fn spawn_after_break(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {}

    fn get_drops(&self, state: BlockState, params: LootParams) -> Vec<ItemStack> {
        block_get_drops(state, params)
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, state: BlockState, block_entity: Option<&entity::BlockEntity>, tool: Option<ItemStack>) {
        level.drop_resources_with_entity(state, pos, block_entity, Some(player), tool);
    }

    fn entity_inside(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        entity: EntityId,
        effects: &mut crate::world::entity::inside_block::InsideBlockEffects,
        is_precise: bool,
    ) {
    }

    fn step_on(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, entity: EntityId) {}

    fn destroy_progress(&self, state: BlockState, level: &mut ServerLevel, player: EntityId, pos: BlockPos) -> Option<f32> {
        None
    }

    fn should_redstone_wire_connect_to(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Option<Direction>) -> bool {
        state.is_signal_source() && direction.is_some()
    }

    fn can_be_replaced_by_fluid(&self, state: BlockState, fluid: Fluid) -> bool {
        state.can_be_replaced() || !state.is_solid()
    }

    fn is_liquid_block_container(&self, state: BlockState) -> bool {
        state.has(WATERLOGGED)
    }

    fn can_place_liquid(&self, player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState, fluid: Fluid) -> bool {
        simple_waterlogged_block::can_place_liquid(fluid)
    }

    fn place_liquid(&self, level: &mut dyn LevelAccessor, pos: BlockPos, state: BlockState, fluid: FluidState) -> bool {
        simple_waterlogged_block::place_liquid(level, pos, state, fluid)
    }

    fn is_bucket_pickup(&self, state: BlockState) -> bool {
        state.has(WATERLOGGED)
    }

    fn pickup_block(&self, player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Option<ItemStack> {
        simple_waterlogged_block::pickup_block(level, pos, state)
    }

    fn pickup_sound(&self) -> Option<i32> {
        crate::world::level::material::pickup_sound(Fluid::Water)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        false
    }

    fn is_bonemeal_success(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        false
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {}

    fn fall_on(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos, entity: EntityId, fall_distance: f64) {
        block_fall_on(level, state, pos, entity, fall_distance);
    }

    fn bounce_on(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos, entity: EntityId, fall_distance: f64) {}

    fn entity_inside_collision_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId) -> Option<Vec<AABB>> {
        None
    }

    fn can_receive_stalactite_drip(&self, fluid: Fluid) -> bool {
        false
    }

    fn receive_stalactite_drip(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, fluid: Fluid) {}

    fn on_land(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, replaced_block: BlockState, entity: EntityId) {}

    fn on_broken_after_fall(&self, level: &mut ServerLevel, pos: BlockPos, entity: EntityId) {}

    fn get_fall_damage_source(&self, entity: EntityId) -> crate::world::damagesource::DamageSource {
        crate::world::damagesource::DamageSource::entity(crisp_data::damage_types::FALLING_BLOCK, entity)
    }
}

pub fn block_fall_on(level: &mut ServerLevel, state: BlockState, _pos: BlockPos, entity: EntityId, fall_distance: f64) {
    let reduced = fall_distance * (1.0f32 - state.block().info().fall_distance_reduction) as f64;
    level.cause_fall_damage(entity, reduced, 1.0, crate::world::damagesource::DamageSource::new(crisp_data::damage_types::FALL));
}

pub fn push_entities_up(state: BlockState, new_state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    use crisp_core::Axis;
    use crisp_core::shapes::{BooleanOp, join_unoptimized};
    let offset_shape = join_unoptimized(state.collision_voxel_shape(), new_state.collision_voxel_shape(), BooleanOp::OnlySecond).moved(pos.x as f64, pos.y as f64, pos.z as f64);
    if offset_shape.is_empty() {
        return new_state;
    }
    let bounds = AABB::new(
        offset_shape.min(Axis::X),
        offset_shape.min(Axis::Y),
        offset_shape.min(Axis::Z),
        offset_shape.max(Axis::X),
        offset_shape.max(Axis::Y),
        offset_shape.max(Axis::Z),
    );
    let colliders = [crate::world::level::collision::Collider { shape: &offset_shape, offset: [0.0; 3] }];
    for colliding_entity in level.get_entities(None, &bounds) {
        let moved = level.entity(colliding_entity).bounding_box().move_by(0.0, 1.0, 0.0);
        let offset = crate::world::level::collision::shapes_collide(Axis::Y, &moved, &colliders, -1.0);
        level.teleport_relative(colliding_entity, 0.0, 1.0 + offset, 0.0);
    }
    new_state
}

pub fn block_player_will_destroy(level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
    level.level_event(Some(player), crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
    state
}

pub fn block_get_drops(state: BlockState, mut params: LootParams) -> Vec<ItemStack> {
    let Some(loot_table) = state.block().info().loot_table else { return Vec::new() };
    params.block_state = Some(state);
    LootTable::get(loot_table).get_random_items(params)
}

pub fn get_drops(
    state: BlockState,
    level: &mut ServerLevel,
    pos: BlockPos,
    block_entity: Option<&entity::BlockEntity>,
    breaker: Option<EntityId>,
    tool: &ItemStack,
) -> Vec<ItemStack> {
    let mut params = LootParams::new(level);
    params.origin = Some(Vec3::at_center_of(pos));
    params.tool = Some(tool);
    params.this_entity = breaker;
    params.block_entity = block_entity;
    of(state).get_drops(state, params)
}

#[allow(clippy::too_many_arguments)]
pub fn drop_from_block_interact_loot_table(
    level: &mut ServerLevel,
    key: &str,
    interacted_block_pos: BlockPos,
    interacted_block_state: BlockState,
    interacted_block_entity: Option<&entity::BlockEntity>,
    tool: Option<&ItemStack>,
    interacting_entity: Option<EntityId>,
    mut consumer: impl FnMut(&mut ServerLevel, ItemStack),
) -> bool {
    let loot_table = LootTable::get(key);
    let mut params = LootParams::new(level);
    params.block_state = Some(interacted_block_state);
    params.origin = Some(Vec3::at_center_of(interacted_block_pos));
    params.block_entity = interacted_block_entity;
    params.interacting_entity = interacting_entity;
    params.tool = tool;
    let drops = loot_table.get_random_items(params);
    if drops.is_empty() {
        return false;
    }
    for stack in drops {
        consumer(level, stack);
    }
    true
}

pub fn is_exception_for_connection(state: BlockState) -> bool {
    use crisp_data::blocks;
    state.block().is_instance_of("LeavesBlock")
        || state.is_block(blocks::BARRIER)
        || state.is_block(blocks::CARVED_PUMPKIN)
        || state.is_block(blocks::JACK_O_LANTERN)
        || state.is_block(blocks::MELON)
        || state.is_block(blocks::PUMPKIN)
        || state.is(&crisp_data::block_tags::SHULKER_BOXES)
}

pub fn is_diode(state: BlockState) -> bool {
    state.block().is_instance_of("DiodeBlock")
}

pub fn can_support_rigid_block(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    level.block_state(pos).is_face_sturdy(Direction::Up, crisp_data::SupportType::Rigid)
}

pub fn can_support_center(level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> bool {
    let state = level.block_state(pos);
    (direction != Direction::Down || !state.is(&crisp_data::block_tags::UNSTABLE_BOTTOM_CENTER))
        && state.is_face_sturdy(direction, crisp_data::SupportType::Center)
}

pub struct PlainBlock;

impl BlockBehaviour for PlainBlock {}

static BEHAVIOURS: LazyLock<Vec<Box<dyn BlockBehaviour>>> = LazyLock::new(|| (0..Block::count()).map(|id| create(Block(id as u16))).collect());

fn create(block: Block) -> Box<dyn BlockBehaviour> {
    for class in block.info().classes {
        if let Some(behaviour) = for_class(class, block) {
            return behaviour;
        }
    }
    Box::new(PlainBlock)
}

fn for_class(class: &str, block: Block) -> Option<Box<dyn BlockBehaviour>> {
    Some(match class {
        "CraftingTableBlock" => Box::new(CraftingTableBlock),
        "BambooSaplingBlock" => Box::new(bamboo_sapling_block::BambooSaplingBlock),
        "BambooStalkBlock" => Box::new(bamboo_stalk_block::BambooStalkBlock),
        "CandleCakeBlock" => Box::new(candle_cake_block::CandleCakeBlock),
        "CandleBlock" => Box::new(candle_block::CandleBlock),
        "BedBlock" => Box::new(abstract_bed_block::AbstractBedBlock::new(abstract_bed_block::BedKind::Bed)),
        "RespawnAnchorBlock" => Box::new(respawn_anchor_block::RespawnAnchorBlock),
        "ComparatorBlock" => Box::new(comparator_block::ComparatorBlock { block }),
        "ComposterBlock" => Box::new(composter_block::ComposterBlock),
        "ConduitBlock" => Box::new(conduit_block::ConduitBlock),
        "BeaconBlock" => Box::new(beacon_block::BeaconBlock),
        "DaylightDetectorBlock" => Box::new(daylight_detector_block::DaylightDetectorBlock),
        "JukeboxBlock" => Box::new(jukebox_block::JukeboxBlock),
        "StrawBedBlock" => Box::new(abstract_bed_block::AbstractBedBlock::new(abstract_bed_block::BedKind::StrawBed)),
        "CakeBlock" => Box::new(cake_block::CakeBlock),
        "PistonHeadBlock" => Box::new(piston_head_block::PistonHeadBlock),
        "LightBlock" => Box::new(light_block::LightBlock),
        "BeehiveBlock" => Box::new(beehive_block::BeehiveBlock),
        "ChestBlock" => Box::new(chest_block::ChestBlock::new(block)),
        "BarrelBlock" => Box::new(barrel_block::BarrelBlock),
        "ShulkerBoxBlock" => Box::new(shulker_box_block::ShulkerBoxBlock),
        "AbstractFurnaceBlock" => Box::new(abstract_furnace_block::AbstractFurnaceBlock),
        "HopperBlock" => Box::new(hopper_block::HopperBlock),
        "DispenserBlock" => Box::new(dispenser_block::DispenserBlock::new(block)),
        "CauldronBlock" => Box::new(cauldron_block::CauldronBlock),
        "FireBlock" => Box::new(fire_block::FireBlock),
        "TntBlock" => Box::new(tnt_block::TntBlock),
        "SoulFireBlock" => Box::new(soul_fire_block::SoulFireBlock),
        "LayeredCauldronBlock" => Box::new(cauldron_block::LayeredCauldronBlock::new(block)),
        "LavaCauldronBlock" => Box::new(cauldron_block::LavaCauldronBlock),
        "DropExperienceBlock" => Box::new(drop_experience_block::DropExperienceBlock::new(block)),
        "RedStoneOreBlock" => Box::new(red_stone_ore_block::RedStoneOreBlock),
        "SpawnerBlock" => Box::new(spawner_block::SpawnerBlock),
        "SculkSensorBlock" => Box::new(sculk_sensor_block::SculkSensorBlock),
        "SculkShriekerBlock" => Box::new(sculk_shrieker_block::SculkShriekerBlock),
        "SculkCatalystBlock" => Box::new(sculk_catalyst_block::SculkCatalystBlock),

        "LiquidBlock" => Box::new(liquid_block::LiquidBlock::new(block)),
        "BubbleColumnBlock" => Box::new(bubble_column_block::BubbleColumnBlock),
        "SpongeBlock" => Box::new(sponge_block::SpongeBlock),
        "WetSpongeBlock" => Box::new(wet_sponge_block::WetSpongeBlock),
        "IceBlock" => Box::new(ice_block::IceBlock),
        "FrostedIceBlock" => Box::new(frosted_ice_block::FrostedIceBlock),
        "ConcretePowderBlock" => Box::new(concrete_powder_block::ConcretePowderBlock::new(block)),
        "AnvilBlock" => Box::new(anvil_block::AnvilBlock),
        "DragonEggBlock" => Box::new(dragon_egg_block::DragonEggBlock),
        "FallingBlock" => Box::new(falling_block::FallingBlock),
        "GrassBlock" => Box::new(grass_block::GrassBlock::new(block)),
        "BonemealableFeaturePlacerBlock" => Box::new(bonemealable_feature_placer_block::BonemealableFeaturePlacerBlock::new(block)),
        "RootedDirtBlock" => Box::new(rooted_dirt_block::RootedDirtBlock),
        "NetherrackBlock" => Box::new(netherrack_block::NetherrackBlock),
        "MagmaBlock" => Box::new(magma_block::MagmaBlock),
        "SlimeBlock" => Box::new(slime_block::SlimeBlock),
        "HoneyBlock" => Box::new(honey_block::HoneyBlock),
        "WebBlock" => Box::new(web_block::WebBlock),
        "PowderSnowBlock" => Box::new(powder_snow_block::PowderSnowBlock),
        "HayBlock" => Box::new(hay_block::HayBlock),
        "WeatheringCopperFullBlock" => Box::new(weathering_copper_full_block::WeatheringCopperFullBlock),
        "CopperBulbBlock" => Box::new(copper_bulb_block::CopperBulbBlock),
        "WeatheringCopperBulbBlock" => Box::new(copper_bulb_block::WeatheringCopperBulbBlock),
        "WeatheringCopperGrateBlock" => Box::new(weathering_copper_grate_block::WeatheringCopperGrateBlock),
        "WaterloggedTransparentBlock" => Box::new(waterlogged_transparent_block::WaterloggedTransparentBlock),
        "MangroveRootsBlock" => Box::new(mangrove_roots_block::MangroveRootsBlock),
        "BarrierBlock" => Box::new(barrier_block::BarrierBlock),
        "PathBlock" => Box::new(path_block::PathBlock::new(block)),
        "BuddingAmethystBlock" => Box::new(budding_amethyst_block::BuddingAmethystBlock),
        "PumpkinBlock" => Box::new(pumpkin_block::PumpkinBlock),
        "CarpetBlock" => Box::new(carpet_block::CarpetBlock),
        "CartographyTableBlock" => Box::new(cartography_table_block::CartographyTableBlock),
        "CarvedPumpkinBlock" => Box::new(carved_pumpkin_block::CarvedPumpkinBlock),
        "ChainBlock" => Box::new(chain_block::ChainBlock),
        "DoorBlock" => Box::new(door_block::DoorBlock::new(block)),
        "EndRodBlock" => Box::new(end_rod_block::EndRodBlock),
        "FaceAttachedHorizontalDirectionalBlock" => Box::new(face_attached_horizontal_directional_block::FaceAttachedHorizontalDirectionalBlock),
        "FenceBlock" => Box::new(fence_block::FenceBlock),
        "FenceGateBlock" => Box::new(fence_gate_block::FenceGateBlock::new(block)),
        "GlazedTerracottaBlock" => Box::new(glazed_terracotta_block::GlazedTerracottaBlock),
        "GrindstoneBlock" => Box::new(grindstone_block::GrindstoneBlock),
        "IronBarsBlock" => Box::new(iron_bars_block::IronBarsBlock),
        "LadderBlock" => Box::new(ladder_block::LadderBlock),
        "LanternBlock" => Box::new(lantern_block::LanternBlock),
        "LightningRodBlock" => Box::new(lightning_rod_block::LightningRodBlock),
        "LoomBlock" => Box::new(loom_block::LoomBlock),
        "RotatedPillarBlock" => Box::new(rotated_pillar_block::RotatedPillarBlock),
        "ScaffoldingBlock" => Box::new(scaffolding_block::ScaffoldingBlock),
        "SlabBlock" => Box::new(slab_block::SlabBlock),
        "SmithingTableBlock" => Box::new(smithing_table_block::SmithingTableBlock),
        "SnowLayerBlock" => Box::new(snow_layer_block::SnowLayerBlock),
        "StairBlock" => Box::new(stair_block::StairBlock),
        "StonecutterBlock" => Box::new(stonecutter_block::StonecutterBlock),
        "TrapDoorBlock" => Box::new(trap_door_block::TrapDoorBlock::new(block)),
        "WallBlock" => Box::new(wall_block::WallBlock),
        "WeatheringCopperBarsBlock" => Box::new(weathering_copper_bars_block::WeatheringCopperBarsBlock),
        "WeatheringCopperChainBlock" => Box::new(weathering_copper_chain_block::WeatheringCopperChainBlock),
        "WeatheringCopperDoorBlock" => Box::new(weathering_copper_door_block::WeatheringCopperDoorBlock::new(block)),
        "WeatheringCopperSlabBlock" => Box::new(weathering_copper_slab_block::WeatheringCopperSlabBlock),
        "WeatheringCopperStairBlock" => Box::new(weathering_copper_stair_block::WeatheringCopperStairBlock),
        "WeatheringCopperTrapDoorBlock" => Box::new(weathering_copper_trap_door_block::WeatheringCopperTrapDoorBlock::new(block)),
        "WeatheringLanternBlock" => Box::new(weathering_lantern_block::WeatheringLanternBlock),
        "WeatheringLightningRodBlock" => Box::new(weathering_lightning_rod_block::WeatheringLightningRodBlock),
        "TorchBlock" => Box::new(torch_block::TorchBlock),
        "WallTorchBlock" => Box::new(wall_torch_block::WallTorchBlock),
        "RedstoneTorchBlock" => Box::new(redstone_torch_block::RedstoneTorchBlock { block }),
        "RedstoneWallTorchBlock" => Box::new(redstone_wall_torch_block::RedstoneWallTorchBlock { torch: redstone_torch_block::RedstoneTorchBlock { block } }),
        "LeverBlock" => Box::new(lever_block::LeverBlock { block }),
        "ButtonBlock" => Box::new(button_block::ButtonBlock::new(block)),
        "RedstoneWireBlock" => Box::new(redstone_wire_block::RedstoneWireBlock::new(block)),
        "RedstoneLampBlock" => Box::new(redstone_lamp_block::RedstoneLampBlock { block }),
        "RepeaterBlock" => Box::new(repeater_block::RepeaterBlock { block }),
        "ObserverBlock" => Box::new(observer_block::ObserverBlock { block }),
        "PoweredBlock" => Box::new(powered_block::PoweredBlock),
        "TargetBlock" => Box::new(target_block::TargetBlock { block }),
        "NoteBlock" => Box::new(note_block::NoteBlock { block }),
        "TripWireHookBlock" => Box::new(trip_wire_hook_block::TripWireHookBlock),
        "TripWireBlock" => Box::new(trip_wire_block::TripWireBlock::new(block)),
        "RailBlock" => Box::new(rail_block::RailBlock { block }),
        "PoweredRailBlock" => Box::new(powered_rail_block::PoweredRailBlock { block }),
        "DetectorRailBlock" => Box::new(detector_rail_block::DetectorRailBlock { block }),
        "PressurePlateBlock" => Box::new(pressure_plate_block::PressurePlateBlock::new(block)),
        "WeightedPressurePlateBlock" => Box::new(weighted_pressure_plate_block::WeightedPressurePlateBlock::new(block)),
        "VegetationBlock" => Box::new(vegetation_block::VegetationBlock),
        "BushBlock" => Box::new(bush_block::BushBlock),
        "FlowerBlock" => Box::new(flower_block::FlowerBlock),
        "WitherRoseBlock" => Box::new(wither_rose_block::WitherRoseBlock),
        "TallGrassBlock" => Box::new(tall_grass_block::TallGrassBlock),
        "DoublePlantBlock" => Box::new(double_plant_block::DoublePlantBlock),
        "TallFlowerBlock" => Box::new(tall_flower_block::TallFlowerBlock),
        "FlowerBedBlock" => Box::new(flower_bed_block::FlowerBedBlock),
        "LeafLitterBlock" => Box::new(leaf_litter_block::LeafLitterBlock),
        "DryVegetationBlock" => Box::new(dry_vegetation_block::DryVegetationBlock),
        "ShortDryGrassBlock" => Box::new(short_dry_grass_block::ShortDryGrassBlock),
        "TallDryGrassBlock" => Box::new(tall_dry_grass_block::TallDryGrassBlock),
        "FireflyBushBlock" => Box::new(firefly_bush_block::FireflyBushBlock),
        "CactusFlowerBlock" => Box::new(cactus_flower_block::CactusFlowerBlock),
        "CropBlock" => Box::new(crop_block::CropBlock),
        "BeetrootBlock" => Box::new(beetroot_block::BeetrootBlock),
        "TorchflowerCropBlock" => Box::new(torchflower_crop_block::TorchflowerCropBlock),
        "PitcherCropBlock" => Box::new(pitcher_crop_block::PitcherCropBlock),
        "StemBlock" => Box::new(stem_block::StemBlock::new(block)),
        "AttachedStemBlock" => Box::new(attached_stem_block::AttachedStemBlock::new(block)),
        "SugarCaneBlock" => Box::new(sugar_cane_block::SugarCaneBlock),
        "CactusBlock" => Box::new(cactus_block::CactusBlock),
        "SnowyBlock" => Box::new(snowy_block::SnowyBlock),
        "SpreadingSnowyBlock" => Box::new(spreading_snowy_block::SpreadingSnowyBlock::new(block)),
        "NyliumBlock" => Box::new(nylium_block::NyliumBlock),
        "FarmlandBlock" => Box::new(farmland_block::FarmlandBlock::new(block)),
        "LeavesBlock" => Box::new(leaves_block::LeavesBlock),
        "MangroveLeavesBlock" => Box::new(mangrove_leaves_block::MangroveLeavesBlock),
        "VineBlock" => Box::new(vine_block::VineBlock),
        "MushroomBlock" => Box::new(mushroom_block::MushroomBlock::new(block)),
        "NetherWartBlock" => Box::new(nether_wart_block::NetherWartBlock),
        "CocoaBlock" => Box::new(cocoa_block::CocoaBlock),
        "SweetBerryBushBlock" => Box::new(sweet_berry_bush_block::SweetBerryBushBlock),
        "SaplingBlock" => Box::new(sapling_block::SaplingBlock::new(block)),
        "MangrovePropaguleBlock" => Box::new(mangrove_propagule_block::MangrovePropaguleBlock::new(block)),
        "AzaleaBlock" => Box::new(azalea_block::AzaleaBlock),
        "FlowerPotBlock" => Box::new(flower_pot_block::FlowerPotBlock::new(block)),
        "LilyPadBlock" => Box::new(lily_pad_block::LilyPadBlock),
        "SeaPickleBlock" => Box::new(sea_pickle_block::SeaPickleBlock),
        "SeagrassBlock" => Box::new(seagrass_block::SeagrassBlock),
        "TallSeagrassBlock" => Box::new(tall_seagrass_block::TallSeagrassBlock),
        "NetherRootsBlock" => Box::new(nether_roots_block::NetherRootsBlock::new(block)),
        "NetherSproutsBlock" => Box::new(nether_sprouts_block::NetherSproutsBlock),
        "NetherFungusBlock" => Box::new(nether_fungus_block::NetherFungusBlock::new(block)),
        "KelpBlock" => Box::new(GrowingPlantHeadBlock::new(block, kelp_block::KelpBlock)),
        "KelpPlantBlock" => Box::new(GrowingPlantBodyBlock::new(block, kelp_plant_block::KelpPlantBlock)),
        "WeepingVinesBlock" => Box::new(GrowingPlantHeadBlock::new(block, weeping_vines_block::WeepingVinesBlock)),
        "WeepingVinesPlantBlock" => Box::new(GrowingPlantBodyBlock::new(block, weeping_vines_plant_block::WeepingVinesPlantBlock)),
        "TwistingVinesBlock" => Box::new(GrowingPlantHeadBlock::new(block, twisting_vines_block::TwistingVinesBlock)),
        "TwistingVinesPlantBlock" => Box::new(GrowingPlantBodyBlock::new(block, twisting_vines_plant_block::TwistingVinesPlantBlock)),
        "CaveVinesBlock" => Box::new(cave_vines_block::CaveVinesBlock(GrowingPlantHeadBlock::new(block, cave_vines_block::CaveVinesParts))),
        "CaveVinesPlantBlock" => Box::new(cave_vines_plant_block::CaveVinesPlantBlock(GrowingPlantBodyBlock::new(block, cave_vines_plant_block::CaveVinesPlantParts))),
        "HangingRootsBlock" => Box::new(hanging_roots_block::HangingRootsBlock),
        "SporeBlossomBlock" => Box::new(spore_blossom_block::SporeBlossomBlock),
        "BigDripleafBlock" => Box::new(big_dripleaf_block::BigDripleafBlock),
        "BigDripleafStemBlock" => Box::new(big_dripleaf_stem_block::BigDripleafStemBlock),
        "SmallDripleafBlock" => Box::new(small_dripleaf_block::SmallDripleafBlock),
        "MossyCarpetBlock" => Box::new(mossy_carpet_block::MossyCarpetBlock),
        "HangingMossBlock" => Box::new(hanging_moss_block::HangingMossBlock),
        "GlowLichenBlock" => Box::new(glow_lichen_block::GlowLichenBlock::new(block)),
        "SculkVeinBlock" => Box::new(sculk_vein_block::SculkVeinBlock),
        "MultifaceBlock" => Box::new(multiface_block::MultifaceBlock),
        "AmethystClusterBlock" => Box::new(amethyst_cluster_block::AmethystClusterBlock),
        "BannerBlock" => Box::new(banner_block::BannerBlock),
        "BaseCoralWallFanBlock" => Box::new(base_coral_wall_fan_block::BaseCoralWallFanBlock),
        "BaseCoralPlantTypeBlock" => Box::new(base_coral_plant_type_block::BaseCoralPlantTypeBlock),
        "CoralPlantBlock" => Box::new(coral_plant_block::CoralPlantBlock::new(block)),
        "CoralFanBlock" => Box::new(coral_fan_block::CoralFanBlock::new(block)),
        "CoralWallFanBlock" => Box::new(coral_wall_fan_block::CoralWallFanBlock::new(block)),
        "CoralBlock" => Box::new(coral_block::CoralBlock::new(block)),
        "TurtleEggBlock" => Box::new(turtle_egg_block::TurtleEggBlock::new(block)),
        "SnifferEggBlock" => Box::new(sniffer_egg_block::SnifferEggBlock::new(block)),
        "FrogspawnBlock" => Box::new(frogspawn_block::FrogspawnBlock::new(block)),
        "ChorusPlantBlock" => Box::new(chorus_plant_block::ChorusPlantBlock::new(block)),
        "ChorusFlowerBlock" => Box::new(chorus_flower_block::ChorusFlowerBlock::new(block)),
        "BellBlock" => Box::new(bell_block::BellBlock),
        "CalibratedSculkSensorBlock" => Box::new(calibrated_sculk_sensor_block::CalibratedSculkSensorBlock),
        "CampfireBlock" => Box::new(campfire_block::CampfireBlock),
        "CeilingHangingSignBlock" => Box::new(ceiling_hanging_sign_block::CeilingHangingSignBlock),
        "ChiseledBookShelfBlock" => Box::new(chiseled_book_shelf_block::ChiseledBookShelfBlock),
        "CommandBlock" => Box::new(command_block::CommandBlock),
        "CopperGolemStatueBlock" => Box::new(copper_golem_statue_block::CopperGolemStatueBlock),
        "CrafterBlock" => Box::new(crafter_block::CrafterBlock),
        "CreakingHeartBlock" => Box::new(creaking_heart_block::CreakingHeartBlock),
        "DecoratedPotBlock" => Box::new(decorated_pot_block::DecoratedPotBlock),
        "EndPortalFrameBlock" => Box::new(end_portal_frame_block::EndPortalFrameBlock),
        "EnderChestBlock" => Box::new(ender_chest_block::EnderChestBlock),
        "HorizontalDirectionalBlock" => Box::new(horizontal_directional_block::HorizontalDirectionalBlock),
        "HugeMushroomBlock" => Box::new(huge_mushroom_block::HugeMushroomBlock),
        "InfestedRotatedPillarBlock" => Box::new(infested_rotated_pillar_block::InfestedRotatedPillarBlock),
        "JigsawBlock" => Box::new(jigsaw_block::JigsawBlock),
        "LecternBlock" => Box::new(lectern_block::LecternBlock),
        "MovingPistonBlock" => Box::new(moving_piston_block::MovingPistonBlock),
        "NetherPortalBlock" => Box::new(nether_portal_block::NetherPortalBlock),
        "PistonBaseBlock" => Box::new(piston_base_block::PistonBaseBlock),
        "ShelfBlock" => Box::new(shelf_block::ShelfBlock),
        "PointedDripstoneBlock" => Box::new(pointed_dripstone_block::PointedDripstoneBlock::new(block)),
        "SulfurSpikeBlock" => Box::new(sulfur_spike_block::SulfurSpikeBlock::new(block)),
        "SkullBlock" => Box::new(skull_block::SkullBlock),
        "StandingSignBlock" => Box::new(standing_sign_block::StandingSignBlock),
        "VaultBlock" => Box::new(vault_block::VaultBlock),
        "WallBannerBlock" => Box::new(wall_banner_block::WallBannerBlock),
        "WallHangingSignBlock" => Box::new(wall_hanging_sign_block::WallHangingSignBlock),
        "WallSignBlock" => Box::new(wall_sign_block::WallSignBlock),
        "WallSkullBlock" => Box::new(wall_skull_block::WallSkullBlock),
        _ => return None,
    })
}

pub struct CraftingTableBlock;

impl BlockBehaviour for CraftingTableBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        level.open_crafting_table(player, pos);
        InteractionResult::SUCCESS_SERVER
    }
}

pub fn behaviour(block: Block) -> &'static dyn BlockBehaviour {
    BEHAVIOURS[block.0 as usize].as_ref()
}

pub fn of(state: BlockState) -> &'static dyn BlockBehaviour {
    behaviour(state.block())
}

pub fn rotate(state: BlockState, rotation: Rotation) -> BlockState {
    of(state).rotate(state, rotation)
}

pub fn mirror(state: BlockState, mirror: Mirror) -> BlockState {
    of(state).mirror(state, mirror)
}

#[cfg(test)]
pub fn click(level: &mut ServerLevel, player: EntityId, pos: BlockPos, face: Direction, location: Vec3) -> InteractionResult {
    let hit = BlockHitResult { location, direction: face, block_pos: pos, inside: false, world_border_hit: false };
    level.use_item_on(player, InteractionHand::MainHand, hit)
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::{blocks, items};

    use super::BlockHitResult;
    use crate::server::level::testing;
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;

    #[test]
    fn places_a_block_against_the_clicked_face() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::STONE, 1));
        let grass = BlockPos::new(0, -61, 0);
        let hit = BlockHitResult { location: Vec3::new(0.5, -60.0, 0.5), direction: Direction::Up, block_pos: grass, inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
        assert_eq!(level.block_state(grass.above()), blocks::STONE.default_state());
        assert_eq!(level.player(player).inventory.slots[36], None);
    }
}
