use crisp_core::{AABB, BlockPos};
use crisp_data::{BlockState, MobEffect, block_tags, blocks, mob_effects, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::argb;
use crate::world::effect::MobEffectInstance;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::item::dye_color::DyeColor;
use crate::world::level::BlockGetter;
use crate::world::level::block::beacon_block::BeaconBlock;
use crate::world::level::block::vegetation_block::fields;
use crate::world::level::chunk::HeightmapType;

const MAX_LEVELS: i32 = 4;
pub const BEACON_EFFECTS: [&[MobEffect]; 4] =
    [&[mob_effects::SPEED, mob_effects::HASTE], &[mob_effects::RESISTANCE, mob_effects::JUMP_BOOST], &[mob_effects::STRENGTH], &[mob_effects::REGENERATION]];
const BLOCKS_CHECK_PER_TICK: i32 = 10;
const TAG_PRIMARY: &str = "primary_effect";
const TAG_SECONDARY: &str = "secondary_effect";

fn is_valid_effect(effect: MobEffect) -> bool {
    BEACON_EFFECTS.iter().any(|effects| effects.contains(&effect))
}

#[derive(Clone)]
pub struct Section {
    color: i32,
    height: i32,
}

impl Section {
    pub fn new(color: i32) -> Section {
        Section { color, height: 1 }
    }

    fn increase_height(&mut self) {
        self.height += 1;
    }

    pub fn get_color(&self) -> i32 {
        self.color
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }
}

#[derive(Clone)]
pub struct BeaconBlockEntity {
    beam_sections: Vec<Section>,
    checking_beam_sections: Vec<Section>,
    levels: i32,
    last_check_y: i32,
    primary_power: Option<MobEffect>,
    secondary_power: Option<MobEffect>,
    name: Option<Nbt>,
    lock_key: Option<Nbt>,
}

impl Default for BeaconBlockEntity {
    fn default() -> BeaconBlockEntity {
        BeaconBlockEntity {
            beam_sections: Vec::new(),
            checking_beam_sections: Vec::new(),
            levels: 0,
            last_check_y: 0,
            primary_power: None,
            secondary_power: None,
            name: None,
            lock_key: None,
        }
    }
}

fn store_effect(output: &mut Vec<(String, Nbt)>, field: &str, effect: Option<MobEffect>) {
    if let Some(effect) = effect {
        output.push((field.into(), Nbt::String(effect.info().name.into())));
    }
}

fn load_effect(input: &Nbt, field: &str) -> Option<MobEffect> {
    let name = crate::commands::arguments::parse_identifier(input.get(field)?.as_str()?)?;
    MobEffect::by_name(&name).filter(|&effect| is_valid_effect(effect))
}

impl BeaconBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.primary_power = load_effect(input, TAG_PRIMARY);
        self.secondary_power = load_effect(input, TAG_SECONDARY);
        self.name = input.get("CustomName").and_then(|name| dc::CUSTOM_NAME.decode(name).ok());
        self.lock_key = input.get("lock").and_then(|lock| dc::LOCK.decode(lock).ok());
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        store_effect(output, TAG_PRIMARY, self.primary_power);
        store_effect(output, TAG_SECONDARY, self.secondary_power);
        output.push(("Levels".into(), Nbt::Int(self.levels)));
        if let Some(name) = &self.name {
            output.push(("CustomName".into(), name.clone()));
        }
        if self.is_locked() {
            output.push(("lock".into(), self.lock_key.clone().unwrap_or(Nbt::Compound(Vec::new()))));
        }
    }

    fn is_locked(&self) -> bool {
        self.lock_key.as_ref().is_some_and(|lock| !matches!(lock, Nbt::Compound(fields) if fields.is_empty()))
    }

    pub fn get_beam_sections(&self) -> &[Section] {
        if self.levels == 0 { &[] } else { &self.beam_sections }
    }

    pub fn set_custom_name(&mut self, name: Option<Nbt>) {
        self.name = name;
    }

    pub fn get_custom_name(&self) -> Option<&Nbt> {
        self.name.as_ref()
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.name = get(dc::CUSTOM_NAME).cloned();
        self.lock_key = get(dc::LOCK).cloned();
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        let mut set = |ty: DataComponentType, value: Option<Nbt>| {
            components.retain(|(t, _)| *t != ty);
            components.push((ty, value));
        };
        set(dc::CUSTOM_NAME, self.name.clone());
        if self.is_locked() {
            set(dc::LOCK, self.lock_key.clone());
        }
    }

    pub fn set_level(&mut self, min_y: i32) {
        self.last_check_y = min_y - 1;
    }
}

fn beacon_beam_color(state: BlockState) -> Option<i32> {
    let block = state.block();
    let color = if block.is_instance_of("BeaconBlock") {
        BeaconBlock.get_color()
    } else if block.is_instance_of("StainedGlassBlock") || block.is_instance_of("StainedGlassPaneBlock") {
        fields(block)["color"].as_str().and_then(DyeColor::by_name).expect("beacon beam blocks have a color")
    } else {
        return None;
    };
    Some(color.texture_diffuse_color())
}

impl ServerLevel {
    fn beacon(&mut self, pos: BlockPos) -> Option<&mut BeaconBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Beacon(beacon) => Some(beacon),
            _ => None,
        }
    }

    pub fn beacon_tick(&mut self, pos: BlockPos) {
        let (x, y, z) = (pos.x, pos.y, pos.z);
        let last_set_block = self.heightmap_pos(HeightmapType::WorldSurface, pos).y;
        let Some(entity) = self.beacon(pos) else { return };
        let mut check_pos = if entity.last_check_y < y {
            entity.checking_beam_sections = Vec::new();
            entity.last_check_y = pos.y - 1;
            pos
        } else {
            BlockPos::new(x, entity.last_check_y + 1, z)
        };
        let mut checking_beam_sections = std::mem::take(&mut entity.checking_beam_sections);
        let mut last_check_y = entity.last_check_y;
        let mut i = 0;
        while i < BLOCKS_CHECK_PER_TICK && check_pos.y <= last_set_block {
            let state = self.block_state(check_pos);
            if let Some(color) = beacon_beam_color(state) {
                if checking_beam_sections.len() <= 1 {
                    checking_beam_sections.push(Section::new(color));
                } else if let Some(last_beam_section) = checking_beam_sections.last_mut() {
                    if color == last_beam_section.get_color() {
                        last_beam_section.increase_height();
                    } else {
                        let average = argb::average(last_beam_section.get_color(), color);
                        checking_beam_sections.push(Section::new(average));
                    }
                }
            } else {
                match checking_beam_sections.last_mut() {
                    Some(last_beam_section) if state.light_dampening() < 15 || state.is_block(blocks::BEDROCK) => last_beam_section.increase_height(),
                    _ => {
                        checking_beam_sections.clear();
                        last_check_y = last_set_block;
                        break;
                    }
                }
            }
            check_pos = check_pos.above();
            last_check_y += 1;
            i += 1;
        }
        let Some(entity) = self.beacon(pos) else { return };
        entity.checking_beam_sections = checking_beam_sections;
        entity.last_check_y = last_check_y;
        let previous_levels = entity.levels;
        let has_beam_sections = !entity.beam_sections.is_empty();
        if self.game_time % 80 == 0 {
            if has_beam_sections {
                let levels = self.beacon_update_base(x, y, z);
                if let Some(entity) = self.beacon(pos) {
                    entity.levels = levels;
                }
            }
            let Some(entity) = self.beacon(pos) else { return };
            if entity.levels > 0 && !entity.beam_sections.is_empty() {
                let (levels, primary_power, secondary_power) = (entity.levels, entity.primary_power, entity.secondary_power);
                self.beacon_apply_effects(pos, levels, primary_power, secondary_power);
                self.beacon_play_sound(pos, "minecraft:block.beacon.ambient");
            }
        }
        let min_y = self.chunks.min_y();
        let Some(entity) = self.beacon(pos) else { return };
        if entity.last_check_y >= last_set_block {
            entity.last_check_y = min_y - 1;
            let was_active = previous_levels > 0;
            entity.beam_sections = std::mem::take(&mut entity.checking_beam_sections);
            let is_active = entity.levels > 0;
            if !was_active && is_active {
                self.beacon_play_sound(pos, "minecraft:block.beacon.activate");
            } else if was_active && !is_active {
                self.beacon_play_sound(pos, "minecraft:block.beacon.deactivate");
            }
        }
    }

    fn beacon_update_base(&mut self, x: i32, y: i32, z: i32) -> i32 {
        let mut levels = 0;
        let mut step = 1;
        while step <= MAX_LEVELS {
            let ly = y - step;
            if ly < self.chunks.min_y() {
                break;
            }
            let mut is_ok = true;
            let mut lx = x - step;
            while lx <= x + step && is_ok {
                for lz in z - step..=z + step {
                    if !self.block_state(BlockPos::new(lx, ly, lz)).is(&block_tags::BEACON_BASE_BLOCKS) {
                        is_ok = false;
                        break;
                    }
                }
                lx += 1;
            }
            if !is_ok {
                break;
            }
            levels = step;
            step += 1;
        }
        levels
    }

    fn beacon_apply_effects(&mut self, world_position: BlockPos, levels: i32, primary_power: Option<MobEffect>, secondary_power: Option<MobEffect>) {
        let Some(primary_power) = primary_power else { return };
        let range = (levels * 10 + 10) as f64;
        let mut base_amp = 0;
        if levels >= 4 && Some(primary_power) == secondary_power {
            base_amp = 1;
        }
        let duration_ticks = (9 + levels * 2) * 20;
        let bb = AABB::of_block(world_position).inflate_all(range).expand_towards(0.0, self.chunks.height() as f64, 0.0);
        let players: Vec<_> = self.get_entities(None, &bb).into_iter().filter(|&id| self.is_player(id)).collect();
        for &player in &players {
            self.add_effect(player, MobEffectInstance::with(primary_power, duration_ticks, base_amp, true, true, true), None);
        }
        if levels >= 4
            && Some(primary_power) != secondary_power
            && let Some(secondary_power) = secondary_power
        {
            for &player in &players {
                self.add_effect(player, MobEffectInstance::with(secondary_power, duration_ticks, 0, true, true, true), None);
            }
        }
    }

    pub fn beacon_play_sound(&mut self, world_position: BlockPos, event: &str) {
        self.play_sound(None, world_position, registry_id(registry::SOUND_EVENT, event), SoundSource::Blocks, 1.0, 1.0);
    }
}
