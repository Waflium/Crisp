use crisp_core::Vec3;
use crisp_protocol::nbt::Nbt;

use super::cube_mob_predicate::CubeMobPredicate;
use super::distance_to_player_predicate::DistanceToPlayerPredicate;
use super::entity_effects_predicate::EntityEffectsPredicate;
use super::entity_equipment_predicate::EntityEquipmentPredicate;
use super::entity_exact_data_components_predicate::EntityExactDataComponentsPredicate;
use super::entity_flags_predicate::EntityFlagsPredicate;
use super::entity_location_predicate::EntityLocationPredicate;
use super::entity_nbt_predicate::EntityNbtPredicate;
use super::entity_partial_components_predicate::EntityPartialComponentsPredicate;
use super::entity_slots_predicate::EntitySlotsPredicate;
use super::entity_tag_predicate::EntityTagPredicate;
use super::entity_type_predicate::EntityTypePredicate;
use super::fishing_hook_predicate::FishingHookPredicate;
use super::lightning_bolt_predicate::LightningBoltPredicate;
use super::movement_affected_by_predicate::MovementAffectedByPredicate;
use super::movement_predicate::MovementPredicate;
use super::passenger_predicate::PassengerPredicate;
use super::periodic_entity_tick_predicate::PeriodicEntityTickPredicate;
use super::player_predicate::PlayerPredicate;
use super::raider_predicate::RaiderPredicate;
use super::sheep_predicate::SheepPredicate;
use super::stepping_on_predicate::SteppingOnPredicate;
use super::targeted_entity_predicate::TargetedEntityPredicate;
use super::team_predicate::TeamPredicate;
use super::vehicle_predicate::VehiclePredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub enum EntitySubPredicate {
    EntityType(EntityTypePredicate),
    Location(EntityLocationPredicate),
    SteppingOn(SteppingOnPredicate),
    MovementAffectedBy(MovementAffectedByPredicate),
    Distance(DistanceToPlayerPredicate),
    Movement(MovementPredicate),
    Effects(EntityEffectsPredicate),
    Nbt(EntityNbtPredicate),
    Flags(EntityFlagsPredicate),
    Equipment(EntityEquipmentPredicate),
    PeriodicTick(PeriodicEntityTickPredicate),
    Vehicle(VehiclePredicate),
    Passenger(PassengerPredicate),
    TargetedEntity(TargetedEntityPredicate),
    Team(TeamPredicate),
    Slots(EntitySlotsPredicate),
    Components(EntityExactDataComponentsPredicate),
    Predicates(EntityPartialComponentsPredicate),
    EntityTags(EntityTagPredicate),
    Lightning(LightningBoltPredicate),
    FishingHook(FishingHookPredicate),
    Player(PlayerPredicate),
    CubeMob(CubeMobPredicate),
    Raider(RaiderPredicate),
    Sheep(SheepPredicate),
}

impl EntitySubPredicate {
    pub fn decode(id: &str, tag: &Nbt) -> DataResult<EntitySubPredicate> {
        Ok(match id {
            "minecraft:entity_type" => EntitySubPredicate::EntityType(EntityTypePredicate::decode(tag)?),
            "minecraft:location" => EntitySubPredicate::Location(EntityLocationPredicate::decode(tag)?),
            "minecraft:stepping_on" => EntitySubPredicate::SteppingOn(SteppingOnPredicate::decode(tag)?),
            "minecraft:movement_affected_by" => EntitySubPredicate::MovementAffectedBy(MovementAffectedByPredicate::decode(tag)?),
            "minecraft:distance" => EntitySubPredicate::Distance(DistanceToPlayerPredicate::decode(tag)?),
            "minecraft:movement" => EntitySubPredicate::Movement(MovementPredicate::decode(tag)?),
            "minecraft:effects" => EntitySubPredicate::Effects(EntityEffectsPredicate::decode(tag)?),
            "minecraft:nbt" => EntitySubPredicate::Nbt(EntityNbtPredicate::decode(tag)?),
            "minecraft:flags" => EntitySubPredicate::Flags(EntityFlagsPredicate::decode(tag)?),
            "minecraft:equipment" => EntitySubPredicate::Equipment(EntityEquipmentPredicate::decode(tag)?),
            "minecraft:periodic_tick" => EntitySubPredicate::PeriodicTick(PeriodicEntityTickPredicate::decode(tag)?),
            "minecraft:vehicle" => EntitySubPredicate::Vehicle(VehiclePredicate::decode(tag)?),
            "minecraft:passenger" => EntitySubPredicate::Passenger(PassengerPredicate::decode(tag)?),
            "minecraft:targeted_entity" => EntitySubPredicate::TargetedEntity(TargetedEntityPredicate::decode(tag)?),
            "minecraft:team" => EntitySubPredicate::Team(TeamPredicate::decode(tag)?),
            "minecraft:slots" => EntitySubPredicate::Slots(EntitySlotsPredicate::decode(tag)?),
            "minecraft:components" => EntitySubPredicate::Components(EntityExactDataComponentsPredicate::decode(tag)?),
            "minecraft:predicates" => EntitySubPredicate::Predicates(EntityPartialComponentsPredicate::decode(tag)?),
            "minecraft:entity_tags" => EntitySubPredicate::EntityTags(EntityTagPredicate::decode(tag)?),
            "minecraft:type_specific/lightning" => EntitySubPredicate::Lightning(LightningBoltPredicate::decode(tag)?),
            "minecraft:type_specific/fishing_hook" => EntitySubPredicate::FishingHook(FishingHookPredicate::decode(tag)?),
            "minecraft:type_specific/player" => EntitySubPredicate::Player(PlayerPredicate::decode(tag)?),
            "minecraft:type_specific/cube_mob" => EntitySubPredicate::CubeMob(CubeMobPredicate::decode(tag)?),
            "minecraft:type_specific/raider" => EntitySubPredicate::Raider(RaiderPredicate::decode(tag)?),
            "minecraft:type_specific/sheep" => EntitySubPredicate::Sheep(SheepPredicate::decode(tag)?),
            _ => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:entity_sub_predicate_type]: {id}")),
        })
    }

    pub fn predicate_type_order(&self) -> i32 {
        match self {
            EntitySubPredicate::EntityType(_) => -1,
            EntitySubPredicate::Vehicle(_)
            | EntitySubPredicate::Passenger(_)
            | EntitySubPredicate::TargetedEntity(_)
            | EntitySubPredicate::Lightning(_)
            | EntitySubPredicate::Player(_) => 1,
            EntitySubPredicate::Nbt(_) => 2,
            _ => 0,
        }
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        match self {
            EntitySubPredicate::EntityType(p) => p.matches(level, entity),
            EntitySubPredicate::Location(p) => p.matches(level, entity),
            EntitySubPredicate::SteppingOn(p) => p.matches(level, entity),
            EntitySubPredicate::MovementAffectedBy(p) => p.matches(level, entity),
            EntitySubPredicate::Distance(p) => p.matches(level, entity, position),
            EntitySubPredicate::Movement(p) => p.matches_entity(level, entity),
            EntitySubPredicate::Effects(p) => p.matches(level, entity),
            EntitySubPredicate::Nbt(p) => p.matches(level, entity),
            EntitySubPredicate::Flags(p) => p.matches(level, entity),
            EntitySubPredicate::Equipment(p) => p.matches(level, entity),
            EntitySubPredicate::PeriodicTick(p) => p.matches(level, entity),
            EntitySubPredicate::Vehicle(p) => p.matches(level, entity, position),
            EntitySubPredicate::Passenger(p) => p.matches(level, entity, position),
            EntitySubPredicate::TargetedEntity(p) => p.matches(level, entity, position),
            EntitySubPredicate::Team(p) => p.matches(level, entity),
            EntitySubPredicate::Slots(p) => p.matches(level, entity),
            EntitySubPredicate::Components(p) => p.matches(level, entity),
            EntitySubPredicate::Predicates(p) => p.matches(level, entity),
            EntitySubPredicate::EntityTags(p) => p.matches(level, entity),
            EntitySubPredicate::Lightning(p) => p.matches(level, entity, position),
            EntitySubPredicate::FishingHook(p) => p.matches(level, entity),
            EntitySubPredicate::Player(p) => p.matches(level, entity),
            EntitySubPredicate::CubeMob(p) => p.matches(level, entity),
            EntitySubPredicate::Raider(p) => p.matches(level, entity),
            EntitySubPredicate::Sheep(p) => p.matches(level, entity),
        }
    }
}
