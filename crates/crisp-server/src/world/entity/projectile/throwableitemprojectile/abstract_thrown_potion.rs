use crisp_core::{BlockPos, Direction};
use crisp_data::block_tags;
use crisp_data::properties::LIT;
use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::server::level::events::{PARTICLES_INSTANT_POTION_SPLASH, PARTICLES_SPELL_POTION_SPLASH, SOUND_EXTINGUISH_FIRE, SOUND_INSTANT_POTION_SPLASH, SOUND_SPELL_POTION_SPLASH};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::item::alchemy::potion_contents::{self, PotionContents};
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::block::{BlockHitResult, abstract_candle_block, campfire_block};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::phys::hit_result::HitResult;

pub const SPLASH_RANGE_SQ: f64 = 16.0;

static EMPTY: Nbt = Nbt::Compound(Vec::new());

impl ServerLevel {
    pub fn abstract_thrown_potion_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        self.projectile_base_on_hit_block(id, hit_result);
        let potion_item_stack = self.projectile(id).get_item();
        let hit_direction = hit_result.direction;
        let block_effect_pos = hit_result.block_pos.relative(hit_direction);
        let potion = PotionContents::new(potion_item_stack.get(dc::POTION_CONTENTS).unwrap_or(&EMPTY));
        if potion.is_tag("minecraft:douses_fire") {
            self.douse_fire(id, block_effect_pos);
            self.douse_fire(id, block_effect_pos.relative(hit_direction.opposite()));
            for direction in Direction::PLANE_HORIZONTAL {
                self.douse_fire(id, block_effect_pos.relative(direction));
            }
        }
    }

    pub fn abstract_thrown_potion_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let potion_item_stack = self.projectile(id).get_item();
        let contents = potion_item_stack.get(dc::POTION_CONTENTS).cloned().unwrap_or_else(|| EMPTY.clone());
        let potion = PotionContents::new(&contents);
        self.affect_entities_around(id, &potion);
        if potion.has_effects() {
            match self.projectile(id).kind {
                ProjectileKind::ThrownSplashPotion(_) => self.thrown_splash_potion_on_hit_as_potion(id, &potion_item_stack, hit_result),
                _ => self.thrown_lingering_potion_on_hit_as_potion(id, &potion_item_stack, hit_result),
            }
        }
        let block_position = self.entity(id).block_position();
        let silent = self.entity(id).is_silent();
        if potion.potion().is_some_and(potion_contents::has_instant_effects) {
            self.level_event(None, PARTICLES_INSTANT_POTION_SPLASH, block_position, potion.get_color());
            if !silent {
                self.level_event(None, SOUND_INSTANT_POTION_SPLASH, block_position, 0);
            }
        } else {
            self.level_event(None, PARTICLES_SPELL_POTION_SPLASH, block_position, potion.get_color());
            if !silent {
                self.level_event(None, SOUND_SPELL_POTION_SPLASH, block_position, 0);
            }
        }
        self.entity_mut(id).discard();
    }

    fn affect_entities_around(&mut self, id: EntityId, potion: &PotionContents) {
        let hurts_water_sensitive_entities = potion.is_tag("minecraft:hurts_water_sensitive_entities");
        let extinguishes_entities = potion.is_tag("minecraft:extinguishes_entities");
        let aabb = self.entity(id).bounding_box().inflate(4.0, 2.0, 4.0);
        if hurts_water_sensitive_entities || extinguishes_entities {
            let entities: Vec<EntityId> = self
                .get_entities(None, &aabb)
                .into_iter()
                .filter(|&entity| self.is_living(entity) && (self.is_sensitive_to_water(entity) || self.is_on_fire(entity)))
                .collect();
            for entity in entities {
                let dist = self.entity(id).distance_to_sqr(self.entity(entity));
                if dist < SPLASH_RANGE_SQ {
                    if hurts_water_sensitive_entities && self.is_sensitive_to_water(entity) {
                        let source = DamageSource::source(crisp_data::damage_types::INDIRECT_MAGIC, Some(id), self.projectile_get_owner(id));
                        self.hurt_server(entity, source, 1.0);
                    }
                    if extinguishes_entities && self.is_on_fire(entity) && self.is_alive(entity) {
                        self.extinguish_fire(entity);
                    }
                }
            }
        }
    }

    fn douse_fire(&mut self, id: EntityId, pos: BlockPos) {
        let block_state = self.block_state(pos);
        if block_state.is(&block_tags::FIRE) {
            self.destroy_block(pos, false, Some(id), 512);
        } else if abstract_candle_block::is_lit(block_state) {
            abstract_candle_block::extinguish(block_state, self, pos);
        } else if campfire_block::is_lit_campfire(block_state) {
            self.level_event(None, SOUND_EXTINGUISH_FIRE, pos, 0);
            self.set_block(pos, block_state.set(LIT, false), UPDATE_ALL);
        }
    }
}
