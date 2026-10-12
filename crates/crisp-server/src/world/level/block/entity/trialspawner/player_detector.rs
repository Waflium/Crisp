use crisp_core::BlockPos;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PlayerDetector {
    NoCreativePlayers,
    #[default]
    IncludingCreativePlayers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EntitySelector {
    #[default]
    SelectFromLevel,
}

impl EntitySelector {
    pub fn get_players(self, level: &ServerLevel, selector: impl Fn(&ServerLevel, EntityId) -> bool) -> Vec<EntityId> {
        match self {
            EntitySelector::SelectFromLevel => level.players.keys().copied().filter(|&player| selector(level, player)).collect(),
        }
    }
}

impl PlayerDetector {
    pub fn detect(self, level: &ServerLevel, selector: EntitySelector, spawner_pos: BlockPos, required_player_range: f64) -> Vec<u128> {
        let players = selector.get_players(level, |level, p| {
            let player = level.player(p);
            player.entity.block_position().closer_than(spawner_pos, required_player_range)
                && (self == PlayerDetector::IncludingCreativePlayers || !player.game_mode.game_type.is_creative())
                && !level.is_spectator(p)
        });
        players.into_iter().map(|player| level.player(player).entity.uuid).collect()
    }
}
