use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use super::abstract_arrow::{self, AbstractArrow};
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::data::DataValue;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::{self, PotionContents};
use crate::world::item::component as dc;

const ENTITY_TYPE: &str = "minecraft:arrow";
const EXPOSED_POTION_DECAY_TIME: i32 = 600;
const NO_EFFECT_COLOR: i32 = -1;
const ID_EFFECT_COLOR: usize = 11;
const EVENT_POTION_PUFF: u8 = 0;

pub struct Arrow {
    pub abstract_arrow: AbstractArrow,
}

impl Arrow {
    pub fn of_type() -> Projectile {
        let mut data = abstract_arrow::defaults();
        data.push(DataValue::Int(NO_EFFECT_COLOR));
        let arrow = Arrow { abstract_arrow: AbstractArrow::new(ItemStack::new(items::ARROW, 1)) };
        Projectile::new(ENTITY_TYPE, abstract_arrow::DIMENSIONS, data, ProjectileKind::Arrow(arrow))
    }

    pub fn at(level: &mut ServerLevel, x: f64, y: f64, z: f64, pickup_item_stack: &ItemStack, fired_from_weapon: Option<&ItemStack>) -> Projectile {
        let mut projectile = abstract_arrow::at(level, Arrow::of_type(), x, y, z, pickup_item_stack, fired_from_weapon);
        projectile.arrow_update_color();
        projectile
    }

    pub fn from_owner(level: &mut ServerLevel, owner: EntityId, pickup_item_stack: &ItemStack, fired_from_weapon: Option<&ItemStack>) -> Projectile {
        let mut projectile = abstract_arrow::from_owner(level, Arrow::of_type(), owner, pickup_item_stack, fired_from_weapon);
        projectile.arrow_update_color();
        projectile
    }
}

fn is_empty_potion_contents(contents: Option<&Nbt>) -> bool {
    contents.is_none_or(|contents| matches!(contents, Nbt::Compound(fields) if fields.is_empty()))
}

impl Projectile {
    fn arrow_potion_contents(&self) -> Option<&Nbt> {
        self.abstract_arrow().expect("an arrow").pickup_item_stack.get(dc::POTION_CONTENTS)
    }

    fn arrow_potion_duration_scale(&self) -> f32 {
        self.abstract_arrow().expect("an arrow").pickup_item_stack.get(dc::POTION_DURATION_SCALE).and_then(Nbt::as_f64).map_or(1.0, |scale| scale as f32)
    }

    pub fn arrow_update_color(&mut self) {
        let color = match self.arrow_potion_contents() {
            contents if is_empty_potion_contents(contents) => NO_EFFECT_COLOR,
            contents => PotionContents::new(contents.expect("potion contents")).get_color(),
        };
        self.entity.data.set(ID_EFFECT_COLOR, DataValue::Int(color));
    }

    pub fn arrow_add_effect(&mut self, effect: &MobEffectInstance) {
        let contents = potion_contents::with_effect_added(self.arrow_potion_contents(), effect);
        self.abstract_arrow_mut().expect("an arrow").pickup_item_stack.set(dc::POTION_CONTENTS, Some(contents));
        self.arrow_update_color();
    }
}

impl ServerLevel {
    pub fn arrow_tick(&mut self, id: EntityId) {
        self.abstract_arrow_tick(id);
        let projectile = self.projectile(id);
        let in_ground_time = projectile.abstract_arrow().expect("an arrow").in_ground_time;
        if projectile.is_in_ground() && in_ground_time != 0 && !is_empty_potion_contents(projectile.arrow_potion_contents()) && in_ground_time >= EXPOSED_POTION_DECAY_TIME {
            self.broadcast_entity_event(id, EVENT_POTION_PUFF);
            self.projectile_mut(id).set_pickup_item_stack(ItemStack::new(items::ARROW, 1));
        }
    }

    pub fn arrow_do_post_hurt_effects(&mut self, id: EntityId, mob: EntityId) {
        let effect_source = self.projectile_get_effect_source(id);
        let projectile = self.projectile(id);
        let Some(contents) = projectile.arrow_potion_contents().cloned() else { return };
        let duration_scale = projectile.arrow_potion_duration_scale();
        for effect in PotionContents::new(&contents).for_each_effect(duration_scale) {
            self.add_effect(mob, effect, Some(effect_source));
        }
    }
}
