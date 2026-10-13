use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::{entity_type_tags, game_rule, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::arrow::abstract_arrow::AbstractArrow;
use super::arrow::arrow::Arrow;
use super::arrow::spectral_arrow::SpectralArrow;
use super::firework_rocket_entity::FireworkRocketEntity;
use super::projectile_deflection::ProjectileDeflection;
use super::throwableitemprojectile::snowball::Snowball;
use super::throwableitemprojectile::thrown_egg::ThrownEgg;
use super::throwableitemprojectile::thrown_enderpearl::ThrownEnderpearl;
use super::throwableitemprojectile::thrown_experience_bottle::ThrownExperienceBottle;
use super::throwableitemprojectile::thrown_lingering_potion::ThrownLingeringPotion;
use super::throwableitemprojectile::thrown_splash_potion::ThrownSplashPotion;
use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::DataValue;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::level::block::{self, BlockHitResult};
use crate::world::level::BlockGetter;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::{HitResult, Type};

pub enum ProjectileKind {
    Snowball(Snowball),
    ThrownEgg(ThrownEgg),
    ThrownEnderpearl(ThrownEnderpearl),
    ThrownExperienceBottle(ThrownExperienceBottle),
    ThrownSplashPotion(ThrownSplashPotion),
    ThrownLingeringPotion(ThrownLingeringPotion),
    Arrow(Arrow),
    SpectralArrow(SpectralArrow),
    FireworkRocket(FireworkRocketEntity),
}

pub struct Projectile {
    pub entity: Entity,
    pub owner: Option<EntityReference>,
    pub can_break: Option<Nbt>,
    pub left_owner: bool,
    left_owner_checked: bool,
    pub has_been_shot: bool,
    last_deflected_by: Option<EntityId>,
    pub kind: ProjectileKind,
}

impl Projectile {
    pub fn new(entity_type: &str, dimensions: EntityDimensions, data: Vec<DataValue>, kind: ProjectileKind) -> Projectile {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), dimensions, data);
        Projectile { entity, owner: None, can_break: None, left_owner: false, left_owner_checked: false, has_been_shot: false, last_deflected_by: None, kind }
    }

    pub fn abstract_arrow(&self) -> Option<&AbstractArrow> {
        match &self.kind {
            ProjectileKind::Arrow(arrow) => Some(&arrow.abstract_arrow),
            ProjectileKind::SpectralArrow(arrow) => Some(&arrow.abstract_arrow),
            _ => None,
        }
    }

    pub fn abstract_arrow_mut(&mut self) -> Option<&mut AbstractArrow> {
        match &mut self.kind {
            ProjectileKind::Arrow(arrow) => Some(&mut arrow.abstract_arrow),
            ProjectileKind::SpectralArrow(arrow) => Some(&mut arrow.abstract_arrow),
            _ => None,
        }
    }

    pub fn is_throwable_item_projectile(&self) -> bool {
        matches!(
            self.kind,
            ProjectileKind::Snowball(_)
                | ProjectileKind::ThrownEgg(_)
                | ProjectileKind::ThrownEnderpearl(_)
                | ProjectileKind::ThrownExperienceBottle(_)
                | ProjectileKind::ThrownSplashPotion(_)
                | ProjectileKind::ThrownLingeringPotion(_)
        )
    }

    pub fn apply_implicit_components(&mut self, components: &ItemStack) {
        if let Some(value) = components.get(dc::CAN_BREAK) {
            self.can_break = Some(value.clone());
        }
        self.entity.apply_components_from_item_stack(components);
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        EntityReference::store(self.owner, output, "Owner");
        if self.left_owner {
            output.push(("LeftOwner".into(), Nbt::Byte(1)));
        }
        output.push(("HasBeenShot".into(), Nbt::Byte(self.has_been_shot as i8)));
        if let Some(can_break) = &self.can_break {
            output.push(("can_break".into(), can_break.clone()));
        }
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        self.owner = EntityReference::read(input, "Owner");
        self.left_owner = input.get("LeftOwner").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        self.has_been_shot = input.get("HasBeenShot").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        self.can_break = input.get("can_break").filter(|v| dc::CAN_BREAK.decode(v).is_ok()).cloned();
    }

    pub fn get_movement_to_shoot(&mut self, xd: f64, yd: f64, zd: f64, pow: f32, uncertainty: f32) -> Vec3 {
        let random = &mut self.entity.random;
        let spread = 0.0172275 * uncertainty as f64;
        Vec3::new(xd, yd, zd)
            .normalize()
            .add(random.triangle(0.0, spread), random.triangle(0.0, spread), random.triangle(0.0, spread))
            .scale(pow as f64)
    }

    pub fn shoot(&mut self, xd: f64, yd: f64, zd: f64, pow: f32, uncertainty: f32) {
        let movement = self.get_movement_to_shoot(xd, yd, zd, pow, uncertainty);
        let e = &mut self.entity;
        e.set_delta_movement(movement);
        e.needs_sync = true;
        let sd = movement.horizontal_distance();
        e.set_y_rot((mth::atan2(movement.x, movement.z) * mth::RAD_TO_DEG as f64) as f32);
        e.set_x_rot((mth::atan2(movement.y, sd) * mth::RAD_TO_DEG as f64) as f32);
        e.y_rot_o = e.y_rot;
        e.x_rot_o = e.x_rot;
        if let Some(arrow) = self.abstract_arrow_mut() {
            arrow.life = 0;
        }
    }

    pub fn shoot_from_rotation(&mut self, source_movement: Vec3, source_on_ground: bool, x_rot: f32, y_rot: f32, y_offset: f32, pow: f32, uncertainty: f32) {
        let rad = mth::DEG_TO_RAD;
        let xd = -mth::sin((y_rot * rad) as f64) * mth::cos((x_rot * rad) as f64);
        let yd = -mth::sin(((x_rot + y_offset) * rad) as f64);
        let zd = mth::cos((y_rot * rad) as f64) * mth::cos((x_rot * rad) as f64);
        self.shoot(xd as f64, yd as f64, zd as f64, pow, uncertainty);
        let e = &mut self.entity;
        e.set_delta_movement(e.delta_movement.add(source_movement.x, if source_on_ground { 0.0 } else { source_movement.y }, source_movement.z));
    }
}

pub fn lerp_rotation(mut rot_o: f32, rot: f32) -> f32 {
    while rot - rot_o < -180.0 {
        rot_o -= 360.0;
    }
    while rot - rot_o >= 180.0 {
        rot_o += 360.0;
    }
    mth::lerp_f(0.2, rot_o, rot)
}

impl ServerLevel {
    pub fn as_projectile(&self, id: EntityId) -> Option<&Projectile> {
        match self.entities.get(&id) {
            Some(LevelEntity::Projectile(projectile)) => Some(projectile),
            _ => None,
        }
    }

    pub fn projectile(&self, id: EntityId) -> &Projectile {
        self.as_projectile(id).expect("a projectile")
    }

    pub fn projectile_mut(&mut self, id: EntityId) -> &mut Projectile {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Projectile(projectile)) => projectile,
            _ => unreachable!("not a projectile"),
        }
    }

    pub fn is_projectile(&self, id: EntityId) -> bool {
        self.as_projectile(id).is_some()
    }

    pub fn projectile_get_owner(&self, id: EntityId) -> Option<EntityId> {
        self.projectile(id).owner.and_then(|owner| owner.get_entity(self))
    }

    pub fn projectile_get_effect_source(&self, id: EntityId) -> EntityId {
        self.projectile_get_owner(id).unwrap_or(id)
    }

    pub fn projectile_owned_by(&self, id: EntityId, entity: EntityId) -> bool {
        self.projectile(id).owner.is_some_and(|owner| owner.get_uuid() == self.entity(entity).uuid)
    }

    pub fn projectile_set_owner_reference(&mut self, id: EntityId, owner: Option<EntityReference>) {
        let is_pearl = matches!(self.projectile(id).kind, ProjectileKind::ThrownEnderpearl(_));
        if is_pearl {
            self.thrown_enderpearl_deregister_from_current_owner(id);
        }
        self.projectile_mut(id).owner = owner;
        if is_pearl {
            self.thrown_enderpearl_register_to_current_owner(id);
        }
    }

    pub fn projectile_add_data(&self, id: EntityId) -> i32 {
        self.projectile_get_owner(id).unwrap_or(0)
    }

    pub fn projectile_tick(&mut self, id: EntityId) {
        match self.projectile(id).kind {
            ProjectileKind::ThrownEnderpearl(_) => self.thrown_enderpearl_tick(id),
            ProjectileKind::Arrow(_) => self.arrow_tick(id),
            ProjectileKind::SpectralArrow(_) => self.abstract_arrow_tick(id),
            ProjectileKind::FireworkRocket(_) => self.firework_rocket_tick(id),
            _ => self.throwable_projectile_tick(id),
        }
    }

    pub fn projectile_base_tick(&mut self, id: EntityId) {
        let projectile = self.projectile_mut(id);
        if !projectile.has_been_shot {
            projectile.has_been_shot = true;
        }
        self.check_left_owner(id);
        self.entity_tick(id);
        if self.entities.contains_key(&id) {
            self.projectile_mut(id).left_owner_checked = false;
        }
    }

    pub fn check_left_owner(&mut self, id: EntityId) {
        let projectile = self.projectile(id);
        if !projectile.left_owner && !projectile.left_owner_checked {
            let left_owner = self.is_outside_owner_collision_range(id);
            let projectile = self.projectile_mut(id);
            projectile.left_owner = left_owner;
            projectile.left_owner_checked = true;
        }
    }

    fn is_outside_owner_collision_range(&self, id: EntityId) -> bool {
        let Some(owner) = self.projectile_get_owner(id) else { return true };
        let e = self.entity(id);
        let aabb = e.bounding_box().expand_towards_vec(e.delta_movement).inflate_all(1.0);
        let root = self.root_vehicle(owner);
        !self.self_and_passengers(root).into_iter().filter(|&entity| self.can_be_picked(entity)).any(|entity| aabb.intersects(&self.entity(entity).bounding_box()))
    }

    pub fn can_break_block_in_adventure_mode(&mut self, id: EntityId, pos: BlockPos) -> bool {
        let owner = self.projectile_get_owner(id);
        let adventure_owner = match owner.and_then(|owner| self.players.get(&owner)) {
            Some(player) => player.game_mode.game_type == GameType::Adventure,
            None => owner.is_some(),
        };
        if !adventure_owner {
            return true;
        }
        let state = self.block_state(pos);
        self.projectile(id).can_break.as_ref().is_some_and(|predicates| crate::world::item::adventure_mode_predicate_test(predicates, state))
    }

    pub fn spawn_projectile(&mut self, mut projectile: Projectile, item_stack: &ItemStack, shoot: impl FnOnce(&mut Projectile)) -> EntityId {
        shoot(&mut projectile);
        let id = projectile.entity.id;
        self.add_fresh_entity(LevelEntity::Projectile(Box::new(projectile)));
        self.apply_on_projectile_spawned(id, item_stack);
        id
    }

    pub fn spawn_projectile_from_rotation(&mut self, projectile: Projectile, item_stack: &ItemStack, source: EntityId, y_offset: f32, pow: f32, uncertainty: f32) -> EntityId {
        let (movement, on_ground) = (self.known_movement(source), self.entity(source).on_ground);
        let (x_rot, y_rot) = (self.entity(source).x_rot, self.entity(source).y_rot);
        self.spawn_projectile(projectile, item_stack, |projectile| projectile.shoot_from_rotation(movement, on_ground, x_rot, y_rot, y_offset, pow, uncertainty))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_projectile_using_shoot(&mut self, projectile: Projectile, item_stack: &ItemStack, target_x: f64, target_y: f64, target_z: f64, pow: f32, uncertainty: f32) -> EntityId {
        self.spawn_projectile(projectile, item_stack, |projectile| projectile.shoot(target_x, target_y, target_z, pow, uncertainty))
    }

    pub fn apply_on_projectile_spawned(&mut self, id: EntityId, pickup_item_stack: &ItemStack) {
        self.on_projectile_spawned(pickup_item_stack, id);
        let weapon = self.projectile(id).abstract_arrow().and_then(|arrow| arrow.fired_from_weapon.clone());
        if let Some(weapon) = weapon
            && !weapon.is_empty()
            && pickup_item_stack.get_item() != weapon.get_item()
        {
            self.on_projectile_spawned(&weapon, id);
        }
    }

    pub fn hit_target_or_deflect_self(&mut self, id: EntityId, hit_result: HitResult) -> ProjectileDeflection {
        match hit_result {
            HitResult::Entity(entity_hit_result) => {
                let entity = entity_hit_result.entity;
                let deflection = self.entity_deflection(entity, id);
                if deflection != ProjectileDeflection::None {
                    let owner = self.projectile(id).owner;
                    if Some(entity) != self.projectile(id).last_deflected_by && self.deflect(id, deflection, Some(entity), owner, false, Vec3::new(1.0, 1.0, 1.0)) {
                        self.projectile_mut(id).last_deflected_by = Some(entity);
                    }
                    return deflection;
                }
            }
            HitResult::Block { hit, .. } => {
                if !hit.world_border_hit {
                    if self.block_state(hit.block_pos).is_air() {
                        return ProjectileDeflection::None;
                    }
                } else if self.should_bounce_on_world_border(id) {
                    let owner = self.projectile(id).owner;
                    if self.deflect(id, ProjectileDeflection::Reverse, None, owner, false, Vec3::new(0.2, 0.2, 0.2)) {
                        return ProjectileDeflection::Reverse;
                    }
                }
            }
        }
        self.projectile_on_hit(id, hit_result);
        ProjectileDeflection::None
    }

    fn should_bounce_on_world_border(&self, id: EntityId) -> bool {
        self.projectile(id).abstract_arrow().is_some()
    }

    pub fn deflect(&mut self, id: EntityId, deflection: ProjectileDeflection, deflecting_entity: Option<EntityId>, new_owner: Option<EntityReference>, _by_attack: bool, power: Vec3) -> bool {
        deflection.deflect(self, id, deflecting_entity, power);
        self.projectile_set_owner_reference(id, new_owner);
        true
    }

    pub fn entity_deflection(&self, entity: EntityId, _projectile: EntityId) -> ProjectileDeflection {
        if entity_type_tags::DEFLECTS_PROJECTILES.contains(self.entity(entity).entity_type as usize) { ProjectileDeflection::Reverse } else { ProjectileDeflection::None }
    }

    pub fn projectile_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        match self.projectile(id).kind {
            ProjectileKind::Snowball(_) => self.snowball_on_hit(id, hit_result),
            ProjectileKind::ThrownEgg(_) => self.thrown_egg_on_hit(id, hit_result),
            ProjectileKind::ThrownEnderpearl(_) => self.thrown_enderpearl_on_hit(id, hit_result),
            ProjectileKind::ThrownExperienceBottle(_) => self.thrown_experience_bottle_on_hit(id, hit_result),
            ProjectileKind::ThrownSplashPotion(_) | ProjectileKind::ThrownLingeringPotion(_) => self.abstract_thrown_potion_on_hit(id, hit_result),
            ProjectileKind::Arrow(_) | ProjectileKind::SpectralArrow(_) | ProjectileKind::FireworkRocket(_) => self.projectile_base_on_hit(id, hit_result),
        }
    }

    pub fn projectile_base_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        match hit_result.get_type() {
            Type::Entity => {
                let HitResult::Entity(entity_hit_result) = hit_result else { unreachable!() };
                let entity_hit = entity_hit_result.entity;
                if entity_type_tags::REDIRECTABLE_PROJECTILE.contains(self.entity(entity_hit).entity_type as usize) && self.is_projectile(entity_hit) {
                    self.on_redirect_projectile(id, entity_hit);
                }
                self.on_hit_entity(id, entity_hit_result);
            }
            Type::Block => {
                let HitResult::Block { hit, .. } = hit_result else { unreachable!() };
                self.on_hit_block(id, hit);
            }
            Type::Miss => {}
        }
    }

    fn on_redirect_projectile(&mut self, id: EntityId, hit_projectile: EntityId) {
        let (owner, owner_reference) = (self.projectile_get_owner(id), self.projectile(id).owner);
        self.deflect(hit_projectile, ProjectileDeflection::AimDeflect, owner, owner_reference, true, Vec3::new(1.0, 1.0, 1.0));
    }

    pub fn on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        match self.projectile(id).kind {
            ProjectileKind::Snowball(_) => self.snowball_on_hit_entity(id, hit_result),
            ProjectileKind::ThrownEgg(_) | ProjectileKind::ThrownEnderpearl(_) => {
                let source = self.thrown(id);
                self.hurt(hit_result.entity, source, 0.0);
            }
            ProjectileKind::Arrow(_) | ProjectileKind::SpectralArrow(_) => self.abstract_arrow_on_hit_entity(id, hit_result),
            ProjectileKind::FireworkRocket(_) => self.firework_rocket_on_hit_entity(id),
            ProjectileKind::ThrownExperienceBottle(_) | ProjectileKind::ThrownSplashPotion(_) | ProjectileKind::ThrownLingeringPotion(_) => {}
        }
    }

    pub fn on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        match self.projectile(id).kind {
            ProjectileKind::Arrow(_) | ProjectileKind::SpectralArrow(_) => self.abstract_arrow_on_hit_block(id, hit_result),
            ProjectileKind::FireworkRocket(_) => self.firework_rocket_on_hit_block(id, hit_result),
            ProjectileKind::ThrownSplashPotion(_) | ProjectileKind::ThrownLingeringPotion(_) => self.abstract_thrown_potion_on_hit_block(id, hit_result),
            _ => self.projectile_base_on_hit_block(id, hit_result),
        }
    }

    pub fn projectile_base_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        let state = self.block_state(hit_result.block_pos);
        block::of(state).on_projectile_hit(self, state, &hit_result, id);
    }

    pub fn can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        if self.projectile(id).abstract_arrow().is_some() {
            return self.abstract_arrow_can_hit_entity(id, entity);
        }
        self.projectile_base_can_hit_entity(id, entity)
    }

    pub fn projectile_base_can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        if !self.can_be_hit_by_projectile(entity) {
            return false;
        }
        let owner = self.projectile_get_owner(id);
        owner.is_none() || self.projectile(id).left_owner || !self.is_passenger_of_same_vehicle(owner.expect("an owner"), entity)
    }

    pub fn update_rotation(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        let sd = movement.horizontal_distance();
        let x_rot = lerp_rotation(e.x_rot_o, (mth::atan2(movement.y, sd) * mth::RAD_TO_DEG as f64) as f32);
        e.set_x_rot(x_rot);
        let y_rot = lerp_rotation(e.y_rot_o, (mth::atan2(movement.x, movement.z) * mth::RAD_TO_DEG as f64) as f32);
        e.set_y_rot(y_rot);
    }

    pub fn projectile_may_interact(&self, id: EntityId, pos: BlockPos) -> bool {
        match self.projectile_get_owner(id) {
            Some(owner) if self.is_player(owner) => self.may_interact(owner, pos),
            owner => owner.is_none() || self.game_rules.get_bool(game_rule::MOB_GRIEFING),
        }
    }

    pub fn may_break(&mut self, id: EntityId, pos: BlockPos) -> bool {
        entity_type_tags::IMPACT_PROJECTILES.contains(self.entity(id).entity_type as usize)
            && self.game_rules.get_bool(game_rule::PROJECTILES_CAN_BREAK_BLOCKS)
            && self.can_break_block_in_adventure_mode(id, pos)
    }

    pub fn calculate_horizontal_hurt_knockback_direction(&self, id: EntityId, hurt_entity: EntityId) -> (f64, f64) {
        match self.projectile(id).kind {
            ProjectileKind::ThrownSplashPotion(_) | ProjectileKind::ThrownLingeringPotion(_) | ProjectileKind::FireworkRocket(_) => {
                let (hurt, this) = (self.entity(hurt_entity).position(), self.entity(id).position());
                (hurt.x - this.x, hurt.z - this.z)
            }
            _ => {
                let movement = self.entity(id).delta_movement;
                (movement.x, movement.z)
            }
        }
    }

    pub fn projectile_hurt_server(&mut self, id: EntityId, source: &DamageSource) -> bool {
        if !self.is_invulnerable_to_base(id, source) {
            self.entity_mut(id).sync_velocity = true;
        }
        false
    }

    pub fn thrown(&self, id: EntityId) -> DamageSource {
        DamageSource::source(crisp_data::damage_types::THROWN, Some(id), self.projectile_get_owner(id))
    }

    fn on_projectile_spawned(&mut self, weapon: &ItemStack, projectile: EntityId) {
        let owner = self.projectile_get_owner(projectile).filter(|&owner| self.is_living(owner));
        enchantment_helper::on_projectile_spawned(self, weapon, projectile, owner);
    }
}
