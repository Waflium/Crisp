use std::collections::HashSet;

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, Vec3, mth};

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, Dynamic4CommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::util::java_format_fixed;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

const MAX_ITERATION_COUNT: i32 = 10000;

static ERROR_FAILED_TO_SPREAD_TEAMS: Dynamic4CommandExceptionType =
    Dynamic4CommandExceptionType(|count, x, z, recommended| tr("commands.spreadplayers.failed.teams", vec![count, x, z, recommended]));
static ERROR_FAILED_TO_SPREAD_ENTITIES: Dynamic4CommandExceptionType =
    Dynamic4CommandExceptionType(|count, x, z, recommended| tr("commands.spreadplayers.failed.entities", vec![count, x, z, recommended]));
static ERROR_INVALID_MAX_HEIGHT: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|supplied_max_height, world_min_height| {
    tr("commands.spreadplayers.failed.invalid.height", vec![supplied_max_height, world_min_height])
});

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn spread(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let center = context.get_coordinates("center").position(server, source);
    let max_height = server.levels[source.level].chunks.max_y() + 1;
    let targets = context.get_entities(server, source, "targets")?;
    spread_players(
        server,
        source,
        (center.x as f32, center.z as f32),
        context.get_float("spreadDistance"),
        context.get_float("maxRange"),
        max_height,
        context.get_bool("respectTeams"),
        &targets,
    )
}

pub fn spread_under(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let center = context.get_coordinates("center").position(server, source);
    let targets = context.get_entities(server, source, "targets")?;
    spread_players(
        server,
        source,
        (center.x as f32, center.z as f32),
        context.get_float("spreadDistance"),
        context.get_float("maxRange"),
        context.get_int("maxHeight"),
        context.get_bool("respectTeams"),
        &targets,
    )
}

#[allow(clippy::too_many_arguments)]
fn spread_players(
    server: &mut Server,
    source: &CommandSourceStack,
    center: (f32, f32),
    spread_distance: f32,
    max_range: f32,
    max_height: i32,
    respect_teams: bool,
    entities: &[EntityId],
) -> Result<i32> {
    let min_y = server.levels[source.level].chunks.min_y();
    if max_height < min_y {
        return Err(ERROR_INVALID_MAX_HEIGHT.create(max_height, min_y));
    }
    let mut random = LegacyRandomSource::create();
    let min_x = (center.0 - max_range) as f64;
    let min_z = (center.1 - max_range) as f64;
    let max_x = (center.0 + max_range) as f64;
    let max_z = (center.1 + max_range) as f64;
    let count = if respect_teams { get_number_of_teams(server, entities) } else { entities.len() };
    let mut positions = create_initial_positions(&mut random, count, min_x, min_z, max_x, max_z);
    let level = &mut server.levels[source.level];
    spread_positions(center, spread_distance as f64, level, &mut random, min_x, min_z, max_x, max_z, max_height, &mut positions, respect_teams)?;
    let distance = set_player_positions(server, entities, source.level, &positions, max_height, respect_teams);
    let kind = if respect_teams { "teams" } else { "entities" };
    source.send_success(
        server,
        || {
            Component::translatable_with(
                &format!("commands.spreadplayers.success.{kind}"),
                vec![(positions.len() as i32).into(), center.0.into(), center.1.into(), java_format_fixed(distance, 2).into()],
            )
        },
        true,
    );
    Ok(positions.len() as i32)
}

fn team_of(server: &Server, entity: EntityId) -> Option<String> {
    let level = server.entity_level(entity);
    if !level.is_player(entity) {
        return None;
    }
    level.team_of(entity).map(|team| team.name().to_string())
}

fn get_number_of_teams(server: &Server, players: &[EntityId]) -> usize {
    let teams: HashSet<Option<String>> = players.iter().map(|&player| team_of(server, player)).collect();
    teams.len()
}

#[allow(clippy::too_many_arguments)]
fn spread_positions(
    center: (f32, f32),
    spread_dist: f64,
    level: &mut impl BlockGetter,
    random: &mut impl RandomSource,
    min_x: f64,
    min_z: f64,
    max_x: f64,
    max_z: f64,
    max_height: i32,
    positions: &mut [Position],
    respect_teams: bool,
) -> Result<()> {
    let mut has_collisions = true;
    let mut min_distance = f32::MAX as f64;
    let mut iteration = 0;
    while iteration < MAX_ITERATION_COUNT && has_collisions {
        has_collisions = false;
        min_distance = f32::MAX as f64;
        for i in 0..positions.len() {
            let position = positions[i];
            let mut neighbour_count = 0;
            let mut average_neighbour_pos = Position::default();
            for (j, neighbour) in positions.iter().enumerate() {
                if i != j {
                    let dist = position.dist(neighbour);
                    min_distance = dist.min(min_distance);
                    if dist < spread_dist {
                        neighbour_count += 1;
                        average_neighbour_pos.x += neighbour.x - position.x;
                        average_neighbour_pos.z += neighbour.z - position.z;
                    }
                }
            }
            let position = &mut positions[i];
            if neighbour_count > 0 {
                average_neighbour_pos.x /= neighbour_count as f64;
                average_neighbour_pos.z /= neighbour_count as f64;
                let length = average_neighbour_pos.get_length();
                if length > 0.0 {
                    average_neighbour_pos.normalize();
                    position.move_away(&average_neighbour_pos);
                } else {
                    position.randomize(random, min_x, min_z, max_x, max_z);
                }
                has_collisions = true;
            }
            if position.clamp(min_x, min_z, max_x, max_z) {
                has_collisions = true;
            }
        }
        if !has_collisions {
            for position in positions.iter_mut() {
                if !position.is_safe(level, max_height) {
                    position.randomize(random, min_x, min_z, max_x, max_z);
                    has_collisions = true;
                }
            }
        }
        iteration += 1;
    }
    if min_distance == f32::MAX as f64 {
        min_distance = 0.0;
    }
    if iteration >= MAX_ITERATION_COUNT {
        let error = if respect_teams { &ERROR_FAILED_TO_SPREAD_TEAMS } else { &ERROR_FAILED_TO_SPREAD_ENTITIES };
        return Err(error.create(positions.len() as i32, center.0, center.1, java_format_fixed(min_distance, 2)));
    }
    Ok(())
}

fn set_player_positions(server: &mut Server, entities: &[EntityId], level: usize, positions: &[Position], max_height: i32, respect_teams: bool) -> f64 {
    let mut avg_distance = 0.0;
    let mut position_index = 0;
    let mut team_positions: Vec<(Option<String>, usize)> = Vec::new();
    for &entity in entities {
        let index = if respect_teams {
            let team = team_of(server, entity);
            match team_positions.iter().find(|(t, _)| *t == team) {
                Some(&(_, index)) => index,
                None => {
                    team_positions.push((team, position_index));
                    position_index += 1;
                    position_index - 1
                }
            }
        } else {
            position_index += 1;
            position_index - 1
        };
        let position = positions[index];
        let spawn_y = position.get_spawn_y(&mut server.levels[level], max_height);
        let e = server.entity_level(entity).entity(entity);
        let (y_rot, x_rot) = (e.y_rot, e.x_rot);
        let pos = Vec3::new(mth::floor(position.x) as f64 + 0.5, spawn_y as f64, mth::floor(position.z) as f64 + 0.5);
        server.teleport_to(entity, level, pos, 0, y_rot, x_rot, true);
        let mut closest = f64::MAX;
        for (test_index, test_position) in positions.iter().enumerate() {
            if index != test_index {
                let dist = position.dist(test_position);
                closest = dist.min(closest);
            }
        }
        avg_distance += closest;
    }
    if entities.len() < 2 { 0.0 } else { avg_distance / entities.len() as f64 }
}

fn create_initial_positions(random: &mut impl RandomSource, count: usize, min_x: f64, min_z: f64, max_x: f64, max_z: f64) -> Vec<Position> {
    (0..count)
        .map(|_| {
            let mut position = Position::default();
            position.randomize(random, min_x, min_z, max_x, max_z);
            position
        })
        .collect()
}

#[derive(Clone, Copy, Default, Debug)]
struct Position {
    x: f64,
    z: f64,
}

impl Position {
    fn dist(&self, target: &Position) -> f64 {
        let dx = self.x - target.x;
        let dz = self.z - target.z;
        (dx * dx + dz * dz).sqrt()
    }

    fn normalize(&mut self) {
        let dist = self.get_length();
        self.x /= dist;
        self.z /= dist;
    }

    fn get_length(&self) -> f64 {
        (self.x * self.x + self.z * self.z).sqrt()
    }

    fn move_away(&mut self, pos: &Position) {
        self.x -= pos.x;
        self.z -= pos.z;
    }

    fn clamp(&mut self, min_x: f64, min_z: f64, max_x: f64, max_z: f64) -> bool {
        let mut changed = false;
        if self.x < min_x {
            self.x = min_x;
            changed = true;
        } else if self.x > max_x {
            self.x = max_x;
            changed = true;
        }
        if self.z < min_z {
            self.z = min_z;
            changed = true;
        } else if self.z > max_z {
            self.z = max_z;
            changed = true;
        }
        changed
    }

    fn get_spawn_y(&self, level: &mut impl BlockGetter, max_height: i32) -> i32 {
        let mut pos = BlockPos::new(mth::floor(self.x), max_height + 1, mth::floor(self.z));
        let mut air2_above = level.block_state(pos).is_air();
        pos = pos.below();
        let mut air1_above = level.block_state(pos).is_air();
        while pos.y > level.get_min_y() {
            pos = pos.below();
            let current_is_air = level.block_state(pos).is_air();
            if !current_is_air && air1_above && air2_above {
                return pos.y + 1;
            }
            air2_above = air1_above;
            air1_above = current_is_air;
        }
        max_height + 1
    }

    fn is_safe(&self, level: &mut impl BlockGetter, max_height: i32) -> bool {
        let pos = BlockPos::containing(self.x, (self.get_spawn_y(level, max_height) - 1) as f64, self.z);
        let state = level.block_state(pos);
        pos.y < max_height && !state.liquid() && state.is(&crisp_data::block_tags::ENTITIES_CAN_TELEPORT_TO)
    }

    fn randomize(&mut self, random: &mut impl RandomSource, min_x: f64, min_z: f64, max_x: f64, max_z: f64) {
        self.x = mth::next_double(random, min_x, max_x);
        self.z = mth::next_double(random, min_z, max_z);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_core::random::{LegacyRandomSource, RandomSource};
    use crisp_data::{BlockState, blocks};

    use super::{create_initial_positions, spread_positions};
    use crate::world::level::BlockGetter;

    const ORACLE: &str = "case 0 4617447883048905792,-4614845817018670584,-60 -4605667291721991794,-4609781763917139174,-60 4621737556379926100,-4601903267591946544,-60 4622100592565682176,4621517118838805295,-60 4620955139650035295,4621276445601158813,-60 next 7326573195622447256
case 1 4624876802252346101,-4620472500106623582,-60 4614456217551176723,-4602453539604007468,-60 4623416028786544232,4616752568008179712,-60 4621070032545779513,-4602705278200981317,-60 4621228514785235110,4605145514064664888,-60 4625900504751276032,-4605916988085389358,-60 4623768119020576322,-4605643837753665344,-60 4616801948045815942,-4610339304054868583,-60 next 3869528714570848026
case 2 error Could not spread 4 entity/entities around 0.5, 0.5 (too many entities for space - try using spread of at most 0.00) next 1535132644386981093
case 3 4608406393937506267,4608514418825845374,-60 4607773943894508383,4608236627460884656,-60 4607340535612618018,4580616557859317312,-60 next 2856717244477545721
case 4 4608030033864530938,4613922005372732535,-60 4613584373212078710,4619409744661941625,-60 next 5061094476259430837
case 5 4618739792228905218,4612811918334230528,-60 4612811918334230528,4612811918334230528,-60 4613891064000249014,4615243023158513148,-60 4612811918334230528,4619004367821864960,-60 4612811918334230528,4616924127716306339,-60 4616542881362161593,4612811918334230528,-60 next 2697257049493821831";

    struct Terrain;

    impl BlockGetter for Terrain {
        fn block_state(&mut self, pos: BlockPos) -> BlockState {
            let (x, y, z) = (pos.x, pos.y, pos.z);
            if !(-64..=319).contains(&y) {
                return blocks::VOID_AIR.default_state();
            }
            if (-1..=0).contains(&x) && (-1..=0).contains(&z) && (-60..=-50).contains(&y) {
                return blocks::STONE.default_state();
            }
            match y {
                -64 => blocks::BEDROCK.default_state(),
                ..-61 => blocks::DIRT.default_state(),
                -61 if (3..=6).contains(&x) && (3..=6).contains(&z) => blocks::WATER.default_state(),
                -61 => blocks::GRASS_BLOCK.default_state(),
                _ => blocks::AIR.default_state(),
            }
        }

        fn get_min_y(&self) -> i32 {
            -64
        }
    }

    #[test]
    fn spread_positions_match_vanilla() {
        let cases: [(i64, f32, f32, f32, f32, i32, usize); 6] = [
            (1, 0.5, 0.5, 2.0, 10.0, 320, 5),
            (42, 10.5, -3.5, 5.0, 8.0, 320, 8),
            (7, 0.5, 0.5, 50.0, 5.0, 320, 4),
            (99, 0.5, 0.5, 0.0, 1.0, -55, 3),
            (123456789, 3.5, 4.5, 3.0, 3.0, 320, 2),
            (5, 4.5, 4.5, 1.0, 2.0, 320, 6),
        ];
        for (i, (expected, (seed, x, z, spread_distance, max_range, max_height, count))) in ORACLE.lines().zip(cases).enumerate() {
            let mut random = LegacyRandomSource::new(seed);
            let (min_x, min_z, max_x, max_z) = ((x - max_range) as f64, (z - max_range) as f64, (x + max_range) as f64, (z + max_range) as f64);
            let mut positions = create_initial_positions(&mut random, count, min_x, min_z, max_x, max_z);
            let mut line = format!("case {i}");
            match spread_positions((x, z), spread_distance as f64, &mut Terrain, &mut random, min_x, min_z, max_x, max_z, max_height, &mut positions, false) {
                Ok(()) => {
                    for position in &positions {
                        let spawn_y = position.get_spawn_y(&mut Terrain, max_height);
                        line.push_str(&format!(" {},{},{spawn_y}", position.x.to_bits() as i64, position.z.to_bits() as i64));
                    }
                }
                Err(e) => line.push_str(&format!(" error {}", e.message.get_string())),
            }
            line.push_str(&format!(" next {}", random.next_long()));
            assert_eq!(line, expected);
        }
    }
}
