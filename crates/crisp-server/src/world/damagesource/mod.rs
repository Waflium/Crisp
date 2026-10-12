pub mod combat_rules;
pub mod combat_tracker;

use crisp_core::Vec3;
use crisp_data::{DamageScaling, DamageType, Tag};

use crate::network::chat::Component;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::scores::player_team::PlayerTeam;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DamageSource {
    pub ty: DamageType,
    pub causing_entity: Option<EntityId>,
    pub direct_entity: Option<EntityId>,
    pub damage_source_position: Option<Vec3>,
}

impl DamageSource {
    pub fn new(ty: DamageType) -> DamageSource {
        DamageSource { ty, causing_entity: None, direct_entity: None, damage_source_position: None }
    }

    pub fn entity(ty: DamageType, entity: EntityId) -> DamageSource {
        DamageSource { ty, causing_entity: Some(entity), direct_entity: Some(entity), damage_source_position: None }
    }

    pub fn food_exhaustion(&self) -> f32 {
        self.ty.info().exhaustion
    }

    pub fn is(&self, tag: &Tag) -> bool {
        self.ty.is(tag)
    }

    pub fn is_type(&self, ty: DamageType) -> bool {
        self.ty == ty
    }

    pub fn is_direct(&self) -> bool {
        self.causing_entity == self.direct_entity
    }

    pub fn msg_id(&self) -> &'static str {
        self.ty.info().msg_id
    }

    pub fn scales_with_difficulty(&self, level: &ServerLevel) -> bool {
        match self.ty.info().scaling {
            DamageScaling::Never => false,
            DamageScaling::WhenCausedByLivingNonPlayer => self.causing_entity.is_some_and(|e| level.is_living(e) && !level.is_player(e)),
            DamageScaling::Always => true,
        }
    }

    pub fn is_creative_player(&self, level: &ServerLevel) -> bool {
        self.causing_entity.and_then(|e| level.players.get(&e)).is_some_and(|p| p.abilities.instabuild)
    }

    pub fn get_weapon_item(&self, level: &ServerLevel) -> Option<crate::world::item::ItemStack> {
        self.direct_entity.filter(|&e| level.is_living(e)).map(|e| level.get_weapon_item(e))
    }

    pub fn source_position(&self, level: &ServerLevel) -> Option<Vec3> {
        self.damage_source_position.or_else(|| self.direct_entity.and_then(|e| level.entity_ref(e)).map(|e| e.position()))
    }

    pub fn localized_death_message(&self, level: &ServerLevel, victim: EntityId) -> Component {
        let death_msg = format!("death.attack.{}", self.msg_id());
        if self.causing_entity.is_none() && self.direct_entity.is_none() {
            match level.kill_credit(victim) {
                Some(source) => Component::translatable_with(&format!("{death_msg}.player"), vec![level.display_name(victim).into(), level.display_name(source).into()]),
                None => Component::translatable_with(&death_msg, vec![level.display_name(victim).into()]),
            }
        } else {
            let name = level.display_name(self.causing_entity.or(self.direct_entity).expect("an entity"));
            Component::translatable_with(&death_msg, vec![level.display_name(victim).into(), name.into()])
        }
    }
}

impl ServerLevel {
    pub fn is_living(&self, id: EntityId) -> bool {
        self.is_player(id) || self.is_mob(id)
    }

    pub fn display_name(&self, id: EntityId) -> Component {
        match self.players.get(&id) {
            Some(p) => self.player_display_name(&p.profile.name, p.profile.uuid),
            None => self.entities.get(&id).map_or_else(Component::empty, |e| e.display_name(self.team_of(id))),
        }
    }

    pub fn player_display_name(&self, name: &str, uuid: u128) -> Component {
        let team = if self.scoreboard.team_count() == 0 { None } else { self.scoreboard.get_players_team(name) };
        let formatted = PlayerTeam::format_name_for_team(team, Component::literal(name));
        crate::network::chat::decorate_display_name(formatted, name, uuid)
    }

    pub fn kill_credit(&self, victim: EntityId) -> Option<EntityId> {
        let living = self.living(victim);
        let lookup = |uuid: u128| self.entity_manager.visible_entity_storage.get(uuid);
        match living.last_hurt_by_player {
            Some(player) => lookup(player).filter(|&p| self.is_player(p)),
            None => living.last_hurt_by_mob.and_then(lookup).filter(|&e| self.is_living(e)),
        }
    }
}
