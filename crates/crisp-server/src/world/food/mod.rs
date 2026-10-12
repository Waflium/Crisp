use crisp_data::FoodProperties;
use crisp_data::damage_types;
use crisp_data::game_rule;
use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;

pub mod constants {
    pub const MAX_FOOD: i32 = 20;
    pub const EXHAUSTION_JUMP: f32 = 0.05;
    pub const EXHAUSTION_SPRINT_JUMP: f32 = 0.2;
    pub const EXHAUSTION_MINE: f32 = 0.005;
    pub const EXHAUSTION_SPRINT: f32 = 0.1;
    pub const EXHAUSTION_SWIM: f32 = 0.01;
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FoodData {
    pub food_level: i32,
    pub saturation_level: f32,
    pub exhaustion_level: f32,
    pub tick_timer: i32,
}

impl Default for FoodData {
    fn default() -> FoodData {
        FoodData { food_level: 20, saturation_level: 5.0, exhaustion_level: 0.0, tick_timer: 0 }
    }
}

impl FoodData {
    fn add(&mut self, food: i32, saturation: f32) {
        self.food_level = (food + self.food_level).clamp(0, constants::MAX_FOOD);
        let max = self.food_level as f32;
        let value = saturation + self.saturation_level;
        self.saturation_level = if value < 0.0 { 0.0 } else { value.min(max) };
    }

    pub fn eat(&mut self, food: &FoodProperties) {
        self.add(food.nutrition, food.saturation);
    }

    pub fn eat_with_modifier(&mut self, food: i32, saturation_modifier: f32) {
        self.add(food, food as f32 * saturation_modifier * 2.0);
    }

    pub fn has_enough_food(&self) -> bool {
        self.food_level as f32 > 6.0
    }

    pub fn needs_food(&self) -> bool {
        self.food_level < 20
    }

    pub fn add_exhaustion(&mut self, amount: f32) {
        self.exhaustion_level = (self.exhaustion_level + amount).min(40.0);
    }

    pub fn load(nbt: &Nbt) -> FoodData {
        let int = |key: &str, default: i32| nbt.get(key).and_then(Nbt::as_i64).map_or(default, |v| v as i32);
        let float = |key: &str, default: f32| nbt.get(key).and_then(Nbt::as_f64).map_or(default, |v| v as f32);
        FoodData {
            food_level: int("foodLevel", 20),
            tick_timer: int("foodTickTimer", 0),
            saturation_level: float("foodSaturationLevel", 5.0),
            exhaustion_level: float("foodExhaustionLevel", 0.0),
        }
    }

    pub fn save(&self, out: &mut Vec<(String, Nbt)>) {
        out.push(("foodLevel".into(), Nbt::Int(self.food_level)));
        out.push(("foodTickTimer".into(), Nbt::Int(self.tick_timer)));
        out.push(("foodSaturationLevel".into(), Nbt::Float(self.saturation_level)));
        out.push(("foodExhaustionLevel".into(), Nbt::Float(self.exhaustion_level)));
    }
}

impl ServerLevel {
    pub fn tick_food(&mut self, id: EntityId) {
        let difficulty = self.difficulty;
        let natural_regen = self.game_rules.get_bool(game_rule::NATURAL_HEALTH_REGENERATION);
        let is_hurt = self.is_hurt(id);
        let food = &mut self.player_mut(id).food_data;
        if food.exhaustion_level > 4.0 {
            food.exhaustion_level -= 4.0;
            if food.saturation_level > 0.0 {
                food.saturation_level = (food.saturation_level - 1.0).max(0.0);
            } else if difficulty != Difficulty::Peaceful {
                food.food_level = (food.food_level - 1).max(0);
            }
        }
        if natural_regen && food.saturation_level > 0.0 && is_hurt && food.food_level >= 20 {
            food.tick_timer += 1;
            if food.tick_timer >= 10 {
                let saturation_spent = food.saturation_level.min(6.0);
                self.heal(id, saturation_spent / 6.0);
                let food = &mut self.player_mut(id).food_data;
                food.add_exhaustion(saturation_spent);
                food.tick_timer = 0;
            }
        } else if natural_regen && food.food_level >= 18 && is_hurt {
            food.tick_timer += 1;
            if food.tick_timer >= 80 {
                self.heal(id, 1.0);
                let food = &mut self.player_mut(id).food_data;
                food.add_exhaustion(6.0);
                food.tick_timer = 0;
            }
        } else if food.food_level <= 0 {
            food.tick_timer += 1;
            if food.tick_timer >= 80 {
                let health = self.player(id).health;
                if health > 10.0 || difficulty == Difficulty::Hard || health > 1.0 && difficulty == Difficulty::Normal {
                    self.hurt_server(id, DamageSource::new(damage_types::STARVE), 1.0);
                }
                self.player_mut(id).food_data.tick_timer = 0;
            }
        } else {
            food.tick_timer = 0;
        }
    }

    pub fn cause_food_exhaustion(&mut self, id: EntityId, amount: f32) {
        let p = self.player_mut(id);
        if !p.abilities.invulnerable {
            p.food_data.add_exhaustion(amount);
        }
    }

    pub fn is_hurt(&self, id: EntityId) -> bool {
        let health = self.player(id).health;
        health > 0.0 && health < self.max_health(id)
    }

    pub fn can_eat(&self, id: EntityId, can_always_eat: bool) -> bool {
        let p = self.player(id);
        p.abilities.invulnerable || can_always_eat || p.food_data.needs_food()
    }
}
