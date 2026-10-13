use crisp_core::Vec3;
use crisp_core::random::RandomSource;
use crisp_data::properties::LEVEL_HONEY;
use crisp_data::{Item, block_tags, blocks, fluid_tags, game_rule, items, registry, registry_id};

use super::block_source::BlockSource;
use super::default_dispense_item_behavior::{self, consume_with_remainder, play_default_animation};
use super::{
    equipment_dispense_item_behavior, flint_and_steel_dispense_item_behavior, projectile_dispense_behavior, shears_dispense_item_behavior, shulker_box_dispense_behavior,
    spawn_egg_item_behavior,
};
use crate::core::cauldron::cauldron_interactions::{is_water_potion, water_potion};
use crate::server::level::events::{PARTICLES_AND_SOUND_PLANT_GROWTH, PARTICLES_WAX_ON, SOUND_DISPENSER_DISPENSE, SOUND_DISPENSER_FAIL};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::LevelEntity;
use crate::world::entity::primed_tnt::PrimedTnt;
use crate::world::item::bucket_item::BucketItem;
use crate::world::item::component::{self as dc};
use crate::world::item::{ItemStack, bone_meal_item, honeycomb_item};
use crate::world::level::block::entity::beehive_block_entity::BeeReleaseStatus;
use crate::world::level::block::respawn_anchor_block::CHARGE;
use crate::world::level::block::{self, beehive_block, respawn_anchor_block};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::BlockGetter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DispenseItemBehavior {
    Default,
    FilledBucket,
    Bucket,
    FlintAndSteel,
    BoneMeal,
    Tnt,
    WitherSkeletonSkull,
    CarvedPumpkin,
    ShulkerBox,
    GlassBottle,
    Glowstone,
    Shears,
    Brush,
    Honeycomb,
    Potion,
    Equipment,
    SpawnEgg,
    Projectile,
}

fn registered(item: Item) -> Option<DispenseItemBehavior> {
    use DispenseItemBehavior::*;
    Some(match item {
        items::ARROW
        | items::TIPPED_ARROW
        | items::SPECTRAL_ARROW
        | items::EGG
        | items::BLUE_EGG
        | items::BROWN_EGG
        | items::SNOWBALL
        | items::EXPERIENCE_BOTTLE
        | items::SPLASH_POTION
        | items::LINGERING_POTION
        | items::FIREWORK_ROCKET => Projectile,
        items::LAVA_BUCKET | items::WATER_BUCKET | items::POWDER_SNOW_BUCKET => FilledBucket,
        items::BUCKET => Bucket,
        items::FLINT_AND_STEEL => FlintAndSteel,
        items::BONE_MEAL => BoneMeal,
        items::TNT => Tnt,
        items::WITHER_SKELETON_SKULL => WitherSkeletonSkull,
        items::CARVED_PUMPKIN => CarvedPumpkin,
        items::GLASS_BOTTLE => GlassBottle,
        items::GLOWSTONE => Glowstone,
        items::SHEARS => Shears,
        items::BRUSH => Brush,
        items::HONEYCOMB => Honeycomb,
        items::POTION => Potion,
        _ if item.info().block.is_some_and(|block| block.is_instance_of("ShulkerBoxBlock")) => ShulkerBox,
        _ => return None,
    })
}

pub fn get_dispense_method(item_stack: &ItemStack) -> DispenseItemBehavior {
    if let Some(behavior) = registered(item_stack.item) {
        return behavior;
    }
    if item_stack.has(dc::EQUIPPABLE) {
        DispenseItemBehavior::Equipment
    } else if item_stack.item.info().classes.contains(&"SpawnEggItem") && item_stack.has(dc::ENTITY_DATA) {
        DispenseItemBehavior::SpawnEgg
    } else {
        DispenseItemBehavior::Default
    }
}

fn target(source: &BlockSource) -> crisp_core::BlockPos {
    source.pos.relative(source.facing())
}

impl DispenseItemBehavior {
    fn is_optional(self) -> bool {
        use DispenseItemBehavior::*;
        matches!(self, FlintAndSteel | BoneMeal | Tnt | WitherSkeletonSkull | CarvedPumpkin | ShulkerBox | GlassBottle | Glowstone | Shears | Brush | Honeycomb)
    }

    pub fn dispense(self, level: &mut ServerLevel, source: &BlockSource, dispensed: ItemStack) -> ItemStack {
        let mut success = true;
        let dispensed_item = dispensed.copy();
        let result = self.execute(level, source, dispensed, &mut success);
        if self == DispenseItemBehavior::Projectile {
            projectile_dispense_behavior::play_sound(level, source, &dispensed_item);
        } else {
            let event = if self.is_optional() && !success { SOUND_DISPENSER_FAIL } else { SOUND_DISPENSER_DISPENSE };
            level.level_event(None, event, source.pos, 0);
        }
        play_default_animation(level, source, source.facing());
        result
    }

    fn execute(self, level: &mut ServerLevel, source: &BlockSource, dispensed: ItemStack, success: &mut bool) -> ItemStack {
        use DispenseItemBehavior::*;
        match self {
            Default => default_dispense_item_behavior::execute(level, source, dispensed),
            FilledBucket => execute_filled_bucket(level, source, dispensed),
            Bucket => execute_bucket(level, source, dispensed),
            FlintAndSteel => flint_and_steel_dispense_item_behavior::execute(level, source, dispensed, success),
            BoneMeal => execute_bone_meal(level, source, dispensed, success),
            Tnt => execute_tnt(level, source, dispensed, success),
            WitherSkeletonSkull | CarvedPumpkin => {
                let mut dispensed = dispensed;
                *success = equipment_dispense_item_behavior::dispense_equipment(level, source, &mut dispensed);
                dispensed
            }
            ShulkerBox => shulker_box_dispense_behavior::execute(level, source, dispensed, success),
            GlassBottle => execute_glass_bottle(level, source, dispensed, success),
            Glowstone => execute_glowstone(level, source, dispensed, success),
            Shears => shears_dispense_item_behavior::execute(level, source, dispensed, success),
            Brush => {
                *success = false;
                dispensed
            }
            Honeycomb => execute_honeycomb(level, source, dispensed, success),
            Potion => execute_potion(level, source, dispensed),
            Equipment => equipment_dispense_item_behavior::execute(level, source, dispensed),
            SpawnEgg => spawn_egg_item_behavior::execute(level, source, dispensed),
            Projectile => projectile_dispense_behavior::execute(level, source, dispensed),
        }
    }
}

fn empty_contents(level: &mut ServerLevel, item: Item, pos: crisp_core::BlockPos) -> bool {
    if let Some(bucket) = BucketItem::for_item(item) {
        return bucket.empty_contents(level, None, pos, None);
    }
    let Some(block) = item.info().block else { return false };
    if level.is_in_valid_bounds(pos) && level.block_state(pos).is_air() {
        level.set_block(pos, block.default_state(), UPDATE_ALL);
        let fields: serde_json::Value = serde_json::from_str(item.info().fields).unwrap_or_default();
        if let Some(place_sound) = fields["placeSound"].as_str() {
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, place_sound), SoundSource::Blocks, 1.0, 1.0);
        }
        return true;
    }
    false
}

fn execute_filled_bucket(level: &mut ServerLevel, source: &BlockSource, dispensed: ItemStack) -> ItemStack {
    let target = target(source);
    if empty_contents(level, dispensed.item, target) {
        consume_with_remainder(level, source, dispensed, ItemStack::new(items::BUCKET, 1))
    } else {
        DispenseItemBehavior::Default.dispense(level, source, dispensed)
    }
}

fn execute_bucket(level: &mut ServerLevel, source: &BlockSource, dispensed: ItemStack) -> ItemStack {
    let target = target(source);
    let block_state = level.block_state(target);
    let behaviour = block::of(block_state);
    if !behaviour.is_bucket_pickup(block_state) {
        return default_dispense_item_behavior::execute(level, source, dispensed);
    }
    match behaviour.pickup_block(None, level, target, block_state).filter(|pickup| !pickup.is_empty()) {
        Some(pickup) => consume_with_remainder(level, source, dispensed, ItemStack::new(pickup.item, 1)),
        None => default_dispense_item_behavior::execute(level, source, dispensed),
    }
}

fn execute_bone_meal(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    *success = true;
    let target = target(source);
    if !bone_meal_item::grow_crop(&mut dispensed, level, target) && !bone_meal_item::grow_water_plant(&mut dispensed, level, target, None) {
        *success = false;
    } else {
        level.level_event(None, PARTICLES_AND_SOUND_PLANT_GROWTH, target, 15);
    }
    dispensed
}

fn execute_tnt(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    if !level.game_rules.get_bool(game_rule::TNT_EXPLODES) {
        *success = false;
        return dispensed;
    }
    let target = target(source);
    let tnt = PrimedTnt::new(level, Vec3::new(target.x as f64 + 0.5, target.y as f64, target.z as f64 + 0.5), None);
    let (x, y, z) = (tnt.entity.x(), tnt.entity.y(), tnt.entity.z());
    level.add_fresh_entity(LevelEntity::PrimedTnt(tnt));
    level.play_sound_at(None, x, y, z, registry_id(registry::SOUND_EVENT, "minecraft:entity.tnt.primed"), SoundSource::Blocks, 1.0, 1.0);
    dispensed.shrink(1);
    *success = true;
    dispensed
}

fn execute_glass_bottle(level: &mut ServerLevel, source: &BlockSource, dispensed: ItemStack, success: &mut bool) -> ItemStack {
    *success = false;
    let target = target(source);
    let state = level.block_state(target);
    if state.is(&block_tags::BEEHIVES) && state.has(LEVEL_HONEY) && state.block().is_instance_of("BeehiveBlock") && state.get(LEVEL_HONEY) >= 5 {
        beehive_block::release_bees_and_reset_honey_level(level, state, target, None, BeeReleaseStatus::BeeReleased);
        *success = true;
        consume_with_remainder(level, source, dispensed, ItemStack::new(items::HONEY_BOTTLE, 1))
    } else if level.fluid_state(target).is(&fluid_tags::WATER) {
        *success = true;
        consume_with_remainder(level, source, dispensed, water_potion())
    } else {
        default_dispense_item_behavior::execute(level, source, dispensed)
    }
}

fn execute_glowstone(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    let pos = target(source);
    let block_state = level.block_state(pos);
    *success = true;
    if block_state.is_block(blocks::RESPAWN_ANCHOR) {
        if block_state.get(CHARGE) != 4 {
            respawn_anchor_block::charge(level, pos, block_state);
            dispensed.shrink(1);
        } else {
            *success = false;
        }
        dispensed
    } else {
        default_dispense_item_behavior::execute(level, source, dispensed)
    }
}

fn execute_honeycomb(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    let pos = target(source);
    let block_state = level.block_state(pos);
    match honeycomb_item::get_waxed(block_state) {
        Some(waxed) => {
            level.set_block(pos, waxed, UPDATE_ALL);
            level.level_event(None, PARTICLES_WAX_ON, pos, 0);
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:item.honeycomb.wax_on"), SoundSource::Blocks, 1.0, 1.0);
            dispensed.shrink(1);
            *success = true;
            dispensed
        }
        None => default_dispense_item_behavior::execute(level, source, dispensed),
    }
}

fn execute_potion(level: &mut ServerLevel, source: &BlockSource, dispensed: ItemStack) -> ItemStack {
    if !is_water_potion(&dispensed) {
        return DispenseItemBehavior::Default.dispense(level, source, dispensed);
    }
    let pos = source.pos;
    let target = target(source);
    if !level.block_state(target).is(&block_tags::CONVERTIBLE_TO_MUD) {
        return DispenseItemBehavior::Default.dispense(level, source, dispensed);
    }
    for _ in 0..5 {
        let x = pos.x as f64 + level.random.next_double();
        let z = pos.z as f64 + level.random.next_double();
        level.send_particles("minecraft:splash", |_| {}, x, (pos.y + 1) as f64, z, 1, (0.0, 0.0, 0.0), 1.0);
    }
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:item.bottle.empty"), SoundSource::Blocks, 1.0, 1.0);
    level.set_block(target, blocks::MUD.default_state(), UPDATE_ALL);
    consume_with_remainder(level, source, dispensed, ItemStack::new(items::GLASS_BOTTLE, 1))
}
