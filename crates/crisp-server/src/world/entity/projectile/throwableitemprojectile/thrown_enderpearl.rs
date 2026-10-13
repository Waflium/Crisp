use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::{BlockState, blocks, damage_types, game_rule};

use super::throwable_item_projectile;
use crate::server::level::ticket::TicketType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::relative::relative;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:ender_pearl";
pub const ENDER_PEARL_TICKET_RADIUS: i32 = 2;

#[derive(Default)]
pub struct ThrownEnderpearl {
    ticket_timer: i64,
}

impl ThrownEnderpearl {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::ThrownEnderpearl(ThrownEnderpearl::default()))
    }

    pub fn from_owner(level: &mut ServerLevel, mob: EntityId, item_stack: &ItemStack) -> Projectile {
        let projectile = throwable_item_projectile::from_owner(level, ENTITY_TYPE, mob, item_stack, ProjectileKind::ThrownEnderpearl(ThrownEnderpearl::default()));
        if level.is_player(mob) {
            level.register_ender_pearl(mob, projectile.entity.uuid);
        }
        projectile
    }
}

fn thrown_enderpearl(projectile: &mut Projectile) -> &mut ThrownEnderpearl {
    match &mut projectile.kind {
        ProjectileKind::ThrownEnderpearl(pearl) => pearl,
        _ => unreachable!("not an ender pearl"),
    }
}

impl ServerLevel {
    pub fn thrown_enderpearl_deregister_from_current_owner(&mut self, id: EntityId) {
        if let Some(owner) = self.projectile_get_owner(id).filter(|&owner| self.is_player(owner)) {
            let uuid = self.entity(id).uuid;
            self.deregister_ender_pearl(owner, uuid);
        }
    }

    pub fn thrown_enderpearl_register_to_current_owner(&mut self, id: EntityId) {
        if let Some(owner) = self.projectile_get_owner(id).filter(|&owner| self.is_player(owner)) {
            let uuid = self.entity(id).uuid;
            self.register_ender_pearl(owner, uuid);
        }
    }

    pub fn register_ender_pearl(&mut self, player: EntityId, pearl: u128) {
        let pearls = &mut self.player_mut(player).ender_pearls;
        if !pearls.contains(&pearl) {
            pearls.push(pearl);
        }
    }

    pub fn deregister_ender_pearl(&mut self, player: EntityId, pearl: u128) {
        self.player_mut(player).ender_pearls.retain(|&p| p != pearl);
    }

    fn register_and_update_ender_pearl_ticket(&mut self, player: EntityId, pearl: EntityId) -> i64 {
        let chunk_pos = self.entity(pearl).chunk_position();
        let uuid = self.entity(pearl).uuid;
        self.register_ender_pearl(player, uuid);
        self.reset_empty_time();
        self.place_ender_pearl_ticket(chunk_pos) - 1
    }

    pub fn place_ender_pearl_ticket(&mut self, chunk: crisp_core::ChunkPos) -> i64 {
        self.chunks.add_ticket_with_radius(TicketType::EnderPearl, chunk, ENDER_PEARL_TICKET_RADIUS);
        TicketType::EnderPearl.timeout()
    }

    fn find_owner_including_dead_player(&self, uuid: u128) -> Option<EntityId> {
        self.entity_manager.visible_entity_storage.get(uuid).or_else(|| self.players.values().find(|p| p.profile.uuid == uuid).map(|p| p.entity.id))
    }

    pub fn thrown_enderpearl_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let random = &mut self.entity_mut(id).random;
        for _ in 0..32 {
            random.next_double();
            random.next_gaussian();
            random.next_gaussian();
        }
        if self.entity(id).is_removed() {
            return;
        }
        let owner = self.projectile_get_owner(id);
        match owner.filter(|&owner| self.is_allowed_to_teleport_owner(owner)) {
            Some(owner) => {
                let teleport_pos = self.entity(id).old_position();
                if self.is_player(owner) {
                    if !self.player(owner).disconnected {
                        self.entity_mut(id).random.next_float();
                        if self.is_on_portal_cooldown(id) {
                            self.set_portal_cooldown(owner);
                        }
                        let transition = TeleportTransition::with_relatives(self.index, teleport_pos, Vec3::ZERO, 0.0, 0.0, relative::ROTATION, PostTeleportTransition::DO_NOTHING);
                        self.server_player_teleport_same_dimension(owner, transition);
                        self.reset_fall_distance(owner);
                        self.living_mut(owner).reset_current_impulse_context();
                        self.hurt_server(owner, DamageSource::new(damage_types::ENDER_PEARL), 5.0);
                        self.thrown_enderpearl_play_sound(teleport_pos);
                    }
                } else {
                    let (y_rot, x_rot) = (self.entity(owner).y_rot, self.entity(owner).x_rot);
                    let transition = TeleportTransition::new(self.index, teleport_pos, Vec3::ZERO, y_rot, x_rot, PostTeleportTransition::DO_NOTHING);
                    self.teleport_same_dimension(owner, transition);
                    self.reset_fall_distance(owner);
                    if self.is_living(owner) {
                        self.living_mut(owner).reset_current_impulse_context();
                    }
                    self.thrown_enderpearl_play_sound(teleport_pos);
                }
                self.entity_mut(id).discard();
            }
            None => self.entity_mut(id).discard(),
        }
    }

    fn is_allowed_to_teleport_owner(&self, owner: EntityId) -> bool {
        if self.is_living(owner) { self.is_alive(owner) && !self.is_sleeping(owner) } else { self.is_alive(owner) }
    }

    pub fn thrown_enderpearl_tick(&mut self, id: EntityId) {
        let position = self.entity(id).position();
        let (chunk_x, chunk_z) = (mth::floor(position.x) >> 4, mth::floor(position.z) >> 4);
        let owner = self.projectile(id).owner.and_then(|owner| self.find_owner_including_dead_player(owner.get_uuid()));
        let owner_player = owner.filter(|&owner| self.is_player(owner));
        if let Some(player) = owner_player
            && !self.is_alive(player)
            && !self.player(player).won_game
            && self.game_rules.get_bool(game_rule::ENDER_PEARLS_VANISH_ON_DEATH)
        {
            self.entity_mut(id).discard();
        } else {
            self.throwable_projectile_tick(id);
        }
        if self.is_alive(id) {
            let current_pos = BlockPos::containing_vec(self.entity(id).position());
            let pearl = thrown_enderpearl(self.projectile_mut(id));
            pearl.ticket_timer -= 1;
            let expired = pearl.ticket_timer <= 0;
            if (expired || chunk_x != current_pos.x >> 4 || chunk_z != current_pos.z >> 4)
                && let Some(player) = owner_player
            {
                let ticket_timer = self.register_and_update_ender_pearl_ticket(player, id);
                thrown_enderpearl(self.projectile_mut(id)).ticket_timer = ticket_timer;
            }
        }
    }

    fn thrown_enderpearl_play_sound(&mut self, position: Vec3) {
        self.play_sound_at(None, position.x, position.y, position.z, sound_event("minecraft:entity.player.teleport"), SoundSource::Players, 1.0, 1.0);
    }

    pub fn thrown_enderpearl_on_inside_block(&mut self, id: EntityId, state: BlockState) {
        if state.is_block(blocks::END_GATEWAY)
            && let Some(player) = self.projectile_get_owner(id).filter(|&owner| self.is_player(owner))
        {
            crate::advancements::triggers::criteria_triggers::ENTER_BLOCK.trigger(self, player, state);
        }
    }

    pub fn thrown_enderpearl_on_removal(&mut self, id: EntityId, reason: Option<crate::world::entity::RemovalReason>) {
        if reason != Some(crate::world::entity::RemovalReason::UnloadedWithPlayer) {
            self.thrown_enderpearl_deregister_from_current_owner(id);
        }
    }
}
