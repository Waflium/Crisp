pub mod ageable_mob;
pub mod ai;
pub mod animal;
pub mod attributes;
pub mod data;
#[cfg(test)]
mod effect_tests;
#[allow(clippy::module_inception)]
mod entity;
pub mod entity_components;
mod entity_dimensions;
pub mod entity_event;
pub mod entity_reference;
pub mod entity_save;
pub mod entity_type;
pub mod entity_types;
pub mod experience_orb;
pub mod fluid_interaction;
pub mod inside_block;
pub mod item;
pub mod lightning_bolt;
pub mod living_entity;
pub mod mob;
pub mod monster;
pub mod pathfinder_mob;
pub mod portal_processor;
pub mod primed_tnt;
pub mod projectile;
pub mod raid;
pub mod player;
pub mod relative;
pub mod schedule;
pub mod slot_access;
#[cfg(test)]
mod survival_tests;
pub mod using_item;
pub mod vehicle;

pub use entity::*;
pub use entity_dimensions::EntityDimensions;
pub use item::ItemEntity;

pub enum LevelEntity {
    Item(ItemEntity),
    ExperienceOrb(experience_orb::ExperienceOrb),
    LightningBolt(lightning_bolt::LightningBolt),
    EyeOfEnder(projectile::eye_of_ender::EyeOfEnder),
    PrimedTnt(primed_tnt::PrimedTnt),
    FallingBlock(item::falling_block_entity::FallingBlockEntity),
    Mob(Box<mob::Mob>),
}

impl LevelEntity {
    pub fn name(&self) -> crate::network::chat::Component {
        match self {
            LevelEntity::Mob(mob) => match mob.entity.custom_name() {
                Some(name) => name,
                None => description(crisp_data::registry::ENTITY_TYPE[mob.entity.entity_type as usize]),
            },
            LevelEntity::Item(item) => crate::network::chat::Component::translatable(item.item.item.info().description_id),
            LevelEntity::LightningBolt(_) => crate::network::chat::Component::translatable("entity.minecraft.lightning_bolt"),
            LevelEntity::EyeOfEnder(_) => crate::network::chat::Component::translatable("entity.minecraft.eye_of_ender"),
            LevelEntity::PrimedTnt(_) => crate::network::chat::Component::translatable("entity.minecraft.tnt"),
            LevelEntity::FallingBlock(falling) => crate::network::chat::Component::translatable_with(
                "entity.minecraft.falling_block_type",
                vec![crate::network::chat::Arg::Component(crate::network::chat::Component::translatable(falling.get_block_state().block().info().description_id))],
            ),
            LevelEntity::ExperienceOrb(_) => crate::network::chat::Component::translatable("entity.minecraft.experience_orb"),
        }
    }

    pub fn display_name(&self, team: Option<&crate::world::scores::player_team::PlayerTeam>) -> crate::network::chat::Component {
        use crate::network::chat::HoverEvent;
        let entity = self.entity();
        let type_id = crisp_data::registry::ENTITY_TYPE[entity.entity_type as usize];
        crate::world::scores::player_team::PlayerTeam::format_name_for_team(team, self.name())
            .with_hover(HoverEvent::ShowEntity { id: type_id, uuid: entity.uuid, name: Some(Box::new(self.name())) })
            .with_insertion(&crate::world::level::storage::format_uuid(entity.uuid))
    }

    pub fn tracking_config(&self) -> crate::server::level::chunk_map::TrackingConfig {
        use crate::server::level::chunk_map::TrackingConfig;
        use crate::server::level::server_entity::UpdateInterval;
        match self {
            LevelEntity::Item(_) => TrackingConfig { range: 6 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: true },
            LevelEntity::PrimedTnt(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(10), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::FallingBlock(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::LightningBolt(_) => TrackingConfig { range: 16 * 16, update_interval: UpdateInterval::Never, track_deltas: true, stepped: false, is_item: false },
            LevelEntity::EyeOfEnder(_) => TrackingConfig { range: 4 * 16, update_interval: UpdateInterval::Periodic(4), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::ExperienceOrb(_) => TrackingConfig { range: 6 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::Mob(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(3), track_deltas: true, stepped: false, is_item: false },
        }
    }

    pub fn add_entity_data(&self) -> i32 {
        match self {
            LevelEntity::FallingBlock(falling) => falling.get_block_state().id() as i32,
            _ => 0,
        }
    }

    pub fn entity(&self) -> &Entity {
        match self {
            LevelEntity::Item(item) => &item.entity,
            LevelEntity::LightningBolt(bolt) => &bolt.entity,
            LevelEntity::EyeOfEnder(eye) => &eye.entity,
            LevelEntity::PrimedTnt(tnt) => &tnt.entity,
            LevelEntity::FallingBlock(falling) => &falling.entity,
            LevelEntity::ExperienceOrb(orb) => &orb.entity,
            LevelEntity::Mob(mob) => &mob.entity,
        }
    }

    pub fn entity_mut(&mut self) -> &mut Entity {
        match self {
            LevelEntity::Item(item) => &mut item.entity,
            LevelEntity::LightningBolt(bolt) => &mut bolt.entity,
            LevelEntity::EyeOfEnder(eye) => &mut eye.entity,
            LevelEntity::PrimedTnt(tnt) => &mut tnt.entity,
            LevelEntity::FallingBlock(falling) => &mut falling.entity,
            LevelEntity::ExperienceOrb(orb) => &mut orb.entity,
            LevelEntity::Mob(mob) => &mut mob.entity,
        }
    }

    pub fn as_item(&self) -> Option<&ItemEntity> {
        match self {
            LevelEntity::Item(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_item_mut(&mut self) -> Option<&mut ItemEntity> {
        match self {
            LevelEntity::Item(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_experience_orb(&self) -> Option<&experience_orb::ExperienceOrb> {
        match self {
            LevelEntity::ExperienceOrb(orb) => Some(orb),
            _ => None,
        }
    }
}

pub fn can_summon(entity_type: &str) -> bool {
    !matches!(entity_type, "minecraft:player" | "minecraft:fishing_bobber")
}

pub fn description(entity_type: &str) -> crate::network::chat::Component {
    crate::network::chat::Component::translatable(&format!("entity.{}", entity_type.replacen(':', ".", 1)))
}
