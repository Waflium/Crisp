use std::cell::RefCell;
use std::rc::Rc;

use crisp_core::{BlockPos, ChunkPos, Direction, SectionPos, mth};
use crisp_data::{BlockState, EquipmentSlot, block_tags, blocks, items};

use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::map_post_processing::MapPostProcessing;
use crate::world::level::block::BlockHitResult;
use crate::world::level::LevelReader;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::material::create_legacy_block;
use crate::world::level::material::map_color::{Brightness, MapColor, get_map_color};
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::level::saveddata::maps::map_item_saved_data::MapItemSavedData;

pub fn create(level: &mut ServerLevel, origin_x: i32, origin_z: i32, scale: i8, track_position: bool, unlimited_tracking: bool) -> ItemStack {
    let mut map = ItemStack::new(items::FILLED_MAP, 1);
    apply_new_saved_data(level, &mut map, origin_x, origin_z, scale, track_position, unlimited_tracking);
    map
}

pub fn apply_new_saved_data(level: &mut ServerLevel, map: &mut ItemStack, origin_x: i32, origin_z: i32, scale: i8, track_position: bool, unlimited_tracking: bool) {
    let dimension = level.dimension;
    let new_id = create_new_saved_data(level, origin_x, origin_z, scale, track_position, unlimited_tracking, dimension);
    map.set(dc::MAP_ID, Some(new_id.to_nbt()));
}

pub fn get_saved_data_by_id(id: Option<MapId>, level: &ServerLevel) -> Option<Rc<RefCell<MapItemSavedData>>> {
    level.get_map_data(id?)
}

pub fn get_saved_data(item_stack: &ItemStack, level: &ServerLevel) -> Option<Rc<RefCell<MapItemSavedData>>> {
    let id = item_stack.get(dc::MAP_ID).and_then(MapId::from_nbt);
    get_saved_data_by_id(id, level)
}

fn create_new_saved_data(level: &mut ServerLevel, x_spawn: i32, z_spawn: i32, scale: i8, tracking_position: bool, unlimited_tracking: bool, dimension: &str) -> MapId {
    let new_data = MapItemSavedData::create_fresh(x_spawn as f64, z_spawn as f64, scale, tracking_position, unlimited_tracking, dimension);
    let id = level.get_free_map_id();
    level.set_map_data(id, new_data);
    id
}

#[derive(Default)]
struct ColorCount {
    entries: Vec<(MapColor, i32)>,
}

impl ColorCount {
    fn add(&mut self, color: MapColor, occurrences: i32) {
        match self.entries.iter_mut().find(|(c, _)| *c == color) {
            Some((_, count)) => *count += occurrences,
            None => self.entries.push((color, occurrences)),
        }
    }

    fn highest_count_first(&self) -> MapColor {
        let mut best: Option<(MapColor, i32)> = None;
        for &(color, count) in &self.entries {
            if best.is_none_or(|(_, best_count)| count > best_count) {
                best = Some((color, count));
            }
        }
        best.map_or(MapColor::NONE, |(color, _)| color)
    }
}

pub fn update(level: &mut ServerLevel, player: EntityId, data: &mut MapItemSavedData) {
    if level.dimension != data.dimension {
        return;
    }
    let scale = 1 << data.scale;
    let center_x = data.center_x;
    let center_z = data.center_z;
    let (player_x, player_z) = {
        let e = &level.player(player).entity;
        (e.x(), e.z())
    };
    let player_img_x = mth::floor(player_x - center_x as f64) / scale + 64;
    let player_img_y = mth::floor(player_z - center_z as f64) / scale + 64;
    let has_ceiling = level.dimension_type().has_ceiling;
    let mut radius = 128 / scale;
    if has_ceiling {
        radius /= 2;
    }
    let holding_player = data.get_holding_player(level, player);
    holding_player.step += 1;
    let step = holding_player.step;
    let min_y = level.chunks.min_y();
    let mut found_consecutive_changes = false;
    for img_x in player_img_x - radius + 1..player_img_x + radius {
        if (img_x & 15) != (step & 15) && !found_consecutive_changes {
            continue;
        }
        found_consecutive_changes = false;
        let mut previous_average_area_height = 0.0;
        for img_y in player_img_y - radius - 1..player_img_y + radius {
            if !(img_x >= 0 && img_y >= -1 && img_x < 128 && img_y < 128) {
                continue;
            }
            let distance_to_player_sqr = (img_x - player_img_x) * (img_x - player_img_x) + (img_y - player_img_y) * (img_y - player_img_y);
            let dither_black = distance_to_player_sqr > (radius - 2) * (radius - 2);
            let averaging_area_min_x = (center_x / scale + img_x - 64) * scale;
            let averaging_area_min_z = (center_z / scale + img_y - 64) * scale;
            let mut color_count = ColorCount::default();
            let chunk_pos = ChunkPos::new(SectionPos::block_to_section_coord(averaging_area_min_x), SectionPos::block_to_section_coord(averaging_area_min_z));
            level.chunks.chunk(chunk_pos);
            let mut water_depth = 0;
            let mut average_area_height = 0.0;
            if has_ceiling {
                let mut ceiling_noise = averaging_area_min_x.wrapping_add(averaging_area_min_z.wrapping_mul(231871));
                ceiling_noise = ceiling_noise.wrapping_mul(ceiling_noise).wrapping_mul(31287121).wrapping_add(ceiling_noise.wrapping_mul(11));
                if (ceiling_noise >> 20 & 1) == 0 {
                    color_count.add(get_map_color(blocks::DIRT.default_state()), 10);
                } else {
                    color_count.add(get_map_color(blocks::STONE.default_state()), 100);
                }
                average_area_height = 100.0;
            } else {
                for averaging_area_delta_x in 0..scale {
                    for averaging_area_delta_z in 0..scale {
                        let (x, z) = (averaging_area_min_x + averaging_area_delta_x, averaging_area_min_z + averaging_area_delta_z);
                        let chunk = level.chunks.chunk(chunk_pos);
                        let mut column_y = chunk.get_height(HeightmapType::WorldSurface, x, z) + 1;
                        let mut block_pos = BlockPos::new(x, 0, z);
                        let mut state;
                        if column_y <= min_y {
                            state = blocks::BEDROCK.default_state();
                        } else {
                            loop {
                                column_y -= 1;
                                block_pos.y = column_y;
                                state = chunk.block_state(block_pos);
                                if !(get_map_color(state) == MapColor::NONE && column_y > min_y) {
                                    break;
                                }
                            }
                            if column_y > min_y && !state.fluid_state().is_empty() {
                                let mut solid_y = column_y - 1;
                                let mut below_pos = block_pos;
                                loop {
                                    below_pos.y = solid_y;
                                    solid_y -= 1;
                                    let below_block = chunk.block_state(below_pos);
                                    water_depth += 1;
                                    if !(solid_y > min_y && !below_block.fluid_state().is_empty()) {
                                        break;
                                    }
                                }
                                state = get_correct_state_for_fluid_block(state);
                            }
                        }
                        data.check_banners(level, x, z);
                        average_area_height += column_y as f64 / (scale * scale) as f64;
                        color_count.add(get_map_color(state), 1);
                    }
                }
            }
            water_depth /= scale * scale;
            let color = color_count.highest_count_first();
            let brightness = if color == MapColor::WATER {
                let diff = water_depth as f64 * 0.1 + ((img_x + img_y) & 1) as f64 * 0.2;
                if diff < 0.5 {
                    Brightness::High
                } else if diff > 0.9 {
                    Brightness::Low
                } else {
                    Brightness::Normal
                }
            } else {
                let diff = (average_area_height - previous_average_area_height) * 4.0 / (scale + 4) as f64 + (((img_x + img_y) & 1) as f64 - 0.5) * 0.4;
                if diff > 0.6 {
                    Brightness::High
                } else if diff < -0.6 {
                    Brightness::Low
                } else {
                    Brightness::Normal
                }
            };
            previous_average_area_height = average_area_height;
            if img_y >= 0 && distance_to_player_sqr < radius * radius && (!dither_black || (img_x + img_y) & 1 != 0) {
                found_consecutive_changes |= data.update_color(img_x, img_y, color.get_packed_id(brightness));
            }
        }
    }
}

fn get_correct_state_for_fluid_block(state: BlockState) -> BlockState {
    let fluid_state = state.fluid_state();
    if !fluid_state.is_empty() && !state.is_face_sturdy_full(Direction::Up) { create_legacy_block(fluid_state) } else { state }
}

fn is_biome_watery(is_biome_watery: &[bool], x: i32, z: i32) -> bool {
    is_biome_watery[(z * 128 + x) as usize]
}

pub fn render_biome_preview_map(level: &ServerLevel, map_item_stack: &ItemStack) {
    let Some(data) = get_saved_data(map_item_stack, level) else { return };
    let mut data = data.borrow_mut();
    if level.dimension != data.dimension {
        return;
    }
    let scale = 1 << data.scale;
    let center_x = data.center_x;
    let center_z = data.center_z;
    let mut biome_watery = vec![false; 16384];
    let unscaled_start_x = center_x / scale - 64;
    let unscaled_start_z = center_z / scale - 64;
    let biome_sample_y = level.get_sea_level();
    let water_on_map_outlines = crisp_data::tag("minecraft:worldgen/biome", "minecraft:water_on_map_outlines");
    for row in 0..128 {
        for column in 0..128 {
            let biome = level.get_biome(BlockPos::new((unscaled_start_x + column) * scale, biome_sample_y, (unscaled_start_z + row) * scale));
            biome_watery[(row * 128 + column) as usize] = water_on_map_outlines.contains(&(biome.0 as i32));
        }
    }
    for mx in 1..127 {
        for mz in 1..127 {
            let mut water_count = 0;
            for dx in -1..2 {
                for dz in -1..2 {
                    if (dx != 0 || dz != 0) && is_biome_watery(&biome_watery, mx + dx, mz + dz) {
                        water_count += 1;
                    }
                }
            }
            let mut brightness = Brightness::Lowest;
            let mut new_color = MapColor::NONE;
            if is_biome_watery(&biome_watery, mx, mz) {
                new_color = MapColor::COLOR_ORANGE;
                if water_count > 7 && mz % 2 == 0 {
                    match (mx + (mth::sin((mz as f32 + 0.0) as f64) * 7.0) as i32) / 8 % 5 {
                        0 | 4 => brightness = Brightness::Low,
                        1 | 3 => brightness = Brightness::Normal,
                        2 => brightness = Brightness::High,
                        _ => {}
                    }
                } else if water_count > 7 {
                    new_color = MapColor::NONE;
                } else if water_count > 5 {
                    brightness = Brightness::Normal;
                } else if water_count > 1 {
                    brightness = Brightness::Low;
                }
            } else if water_count > 0 {
                new_color = MapColor::COLOR_BROWN;
                brightness = if water_count > 3 { Brightness::Normal } else { Brightness::Lowest };
            }
            if new_color != MapColor::NONE {
                data.set_color(mx, mz, new_color.get_packed_id(brightness));
            }
        }
    }
}

pub fn inventory_tick(item_stack: &ItemStack, level: &mut ServerLevel, owner: EntityId, slot: Option<EquipmentSlot>) {
    let Some(data) = get_saved_data(item_stack, level) else { return };
    let mut data = data.borrow_mut();
    if level.is_player(owner) {
        data.tick_carried_by(level, owner, item_stack);
    }
    if !data.locked && matches!(slot, Some(EquipmentSlot::Mainhand | EquipmentSlot::Offhand)) {
        update(level, owner, &mut data);
    }
}

pub fn on_crafted_post_process(item_stack: &mut ItemStack, level: &mut ServerLevel) {
    let Some(post_processing) = item_stack.remove(dc::MAP_POST_PROCESSING) else { return };
    match MapPostProcessing::from_nbt(&post_processing) {
        Some(MapPostProcessing::Lock) => lock_map(item_stack, level),
        Some(MapPostProcessing::Scale) => scale_map(item_stack, level),
        None => {}
    }
}

fn scale_map(item_stack: &mut ItemStack, level: &mut ServerLevel) {
    let Some(original) = get_saved_data(item_stack, level) else { return };
    let id = level.get_free_map_id();
    let scaled = original.borrow().scaled();
    level.set_map_data(id, scaled);
    item_stack.set(dc::MAP_ID, Some(id.to_nbt()));
}

fn lock_map(map: &mut ItemStack, level: &mut ServerLevel) {
    let Some(map_data) = get_saved_data(map, level) else { return };
    let id = level.get_free_map_id();
    let new_data = map_data.borrow().locked();
    level.set_map_data(id, new_data);
    map.set(dc::MAP_ID, Some(id.to_nbt()));
}

pub fn use_on(level: &mut ServerLevel, hit: &BlockHitResult, item_stack: &ItemStack) -> Option<InteractionResult> {
    let clicked = crate::world::level::BlockGetter::block_state(level, hit.block_pos);
    if !clicked.is(&block_tags::BANNERS) {
        return None;
    }
    if let Some(data) = get_saved_data(item_stack, level)
        && !data.borrow_mut().toggle_banner(level, hit.block_pos)
    {
        return Some(InteractionResult::Fail);
    }
    Some(InteractionResult::SUCCESS)
}
