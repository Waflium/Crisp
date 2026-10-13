use std::sync::atomic::{AtomicBool, Ordering};

use crisp_core::random::RandomSource;
use crisp_core::{Vec3, mth};
use crisp_data::{EquipmentSlot, items};
use crisp_protocol::nbt::Nbt;

use super::ItemStack;
use super::component::{self as dc, bundle_contents};
use super::enchantment::enchantment_helper;
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::projectile::firework_rocket_entity::FireworkRocketEntity;
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::{InteractionHand, InteractionResult};

const MAX_CHARGE_DURATION: f32 = 1.25;
const START_SOUND_PERCENT: f32 = 0.2;
const MID_SOUND_PERCENT: f32 = 0.5;
const ARROW_POWER: f32 = 3.15;
const FIREWORK_POWER: f32 = 1.6;
pub const USE_DURATION: i32 = 72000;

static START_SOUND_PLAYED: AtomicBool = AtomicBool::new(false);
static MID_LOAD_SOUND_PLAYED: AtomicBool = AtomicBool::new(false);

fn charged_projectiles(item_stack: &ItemStack) -> Vec<ItemStackTemplate> {
    item_stack.get(dc::CHARGED_PROJECTILES).map_or(Vec::new(), |charged| charged.as_list().iter().map(ItemStackTemplate::from_nbt).collect())
}

fn contains(projectiles: &[ItemStackTemplate], item: crisp_data::Item) -> bool {
    projectiles.iter().any(|projectile| projectile.item == item.name())
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let charged = charged_projectiles(&item_stack);
    if !charged.is_empty() {
        perform_shooting(level, player, hand, get_shooting_power(&charged), 1.0, None);
        return (InteractionResult::CONSUME, None);
    }
    if !level.get_projectile(player, &item_stack).is_empty() {
        START_SOUND_PLAYED.store(false, Ordering::Relaxed);
        MID_LOAD_SOUND_PLAYED.store(false, Ordering::Relaxed);
        level.start_using_item(player, hand);
        return (InteractionResult::CONSUME, None);
    }
    (InteractionResult::Fail, None)
}

fn get_shooting_power(projectiles: &[ItemStackTemplate]) -> f32 {
    if contains(projectiles, items::FIREWORK_ROCKET) { FIREWORK_POWER } else { ARROW_POWER }
}

pub fn release_using(level: &mut ServerLevel, item_stack: &ItemStack, entity: EntityId, remaining_time: i32) -> bool {
    let time_held = USE_DURATION - remaining_time;
    get_power_for_time(level, time_held, item_stack, entity) >= 1.0 && is_charged(item_stack)
}

fn try_load_projectiles(level: &mut ServerLevel, shooter: EntityId, held_item: &mut ItemStack) -> bool {
    let projectile = level.get_projectile(shooter, held_item);
    let drawn = level.draw(held_item, projectile, shooter);
    if drawn.is_empty() {
        return false;
    }
    let templates: Vec<Nbt> = drawn.iter().filter(|item| !item.is_empty()).map(bundle_contents::template_of).collect();
    if templates.len() != drawn.len() {
        warn!("Tried to load invalid items as charged projectiles");
    }
    held_item.set(dc::CHARGED_PROJECTILES, Some(Nbt::List(templates)));
    true
}

pub fn is_charged(item_stack: &ItemStack) -> bool {
    !charged_projectiles(item_stack).is_empty()
}

pub fn shot_vector(level: &ServerLevel, shooter: EntityId, angle: f32, _target_override: Option<EntityId>) -> [f32; 3] {
    let e = level.entity(shooter);
    let (x_rot, y_rot) = (e.x_rot, e.y_head_rot);
    let up_vector = calculate_view_vector(x_rot - 90.0, y_rot);
    let view_vec = calculate_view_vector(x_rot, y_rot);
    let quaternion = Quaternionf::set_angle_axis(angle * (std::f64::consts::PI / 180.0) as f32, up_vector.x as f32, up_vector.y as f32, up_vector.z as f32);
    quaternion.transform(view_vec.x as f32, view_vec.y as f32, view_vec.z as f32)
}

pub fn play_shoot_sound(level: &mut ServerLevel, shooter: EntityId, index: usize) {
    let sound_pitch = get_shot_pitch(&mut level.entity_mut(shooter).random, index);
    let e = level.entity(shooter);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let source = level.sound_source(shooter);
    level.play_sound_at(None, x, y, z, sound_event("minecraft:item.crossbow.shoot"), source, 1.0, sound_pitch);
}

fn calculate_view_vector(x_rot: f32, y_rot: f32) -> Vec3 {
    let x_rot_rad = x_rot * (std::f64::consts::PI / 180.0) as f32;
    let y_rot_rad = -y_rot * (std::f64::consts::PI / 180.0) as f32;
    let (y_cos, y_sin) = (mth::cos(y_rot_rad as f64), mth::sin(y_rot_rad as f64));
    let (x_cos, x_sin) = (mth::cos(x_rot_rad as f64), mth::sin(x_rot_rad as f64));
    Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
}

struct Quaternionf {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Quaternionf {
    fn set_angle_axis(angle: f32, x: f32, y: f32, z: f32) -> Quaternionf {
        let s = ((angle * 0.5) as f64).sin() as f32;
        Quaternionf { x: x * s, y: y * s, z: z * s, w: cos_from_sin(s, angle * 0.5) }
    }

    fn transform(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let (xx, yy, zz, ww) = (self.x * self.x, self.y * self.y, self.z * self.z, self.w * self.w);
        let (xy, xz, yz, xw) = (self.x * self.y, self.x * self.z, self.y * self.z, self.x * self.w);
        let (zw, yw) = (self.z * self.w, self.y * self.w);
        let k = 1.0 / (xx + yy + zz + ww);
        let fma = |a: f32, b: f32, c: f32| a * b + c;
        [
            fma((xx - yy - zz + ww) * k, x, fma(2.0 * (xy - zw) * k, y, (2.0 * (xz + yw) * k) * z)),
            fma(2.0 * (xy + zw) * k, x, fma((yy - xx - zz + ww) * k, y, (2.0 * (yz - xw) * k) * z)),
            fma(2.0 * (xz - yw) * k, x, fma(2.0 * (yz + xw) * k, y, ((zz - xx - yy + ww) * k) * z)),
        ]
    }
}

fn cos_from_sin(sin: f32, angle: f32) -> f32 {
    let pi_f = std::f64::consts::PI as f32;
    let pi2_f = (std::f64::consts::PI * 2.0) as f32;
    let pi_half_f = (std::f64::consts::PI * 0.5) as f32;
    let cos = ((1.0 - sin * sin) as f64).sqrt() as f32;
    let a = angle + pi_half_f;
    let mut b = a - (a / pi2_f) as i32 as f32 * pi2_f;
    if (b as f64) < 0.0 {
        b += pi2_f;
    }
    if b >= pi_f { -cos } else { cos }
}

pub fn create_projectile(level: &mut ServerLevel, shooter: EntityId, held_item: &ItemStack, projectile: &ItemStack, is_crit: bool) -> Projectile {
    if projectile.is(items::FIREWORK_ROCKET) {
        let e = level.entity(shooter);
        return FireworkRocketEntity::shot_at_angle_with_owner(projectile, e.uuid, e.x(), e.eye_y() - 0.15f32 as f64, e.z(), true);
    }
    let mut projectile_entity = level.projectile_weapon_create_projectile(shooter, held_item, projectile, is_crit);
    if let Some(arrow) = projectile_entity.abstract_arrow_mut() {
        arrow.set_sound_event(sound_event("minecraft:item.crossbow.hit"));
    }
    projectile_entity
}

pub fn perform_shooting(level: &mut ServerLevel, shooter: EntityId, hand: InteractionHand, power: f32, uncertainty: f32, target_override: Option<EntityId>) {
    let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
    let Some(mut weapon) = level.item_by_slot(shooter, slot) else { return };
    let charged = charged_projectiles(&weapon);
    weapon.set(dc::CHARGED_PROJECTILES, Some(Nbt::List(Vec::new())));
    level.set_item_slot(shooter, slot, weapon);
    if charged.is_empty() {
        return;
    }
    let projectiles: Vec<ItemStack> = charged.iter().map(|template| template.create().unwrap_or_else(ItemStack::empty)).collect();
    let is_player = level.is_player(shooter);
    level.projectile_weapon_shoot(shooter, hand, &projectiles, power, uncertainty, is_player, target_override);
    if is_player {
        let weapon = level.item_by_slot(shooter, slot).unwrap_or_else(ItemStack::empty);
        criteria_triggers::SHOT_CROSSBOW.trigger(level, shooter, &weapon);
        level.award_stat(shooter, crate::stats::stats::ITEM_USED.get(weapon.get_item().0), 1);
    }
}

fn get_shot_pitch(random: &mut impl RandomSource, index: usize) -> f32 {
    if index == 0 { 1.0 } else { get_random_shot_pitch(index & 1 == 1, random) }
}

fn get_random_shot_pitch(high_pitch: bool, random: &mut impl RandomSource) -> f32 {
    let range_decider = if high_pitch { 0.63 } else { 0.43 };
    1.0 / (random.next_float() * 0.5 + 1.8) + range_decider
}

pub fn on_use_tick(level: &mut ServerLevel, entity: EntityId, item_stack: &mut ItemStack, ticks_remaining: i32) {
    let sounds = get_charging_sounds(item_stack);
    let tick_percent = (USE_DURATION - ticks_remaining) as f32 / get_charge_duration(level, item_stack, entity) as f32;
    if tick_percent < START_SOUND_PERCENT {
        START_SOUND_PLAYED.store(false, Ordering::Relaxed);
        MID_LOAD_SOUND_PLAYED.store(false, Ordering::Relaxed);
    }
    let e = level.entity(entity);
    let (x, y, z) = (e.x(), e.y(), e.z());
    if tick_percent >= START_SOUND_PERCENT && !START_SOUND_PLAYED.load(Ordering::Relaxed) {
        START_SOUND_PLAYED.store(true, Ordering::Relaxed);
        if let Some(sound) = &sounds.start {
            level.play_sound_at(None, x, y, z, sound_event(sound), SoundSource::Players, 0.5, 1.0);
        }
    }
    if tick_percent >= MID_SOUND_PERCENT && !MID_LOAD_SOUND_PLAYED.load(Ordering::Relaxed) {
        MID_LOAD_SOUND_PLAYED.store(true, Ordering::Relaxed);
        if let Some(sound) = &sounds.mid {
            level.play_sound_at(None, x, y, z, sound_event(sound), SoundSource::Players, 0.5, 1.0);
        }
    }
    if tick_percent >= 1.0 && !is_charged(item_stack) && try_load_projectiles(level, entity, item_stack) {
        if let Some(sound) = &sounds.end {
            let source = level.sound_source(entity);
            let pitch = 1.0 / (level.random.next_float() * 0.5 + 1.0) + 0.2;
            level.play_sound_at(None, x, y, z, sound_event(sound), source, 1.0, pitch);
        }
    }
}

pub fn get_charge_duration(level: &mut ServerLevel, crossbow: &ItemStack, user: EntityId) -> i32 {
    let duration = enchantment_helper::modify_crossbow_charging_time(level, crossbow, user, MAX_CHARGE_DURATION);
    mth::floor_f(duration * 20.0)
}

struct ChargingSounds {
    start: Option<String>,
    mid: Option<String>,
    end: Option<String>,
}

fn get_charging_sounds(item_stack: &ItemStack) -> ChargingSounds {
    match enchantment_helper::pick_highest_crossbow_charging_sounds(item_stack) {
        Some(sounds) => {
            let sound = |key: &str| {
                sounds.get(key).and_then(|sound| match sound {
                    Nbt::String(id) => crate::commands::arguments::parse_identifier(id),
                    other => other.get("sound_id").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier),
                })
            };
            ChargingSounds { start: sound("start"), mid: sound("mid"), end: sound("end") }
        }
        None => ChargingSounds {
            start: Some("minecraft:item.crossbow.loading_start".into()),
            mid: Some("minecraft:item.crossbow.loading_middle".into()),
            end: Some("minecraft:item.crossbow.loading_end".into()),
        },
    }
}

fn get_power_for_time(level: &mut ServerLevel, time_held: i32, item_stack: &ItemStack, holder: EntityId) -> f32 {
    let pow = time_held as f32 / get_charge_duration(level, item_stack, holder) as f32;
    if pow > 1.0 { 1.0 } else { pow }
}

pub fn use_on_release(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is_instance_of("CrossbowItem")
}
