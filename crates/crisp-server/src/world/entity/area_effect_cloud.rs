use crisp_core::{Vec3, mth};
use crisp_data::{registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::data::{DataValue, entity_defaults};
use super::entity_reference::EntityReference;
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::core::particles::particle_options::ParticleOptions;
use crate::core::particles::particle_types;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component as dc;
use crate::world::level::block::entity::int_value;

const TIME_BETWEEN_APPLICATIONS: i32 = 5;
const DATA_RADIUS: usize = 8;
const DATA_WAITING: usize = 9;
const DATA_PARTICLE: usize = 10;
const MAX_RADIUS: f32 = 32.0;
const MINIMAL_RADIUS: f32 = 0.5;
const DEFAULT_RADIUS: f32 = 3.0;
pub const HEIGHT: f32 = 0.5;
pub const INFINITE_DURATION: i32 = -1;
const DEFAULT_WAIT_TIME: i32 = 20;
const DEFAULT_REAPPLICATION_DELAY: i32 = 20;

fn entity_effect_particle(color: i32) -> ParticleOptions {
    particle_types::parse("minecraft:entity_effect", &Nbt::Compound(vec![("color".into(), Nbt::Int(color))])).expect("an entity effect particle")
}

pub struct AreaEffectCloud {
    pub entity: Entity,
    custom_particle: Option<ParticleOptions>,
    potion_contents: Option<Nbt>,
    potion_duration_scale: f32,
    victims: Vec<(EntityId, i32)>,
    pub duration: i32,
    pub wait_time: i32,
    pub reapplication_delay: i32,
    pub duration_on_use: i32,
    pub radius_on_use: f32,
    pub radius_per_tick: f32,
    owner: Option<EntityReference>,
}

impl AreaEffectCloud {
    pub fn of_type() -> AreaEffectCloud {
        let mut data = entity_defaults();
        data.push(DataValue::Float(DEFAULT_RADIUS));
        data.push(DataValue::Boolean(false));
        data.push(DataValue::Particle(entity_effect_particle(-1)));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:area_effect_cloud"), dimensions(DEFAULT_RADIUS), data);
        entity.no_physics = true;
        AreaEffectCloud {
            entity,
            custom_particle: None,
            potion_contents: None,
            potion_duration_scale: 1.0,
            victims: Vec::new(),
            duration: INFINITE_DURATION,
            wait_time: DEFAULT_WAIT_TIME,
            reapplication_delay: DEFAULT_REAPPLICATION_DELAY,
            duration_on_use: 0,
            radius_on_use: 0.0,
            radius_per_tick: 0.0,
            owner: None,
        }
    }

    pub fn new(x: f64, y: f64, z: f64) -> AreaEffectCloud {
        let mut cloud = AreaEffectCloud::of_type();
        cloud.entity.set_pos(Vec3::new(x, y, z));
        cloud
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.entity.data.set(DATA_RADIUS, DataValue::Float(mth::clamp_f(radius, 0.0, MAX_RADIUS)));
        self.refresh_dimensions();
    }

    fn refresh_dimensions(&mut self) {
        let position = self.entity.position();
        self.entity.dimensions = dimensions(self.get_radius());
        self.entity.set_pos(position);
    }

    pub fn get_radius(&self) -> f32 {
        match self.entity.data.get(DATA_RADIUS) {
            DataValue::Float(radius) => *radius,
            _ => DEFAULT_RADIUS,
        }
    }

    pub fn set_potion_contents(&mut self, contents: Option<Nbt>) {
        self.potion_contents = contents;
        self.update_particle();
    }

    pub fn set_custom_particle(&mut self, custom_particle: Option<ParticleOptions>) {
        self.custom_particle = custom_particle;
        self.update_particle();
    }

    fn update_particle(&mut self) {
        let particle = match &self.custom_particle {
            Some(custom_particle) => custom_particle.clone(),
            None => entity_effect_particle(self.potion_contents().get_color() | 0xFF000000u32 as i32),
        };
        self.entity.data.set(DATA_PARTICLE, DataValue::Particle(particle));
    }

    fn potion_contents(&self) -> PotionContents<'_> {
        static EMPTY: Nbt = Nbt::Compound(Vec::new());
        PotionContents::new(self.potion_contents.as_ref().unwrap_or(&EMPTY))
    }

    pub fn add_effect(&mut self, effect: &MobEffectInstance) {
        let contents = crate::world::item::alchemy::potion_contents::with_effect_added(self.potion_contents.as_ref(), effect);
        self.set_potion_contents(Some(contents));
    }

    fn set_waiting(&mut self, waiting: bool) {
        self.entity.data.set(DATA_WAITING, DataValue::Boolean(waiting));
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self.entity.data.get(DATA_WAITING), DataValue::Boolean(true))
    }

    pub fn set_owner(&mut self, owner: Option<u128>) {
        self.owner = owner.map(EntityReference::of);
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        if let Some(contents) = stack.get(dc::POTION_CONTENTS) {
            self.set_potion_contents(Some(contents.clone()));
        }
        if let Some(scale) = stack.get(dc::POTION_DURATION_SCALE).and_then(Nbt::as_f64) {
            self.potion_duration_scale = scale as f32;
        }
        self.entity.apply_components_from_item_stack(stack);
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Age".into(), Nbt::Int(self.entity.tick_count)));
        output.push(("Duration".into(), Nbt::Int(self.duration)));
        output.push(("WaitTime".into(), Nbt::Int(self.wait_time)));
        output.push(("ReapplicationDelay".into(), Nbt::Int(self.reapplication_delay)));
        output.push(("DurationOnUse".into(), Nbt::Int(self.duration_on_use)));
        output.push(("RadiusOnUse".into(), Nbt::Float(self.radius_on_use)));
        output.push(("RadiusPerTick".into(), Nbt::Float(self.radius_per_tick)));
        output.push(("Radius".into(), Nbt::Float(self.get_radius())));
        if let Some(custom_particle) = &self.custom_particle {
            let mut fields = vec![("type".to_string(), Nbt::String(custom_particle.particle_type.into()))];
            if let Nbt::Compound(options) = &custom_particle.options {
                fields.extend(options.iter().cloned());
            }
            output.push(("custom_particle".into(), Nbt::Compound(fields)));
        }
        EntityReference::store(self.owner, output, "Owner");
        if let Some(contents) = self.potion_contents.as_ref().filter(|c| !matches!(c, Nbt::Compound(fields) if fields.is_empty())) {
            output.push(("potion_contents".into(), contents.clone()));
        }
        if self.potion_duration_scale != 1.0 {
            output.push(("potion_duration_scale".into(), Nbt::Float(self.potion_duration_scale)));
        }
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        let int = |key: &str, default: i32| input.get(key).and_then(int_value).unwrap_or(default);
        let float = |key: &str, default: f32| input.get(key).and_then(Nbt::as_f64).map_or(default, |v| v as f32);
        self.entity.tick_count = int("Age", 0);
        self.duration = int("Duration", INFINITE_DURATION);
        self.wait_time = int("WaitTime", DEFAULT_WAIT_TIME);
        self.reapplication_delay = int("ReapplicationDelay", DEFAULT_REAPPLICATION_DELAY);
        self.duration_on_use = int("DurationOnUse", 0);
        self.radius_on_use = float("RadiusOnUse", 0.0);
        self.radius_per_tick = float("RadiusPerTick", 0.0);
        self.set_radius(float("Radius", DEFAULT_RADIUS));
        self.owner = EntityReference::read(input, "Owner");
        let custom_particle = input.get("custom_particle").and_then(|particle| {
            let particle_type = particle.get("type").and_then(Nbt::as_str)?;
            let particle_type = registry::PARTICLE_TYPE.iter().find(|&&name| Some(name.to_string()) == crate::commands::arguments::parse_identifier(particle_type))?;
            let options = match particle {
                Nbt::Compound(fields) => Nbt::Compound(fields.iter().filter(|(key, _)| key != "type").cloned().collect()),
                _ => Nbt::Compound(Vec::new()),
            };
            particle_types::parse(particle_type, &options).ok()
        });
        self.set_custom_particle(custom_particle);
        let contents = input.get("potion_contents").filter(|contents| dc::POTION_CONTENTS.decode(contents).is_ok()).cloned();
        self.set_potion_contents(contents);
        self.potion_duration_scale = float("potion_duration_scale", 1.0);
    }
}

fn dimensions(radius: f32) -> EntityDimensions {
    EntityDimensions::scalable(radius * 2.0, HEIGHT)
}

impl ServerLevel {
    fn area_effect_cloud(&mut self, id: EntityId) -> &mut AreaEffectCloud {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::AreaEffectCloud(cloud)) => cloud,
            _ => unreachable!("not an area effect cloud"),
        }
    }

    pub fn tick_area_effect_cloud(&mut self, id: EntityId) {
        self.entity_tick(id);
        self.area_effect_cloud_server_tick(id);
    }

    fn area_effect_cloud_server_tick(&mut self, id: EntityId) {
        let cloud = self.area_effect_cloud(id);
        let tick_count = cloud.entity.tick_count;
        if cloud.duration != INFINITE_DURATION && tick_count - cloud.wait_time >= cloud.duration {
            cloud.entity.discard();
            return;
        }
        let is_waiting = cloud.is_waiting();
        let should_wait = tick_count < cloud.wait_time;
        if is_waiting != should_wait {
            cloud.set_waiting(should_wait);
        }
        if should_wait {
            return;
        }
        let mut radius = cloud.get_radius();
        if cloud.radius_per_tick != 0.0 {
            radius += cloud.radius_per_tick;
            if radius < MINIMAL_RADIUS {
                cloud.entity.discard();
                return;
            }
            cloud.set_radius(radius);
        }
        if tick_count % TIME_BETWEEN_APPLICATIONS != 0 {
            return;
        }
        cloud.victims.retain(|&(_, until)| tick_count < until);
        if !cloud.potion_contents().has_effects() {
            cloud.victims.clear();
            return;
        }
        let all_effects = cloud.potion_contents().for_each_effect(cloud.potion_duration_scale);
        let (bb, position) = (cloud.entity.bounding_box(), cloud.entity.position());
        let entities: Vec<EntityId> = self.get_entities(None, &bb).into_iter().filter(|&entity| self.is_living(entity)).collect();
        for entity in entities {
            if self.area_effect_cloud(id).victims.iter().any(|&(victim, _)| victim == entity)
                || !self.is_affected_by_potions(entity)
                || all_effects.iter().all(|effect| !self.can_be_affected(entity, effect))
            {
                continue;
            }
            let e = self.entity(entity);
            let (xd, zd) = (e.x() - position.x, e.z() - position.z);
            let dist = xd * xd + zd * zd;
            if dist > (radius * radius) as f64 {
                continue;
            }
            let cloud = self.area_effect_cloud(id);
            let until = tick_count + cloud.reapplication_delay;
            cloud.victims.push((entity, until));
            let owner = cloud.owner.and_then(|owner| owner.get_entity(self)).filter(|&owner| self.is_living(owner));
            for effect in &all_effects {
                if effect.effect.info().instantaneous {
                    self.apply_instantaneous_effect(effect.effect, Some(id), owner, entity, effect.amplifier, 0.5);
                } else {
                    self.add_effect(entity, effect.clone(), Some(id));
                }
            }
            let cloud = self.area_effect_cloud(id);
            if cloud.radius_on_use != 0.0 {
                radius += cloud.radius_on_use;
                if radius < MINIMAL_RADIUS {
                    cloud.entity.discard();
                    return;
                }
                cloud.set_radius(radius);
            }
            if cloud.duration_on_use != 0 && cloud.duration != INFINITE_DURATION {
                cloud.duration += cloud.duration_on_use;
                if cloud.duration <= 0 {
                    cloud.entity.discard();
                    return;
                }
            }
        }
    }
}
