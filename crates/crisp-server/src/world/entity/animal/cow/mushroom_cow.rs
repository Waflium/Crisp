use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{blocks, items, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::abstract_cow::{AbstractCow, CowSoundVariant};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::COW_ATTRIBUTES;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::variant::variant_utils;
use crate::world::entity::{Entity, EntityDimensions, EntityId, ItemEntity, LevelEntity, sound_event};
use crate::world::item::component as dc;
use crate::world::item::{ItemStack, item_utils};
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::block::suspicious_effect_holder;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_TYPE: usize = 18;
const MUTATE_CHANCE: i32 = 1024;
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.45, 0.7).with_eye_height(0.69).with_passenger_attachments(&[crisp_core::Vec3::new(0.0, 0.75, 0.0)]);
const SHEAR_MOOSHROOM: &str = "minecraft:shearing/mooshroom";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Red,
    Brown,
}

impl Variant {
    pub const DEFAULT: Variant = Variant::Red;

    pub fn id(self) -> i32 {
        match self {
            Variant::Red => 0,
            Variant::Brown => 1,
        }
    }

    pub fn by_id(id: i32) -> Variant {
        if id >= 1 { Variant::Brown } else { Variant::Red }
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Variant::Red => "red",
            Variant::Brown => "brown",
        }
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        match name {
            "red" => Some(Variant::Red),
            "brown" => Some(Variant::Brown),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct MushroomCow {
    pub abstract_cow: AbstractCow,
    pub stew_effects: Option<Nbt>,
    pub last_lightning_bolt_uuid: Option<u128>,
}

pub fn get_variant(entity: &Entity) -> Variant {
    match entity.data.get(DATA_TYPE) {
        DataValue::Int(id) => Variant::by_id(*id),
        _ => Variant::DEFAULT,
    }
}

pub fn set_variant(entity: &mut Entity, variant: Variant) {
    entity.data.set(DATA_TYPE, DataValue::Int(variant.id()));
}

pub fn get_sound_set() -> &'static CowSoundVariant {
    let classic = variant_utils::read_variant(super::cow::SOUND_VARIANT_REGISTRY, "minecraft:classic").unwrap_or(0);
    super::cow::sound_variant(classic)
}

impl MushroomCow {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Int(Variant::DEFAULT.id()));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:mooshroom"), super::abstract_cow::DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&COW_ATTRIBUTES), MobKind::MushroomCow(MushroomCow::default()));
        Animal::init(&mut mob);
        mob
    }
}

fn mushroom_cow(level: &mut ServerLevel, id: EntityId) -> &mut MushroomCow {
    match &mut level.mob_mut(id).expect("a mooshroom").kind {
        MobKind::MushroomCow(mushroom_cow) => mushroom_cow,
        _ => unreachable!("not a mooshroom"),
    }
}

impl ServerLevel {
    pub fn mushroom_cow_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        if PathNavigationRegion::new(self).get_block_state(pos.below()).is_block(blocks::MYCELIUM) { 10.0 } else { self.get_pathfinding_cost_from_light_levels(pos) }
    }

    pub fn mushroom_cow_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        let lightning_bolt_uuid = self.entity(lightning_bolt).uuid;
        if mushroom_cow(self, id).last_lightning_bolt_uuid != Some(lightning_bolt_uuid) {
            let variant = if get_variant(self.entity(id)) == Variant::Red { Variant::Brown } else { Variant::Red };
            set_variant(self.entity_mut(id), variant);
            mushroom_cow(self, id).last_lightning_bolt_uuid = Some(lightning_bolt_uuid);
            self.entity_play_sound(id, sound_event("minecraft:entity.mooshroom.convert"), 2.0, 1.0);
        }
    }

    pub fn mushroom_cow_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::BOWL) && !self.is_baby(id) {
            let stew = match mushroom_cow(self, id).stew_effects.take() {
                Some(stew_effects) => {
                    let mut stew = ItemStack::new(items::SUSPICIOUS_STEW, 1);
                    let stew_effects = dc::SUSPICIOUS_STEW_EFFECTS.decode(&stew_effects).unwrap_or(stew_effects);
                    stew.set(dc::SUSPICIOUS_STEW_EFFECTS, Some(stew_effects));
                    stew
                }
                None => ItemStack::new(items::MUSHROOM_STEW, 1),
            };
            let is_suspicious = stew.is(items::SUSPICIOUS_STEW);
            let bowl_or_stew = item_utils::create_filled_result(self, player, hand, item_stack, stew, false);
            self.player_mut(player).set_item_in_hand(hand, Some(bowl_or_stew));
            let milk_sound = if is_suspicious { "minecraft:entity.mooshroom.suspicious_milk" } else { "minecraft:entity.mooshroom.milk" };
            self.entity_play_sound(id, sound_event(milk_sound), 1.0, 1.0);
            return InteractionResult::SUCCESS;
        }
        if item_stack.is(items::SHEARS) && self.mushroom_cow_ready_for_shearing(id) {
            self.mushroom_cow_shear(id, SoundSource::Players, &item_stack);
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            return InteractionResult::SUCCESS;
        }
        if get_variant(self.entity(id)) == Variant::Brown && !self.is_baby(id) {
            let Some(effects_from_item_stack) = suspicious_effect_holder::try_get(item_stack.get_item()) else {
                return self.abstract_cow_mob_interact(id, player, hand);
            };
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.get_y(0.5), e.z());
            if mushroom_cow(self, id).stew_effects.is_some() {
                self.send_particles_with_speeds("minecraft:smoke", |_| {}, x, y, z, 2, (0.5, 0.0, 0.5), (0.0, 0.2, 0.0), 2);
            } else {
                item_stack.consume(1, self, Some(player));
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
                let particle = |p: &mut crisp_protocol::Writer| {
                    p.i32(-1);
                    p.f32(1.0);
                };
                self.send_particles_with_speeds("minecraft:effect", particle, x, y, z, 4, (0.5, 0.0, 0.5), (0.0, 0.2, 0.0), 2);
                mushroom_cow(self, id).stew_effects = Some(effects_from_item_stack);
                self.entity_play_sound(id, sound_event("minecraft:entity.mooshroom.eat"), 2.0, 1.0);
            }
            return InteractionResult::SUCCESS;
        }
        self.abstract_cow_mob_interact(id, player, hand)
    }

    pub fn mushroom_cow_shear(&mut self, id: EntityId, sound_source: SoundSource, tool: &ItemStack) {
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.mooshroom.shear"), sound_source, 1.0, 1.0);
        let params = ConversionParams::single(self, id, false, false);
        self.convert_to(id, "minecraft:cow", params, |level, _cow| {
            let e = level.entity(id);
            let (x, y, z) = (e.x(), e.get_y(0.5), e.z());
            level.send_particles("minecraft:explosion", |_| {}, x, y, z, 1, (0.0, 0.0, 0.0), 0.0);
            level.drop_from_shearing_loot_table(id, SHEAR_MOOSHROOM, tool, |l, drop| {
                for _ in 0..drop.count {
                    let e = l.entity(id);
                    let pos = crisp_core::Vec3::new(e.x(), e.get_y(1.0), e.z());
                    l.add_fresh_entity(LevelEntity::Item(ItemEntity::new(pos, drop.copy_with_count(1))));
                }
            });
        });
    }

    pub fn mushroom_cow_ready_for_shearing(&self, id: EntityId) -> bool {
        !self.is_baby(id)
    }

    pub fn mushroom_cow_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(MushroomCow::create());
        let variant = self.get_offspring_variant(id, partner);
        set_variant(&mut baby.entity, variant);
        Some(baby)
    }

    fn get_offspring_variant(&mut self, id: EntityId, mate: EntityId) -> Variant {
        let variant = get_variant(self.entity(id));
        let mate_variant = get_variant(self.entity(mate));
        let random = &mut self.entity_mut(id).random;
        if variant == mate_variant && random.next_int_bounded(MUTATE_CHANCE) == 0 {
            if variant == Variant::Brown { Variant::Red } else { Variant::Brown }
        } else if random.next_boolean() {
            variant
        } else {
            mate_variant
        }
    }
}
