use std::sync::{LazyLock, Mutex};

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{AABB, Vec3, mth};
use crisp_data::registry;

use super::super::entity_argument::{ERROR_NOT_SINGLE_ENTITY, ERROR_SELECTORS_NOT_ALLOWED, NO_ENTITIES_FOUND, NO_PLAYERS_FOUND};
use crate::commands::arguments::nbt_path_argument::compare_nbt;
use crate::advancements::predicates::min_max_bounds::{Doubles, Ints};
use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Component, HoverEvent};
use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::world::entity::{Entity, EntityId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Arbitrary,
    Nearest,
    Furthest,
    Random,
}

static SHUFFLE_RANDOM: LazyLock<Mutex<LegacyRandomSource>> = LazyLock::new(|| Mutex::new(LegacyRandomSource::create()));

impl Order {
    fn apply(self, server: &Server, pos: Vec3, entities: &mut [EntityId]) {
        let distance = |id: &EntityId| entity(server, *id).position().distance_to_sqr(pos);
        match self {
            Order::Arbitrary => {}
            Order::Nearest => entities.sort_by(|a, b| distance(a).total_cmp(&distance(b))),
            Order::Furthest => entities.sort_by(|a, b| distance(b).total_cmp(&distance(a))),
            Order::Random => {
                let mut random = SHUFFLE_RANDOM.lock().unwrap();
                for i in (2..=entities.len()).rev() {
                    let j = random.next_int_bounded(i as i32) as usize;
                    entities.swap(i - 1, j);
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum EntityPredicate {
    Alive,
    Name { name: String, inverted: bool },
    GameMode { mode: GameType, inverted: bool },
    Team { name: String, inverted: bool },
    Type { entity_type: i32, inverted: bool },
    TypeTag { tag: String, inverted: bool },
    Tag { tag: String, inverted: bool },
    Nbt { tag: crisp_protocol::nbt::Nbt, inverted: bool },
    Scores(Vec<(String, Ints)>),
    Rotation { min: f32, max: f32, x: bool },
    Level(Ints),
}

impl EntityPredicate {
    fn test(&self, server: &Server, id: EntityId) -> bool {
        let e = entity(server, id);
        match self {
            EntityPredicate::Alive => server.level.is_alive(id),
            EntityPredicate::Name { name, inverted } => (server.entity_name(id) == *name) != *inverted,
            EntityPredicate::GameMode { mode, inverted } => match server.level.players.get(&id) {
                Some(player) => (player.game_mode.game_type == *mode) ^ *inverted,
                None => false,
            },
            EntityPredicate::Team { name, inverted } => (server.level.team_of(id).map_or("", |t| t.name()) == name) != *inverted,
            EntityPredicate::Type { entity_type, inverted } => (e.entity_type == *entity_type) != *inverted,
            EntityPredicate::TypeTag { tag, inverted } => crisp_data::tag("minecraft:entity_type", tag).contains(&e.entity_type) != *inverted,
            EntityPredicate::Tag { tag, inverted } => {
                if tag.is_empty() { e.tags.is_empty() != *inverted } else { e.tags.contains(tag) != *inverted }
            }
            EntityPredicate::Nbt { tag, inverted } => compare_nbt(tag, Some(&server.level.entity_tag_to_compare(id))) != *inverted,
            EntityPredicate::Scores(expected) => {
                let scoreboard = &server.level.scoreboard;
                let holder = server.level.scoreboard_name(id);
                expected.iter().all(|(objective, range)| {
                    scoreboard
                        .get_objective(objective)
                        .and_then(|o| scoreboard.get_player_score_info(&holder, o.id()))
                        .is_some_and(|score| range.matches(score.value))
                })
            }
            &EntityPredicate::Rotation { min, max, x } => {
                let rotation = mth::wrap_degrees(if x { e.x_rot } else { e.y_rot });
                if min > max { rotation >= min || rotation <= max } else { rotation >= min && rotation <= max }
            }
            EntityPredicate::Level(level) => server.level.players.contains_key(&id) && level.matches(0),
        }
    }
}

fn entity(server: &Server, id: EntityId) -> &Entity {
    server.level.entity(id)
}

#[derive(Clone, PartialEq, Debug)]
pub struct EntitySelector {
    pub max_results: i32,
    pub includes_entities: bool,
    pub world_limited: bool,
    pub predicates: Vec<EntityPredicate>,
    pub range: Option<Doubles>,
    pub position: (Option<f64>, Option<f64>, Option<f64>),
    pub aabb: Option<AABB>,
    pub order: Order,
    pub current_entity: bool,
    pub player_name: Option<String>,
    pub entity_uuid: Option<u128>,
    pub entity_type: Option<i32>,
    pub uses_selector: bool,
    pub unsupported: Option<&'static str>,
}

impl EntitySelector {
    pub fn is_self_selector(&self) -> bool {
        self.current_entity
    }

    fn check_permissions(&self, source: &CommandSourceStack) -> Result<(), CommandSyntaxException> {
        if self.uses_selector && !source.has_permission(2) {
            return Err(ERROR_SELECTORS_NOT_ALLOWED.create());
        }
        if let Some(option) = self.unsupported {
            return Err(CommandSyntaxException::new(Component::translatable("command.failed").with_hover(HoverEvent::ShowText(Box::new(
                Component::literal(format!("The {option} selector option is not implemented in Crisp yet")),
            )))));
        }
        Ok(())
    }

    pub fn find_single_entity(&self, server: &Server, source: &CommandSourceStack) -> Result<EntityId, CommandSyntaxException> {
        self.check_permissions(source)?;
        let entities = self.find_entities(server, source)?;
        match entities[..] {
            [] => Err(NO_ENTITIES_FOUND.create()),
            [entity] => Ok(entity),
            _ => Err(ERROR_NOT_SINGLE_ENTITY.create()),
        }
    }

    pub fn find_entities(&self, server: &Server, source: &CommandSourceStack) -> Result<Vec<EntityId>, CommandSyntaxException> {
        self.check_permissions(source)?;
        if !self.includes_entities {
            return self.find_players(server, source);
        }
        if let Some(name) = &self.player_name {
            return Ok(player_by_name(server, name).into_iter().collect());
        }
        if let Some(uuid) = self.entity_uuid {
            let found = server.level.all_entities().find(|&id| entity(server, id).uuid == uuid);
            return Ok(found.into_iter().collect());
        }
        let pos = self.apply_position(source.position);
        let absolute_aabb = self.absolute_aabb(pos);
        if self.current_entity {
            return Ok(source.entity.filter(|&e| server.level.entity_ref(e).is_some() && self.test(server, e, pos, absolute_aabb)).into_iter().collect());
        }
        let mut result = Vec::new();
        self.add_entities(server, &mut result, absolute_aabb, pos);
        Ok(self.sort_and_limit(server, pos, result))
    }

    fn add_entities(&self, server: &Server, result: &mut Vec<EntityId>, aabb: Option<AABB>, pos: Vec3) {
        let limit = self.result_limit();
        if result.len() >= limit {
            return;
        }
        let of_type = |id: &EntityId| self.entity_type.is_none_or(|t| entity(server, *id).entity_type == t);
        let candidates: Vec<EntityId> = match aabb {
            None => server.level.all_entities().filter(of_type).collect(),
            Some(bb) => {
                let section = |v: f64| mth::floor(v) >> 4;
                let (x0, y0, z0) = (section(bb.min_x - 2.0), section(bb.min_y - 4.0), section(bb.min_z - 2.0));
                let (x1, y1, z1) = (section(bb.max_x + 2.0), section(bb.max_y), section(bb.max_z + 2.0));
                let mut in_sections: Vec<(i32, u32, u32, EntityId)> = server
                    .level
                    .all_entities()
                    .filter_map(|id| {
                        let block = entity(server, id).block_position();
                        let (x, y, z) = (block.x >> 4, block.y >> 4, block.z >> 4);
                        let inside = (x0..=x1).contains(&x) && (y0..=y1).contains(&y) && (z0..=z1).contains(&z);
                        inside.then_some((x, (z & 0x3F_FFFF) as u32, (y & 0xF_FFFF) as u32, id))
                    })
                    .collect();
                in_sections.sort_by_key(|&(x, z, y, _)| (x, z, y));
                in_sections.into_iter().map(|e| e.3).filter(|id| of_type(id) && entity(server, *id).bounding_box().intersects(&bb)).collect()
            }
        };
        for id in candidates {
            if self.test(server, id, pos, aabb) {
                result.push(id);
                if result.len() >= limit {
                    return;
                }
            }
        }
    }

    fn result_limit(&self) -> usize {
        if self.order == Order::Arbitrary { self.max_results as usize } else { i32::MAX as usize }
    }

    pub fn find_single_player(&self, server: &Server, source: &CommandSourceStack) -> Result<EntityId, CommandSyntaxException> {
        self.check_permissions(source)?;
        let players = self.find_players(server, source)?;
        match players[..] {
            [player] => Ok(player),
            _ => Err(NO_PLAYERS_FOUND.create()),
        }
    }

    pub fn find_players(&self, server: &Server, source: &CommandSourceStack) -> Result<Vec<EntityId>, CommandSyntaxException> {
        self.check_permissions(source)?;
        if let Some(name) = &self.player_name {
            return Ok(player_by_name(server, name).into_iter().collect());
        }
        if let Some(uuid) = self.entity_uuid {
            return Ok(server.level.players.values().find(|p| p.profile.uuid == uuid).map(|p| p.entity.id).into_iter().collect());
        }
        let pos = self.apply_position(source.position);
        let absolute_aabb = self.absolute_aabb(pos);
        if self.current_entity {
            return Ok(source.player_id(server).filter(|&p| self.test(server, p, pos, absolute_aabb)).into_iter().collect());
        }
        let limit = self.result_limit();
        let mut result = Vec::new();
        for &id in server.level.players.keys() {
            if self.test(server, id, pos, absolute_aabb) {
                result.push(id);
                if result.len() >= limit {
                    if self.world_limited {
                        break;
                    }
                    return Ok(result);
                }
            }
        }
        Ok(self.sort_and_limit(server, pos, result))
    }

    fn apply_position(&self, origin: Vec3) -> Vec3 {
        let (x, y, z) = self.position;
        Vec3::new(x.unwrap_or(origin.x), y.unwrap_or(origin.y), z.unwrap_or(origin.z))
    }

    fn absolute_aabb(&self, pos: Vec3) -> Option<AABB> {
        self.aabb.map(|bb| bb.move_vec(pos))
    }

    fn test(&self, server: &Server, id: EntityId, pos: Vec3, aabb: Option<AABB>) -> bool {
        self.predicates.iter().all(|p| p.test(server, id))
            && aabb.is_none_or(|bb| bb.intersects(&entity(server, id).bounding_box()))
            && self.range.is_none_or(|range| range.matches_sqr(entity(server, id).position().distance_to_sqr(pos)))
    }

    fn sort_and_limit(&self, server: &Server, pos: Vec3, mut result: Vec<EntityId>) -> Vec<EntityId> {
        if result.len() > 1 {
            self.order.apply(server, pos, &mut result);
        }
        result.truncate(self.max_results as usize);
        result
    }
}

pub fn player_by_name(server: &Server, name: &str) -> Option<EntityId> {
    server.level.players.values().find(|p| java_equals_ignore_case(&p.profile.name, name)).map(|p| p.entity.id)
}

pub fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    let (a, b): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    let single = |c: u16, upper: bool| {
        let Some(ch) = char::from_u32(c as u32) else { return c as u32 };
        let mut mapped = if upper { ch.to_uppercase().collect::<Vec<_>>() } else { ch.to_lowercase().collect::<Vec<_>>() };
        if mapped.len() == 1 && (mapped[0] as u32) < 0x10000 { mapped.pop().unwrap() as u32 } else { c as u32 }
    };
    a.len() == b.len()
        && a.iter().zip(&b).all(|(&x, &y)| {
            x == y || {
                let (ux, uy) = (single(x, true), single(y, true));
                ux == uy || single(ux as u16, false) == single(uy as u16, false)
            }
        })
}

pub fn join_names(server: &Server, entities: &[EntityId]) -> Component {
    crate::server::commands::list_players_command::format_list(entities.iter().map(|&e| server.entity_display_name(e)).collect())
}

pub fn player_type() -> i32 {
    crisp_data::registry_id(registry::ENTITY_TYPE, "minecraft:player")
}
