use crisp_core::AABB;
use crisp_protocol::nbt::Nbt;

use super::entity_predicate::EntityPredicate;
use crate::advancements::advancement_progress::AdvancementProgress;
use crate::server::server_advancement_manager::ServerAdvancementManager;
use crate::advancements::predicates::food_predicate::FoodPredicate;
use crate::advancements::predicates::game_type_predicate::GameTypePredicate;
use crate::advancements::predicates::input_predicate::InputPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::server::level::ServerLevel;
use crate::stats::stat::Stat;
use crate::stats::stat_type::StatType;
use crate::stats::stats_counter::StatsCounter;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

pub const LOOKING_AT_RANGE: f64 = 100.0;

#[derive(Debug)]
struct StatMatcher {
    stat: Stat,
    range: Ints,
}

impl StatMatcher {
    fn decode(tag: &Nbt) -> DataResult<StatMatcher> {
        let stat_type = codec::decode_field(tag, "type", codec::identifier)?;
        let stat_type = StatType::by_id(crate::world::item::component::codec::registry_id("minecraft:stat_type", &stat_type)?).expect("a stat type");
        let value = codec::decode_field(tag, "stat", codec::identifier)?;
        let value = crate::world::item::component::codec::registry_id(stat_type.registry_key(), &value)?;
        Ok(StatMatcher { stat: stat_type.get(value as u16), range: codec::decode_or(tag, "value", Ints::ANY, Ints::decode)? })
    }

    fn matches(&self, counter: &impl StatsCounter) -> bool {
        self.range.matches(counter.get_value(self.stat))
    }
}

#[derive(Debug)]
enum AdvancementPredicate {
    Done(bool),
    Criterions(Vec<(String, bool)>),
}

impl AdvancementPredicate {
    fn decode(tag: &Nbt) -> DataResult<AdvancementPredicate> {
        match codec::either(tag, codec::bool_value, |t| codec::unbounded_map(t, |k| Ok(k.to_string()), codec::bool_value))? {
            Ok(done) => Ok(AdvancementPredicate::Done(done)),
            Err(criterions) => Ok(AdvancementPredicate::Criterions(criterions)),
        }
    }

    fn test(&self, progress: &AdvancementProgress) -> bool {
        match self {
            AdvancementPredicate::Done(state) => progress.is_done() == *state,
            AdvancementPredicate::Criterions(criterions) => {
                criterions.iter().all(|(name, done)| progress.get_criterion(name).is_some_and(|criterion| criterion.is_done() == *done))
            }
        }
    }
}

#[derive(Debug)]
pub struct PlayerPredicate {
    level: Ints,
    food: FoodPredicate,
    game_type: GameTypePredicate,
    stats: Vec<StatMatcher>,
    recipes: Vec<(String, bool)>,
    advancements: Vec<(String, AdvancementPredicate)>,
    looking_at: Option<Box<EntityPredicate>>,
    input: Option<InputPredicate>,
}

impl PlayerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PlayerPredicate> {
        codec::map(tag)?;
        Ok(PlayerPredicate {
            level: codec::decode_or(tag, "level", Ints::ANY, Ints::decode)?,
            food: codec::decode_or(tag, "food", FoodPredicate::ANY, FoodPredicate::decode)?,
            game_type: codec::decode_or(tag, "gamemode", GameTypePredicate::any(), GameTypePredicate::decode)?,
            stats: codec::decode_or(tag, "stats", Vec::new(), |t| codec::list_of(t, StatMatcher::decode))?,
            recipes: codec::decode_or(tag, "recipes", Vec::new(), |t| codec::unbounded_map(t, crate::resources::identifier::read, codec::bool_value))?,
            advancements: codec::decode_or(tag, "advancements", Vec::new(), |t| {
                codec::unbounded_map(t, crate::resources::identifier::read, AdvancementPredicate::decode)
            })?,
            looking_at: codec::decode_optional(tag, "looking_at", |t| Ok(Box::new(EntityPredicate::decode(t)?)))?,
            input: codec::decode_optional(tag, "input", InputPredicate::decode)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        let Some(player) = level.players.get(&entity) else { return false };
        if !self.level.matches(player.experience.level) {
            return false;
        }
        if !self.food.matches(&player.food_data) {
            return false;
        }
        if !self.game_type.matches(player.game_mode.game_type) {
            return false;
        }
        if !self.stats.iter().all(|stat| stat.matches(&player.stats)) {
            return false;
        }
        if self.recipes.iter().any(|(recipe, present)| player.recipe_book.contains(recipe) != *present) {
            return false;
        }
        if !self.advancements.is_empty() {
            let server_advancements = ServerAdvancementManager::get_bound();
            for (id, predicate) in &self.advancements {
                let Some(advancement) = server_advancements.get(id) else { return false };
                if !predicate.test(level.player_mut(entity).advancements.get_or_start_progress(advancement)) {
                    return false;
                }
            }
        }
        let player = &level.players[&entity];
        let last_client_input = player.last_client_input;
        if let Some(looking_at) = &self.looking_at {
            let from = level.entity(entity).eye_position();
            let view_vec = level.entity(entity).view_vector();
            let to = from.add(view_vec.x * LOOKING_AT_RANGE, view_vec.y * LOOKING_AT_RANGE, view_vec.z * LOOKING_AT_RANGE);
            let area = AABB::from_corners(from, to).inflate_all(1.0);
            let Some((looking_at_entity, _)) = level.get_entity_hit_result(entity, from, to, &area, |level, e| !level.is_spectator(e), 0.0) else {
                return false;
            };
            let position = level.entity(entity).position();
            if !looking_at.matches(level, Some(position), Some(looking_at_entity)) || !level.has_line_of_sight(entity, looking_at_entity) {
                return false;
            }
        }
        self.input.as_ref().is_none_or(|input| input.matches(last_client_input))
    }
}
