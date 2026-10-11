use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::{DamageType, MobEffect, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::world::level::is_in_spawnable_bounds;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::util::java_round_f;
use crate::util::valueproviders::FloatProvider;
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::enchantment::enchanted_item_in_use::EnchantedItemInUse;
use crate::world::item::enchantment::level_based_value::LevelBasedValue;

#[allow(dead_code)]
#[derive(Debug)]
pub enum EnchantmentEntityEffect {
    AllOf(Vec<EnchantmentEntityEffect>),
    ApplyMobEffect { to_apply: HolderSet, min_duration: LevelBasedValue, max_duration: LevelBasedValue, min_amplifier: LevelBasedValue, max_amplifier: LevelBasedValue },
    ChangeItemDamage { amount: LevelBasedValue },
    DamageEntity { min_damage: LevelBasedValue, max_damage: LevelBasedValue, damage_type: DamageType },
    Explode(Nbt),
    Ignite { duration: LevelBasedValue },
    ApplyImpulse(Nbt),
    ApplyExhaustion { amount: LevelBasedValue },
    PlaySound { sound_events: Vec<i32>, volume: FloatProvider, pitch: FloatProvider },
    ReplaceBlock(Nbt),
    ReplaceDisk(Nbt),
    RunFunction(String),
    SetBlockProperties(Nbt),
    SpawnParticles(Nbt),
    SummonEntity { entity_types: HolderSet, join_team: bool },
}

fn float_provider(tag: &Nbt, min: f32, max: f32) -> DataResult<FloatProvider> {
    let provider = FloatProvider::parse(&codec::to_json(tag))?;
    let (lo, hi) = match provider {
        FloatProvider::Constant(value) => (value, value),
        FloatProvider::Uniform { min_inclusive, max_exclusive } => (min_inclusive, max_exclusive),
        FloatProvider::ClampedNormal { min, max, .. } => (min, max),
        FloatProvider::Trapezoid { min, max, .. } => (min, max),
    };
    if lo < min || hi > max {
        return Err(format!("Value provider too low or high: {lo} [{min}-{max}]"));
    }
    Ok(provider)
}

fn sound_event(tag: &Nbt) -> DataResult<i32> {
    let id = match tag {
        Nbt::String(_) => codec::identifier(tag)?,
        _ => codec::decode_field(tag, "sound_id", codec::identifier)?,
    };
    Ok(registry_id(registry::SOUND_EVENT, &id))
}

impl EnchantmentEntityEffect {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentEntityEffect> {
        codec::map(tag)?;
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let value = |key: &str| codec::decode_field(tag, key, LevelBasedValue::decode);
        Ok(match ty.as_str() {
            "minecraft:all_of" => EnchantmentEntityEffect::AllOf(codec::decode_field(tag, "effects", |t| codec::list_of(t, EnchantmentEntityEffect::decode))?),
            "minecraft:apply_mob_effect" => EnchantmentEntityEffect::ApplyMobEffect {
                to_apply: codec::decode_field(tag, "to_apply", |t| HolderSet::decode("minecraft:mob_effect", t))?,
                min_duration: value("min_duration")?,
                max_duration: value("max_duration")?,
                min_amplifier: value("min_amplifier")?,
                max_amplifier: value("max_amplifier")?,
            },
            "minecraft:change_item_damage" => EnchantmentEntityEffect::ChangeItemDamage { amount: value("amount")? },
            "minecraft:damage_entity" => EnchantmentEntityEffect::DamageEntity {
                min_damage: value("min_damage")?,
                max_damage: value("max_damage")?,
                damage_type: codec::decode_field(tag, "damage_type", |t| {
                    let id = codec::identifier(t)?;
                    DamageType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:damage_type]: {id}"))
                })?,
            },
            "minecraft:explode" => EnchantmentEntityEffect::Explode(tag.clone()),
            "minecraft:ignite" => EnchantmentEntityEffect::Ignite { duration: value("duration")? },
            "minecraft:apply_impulse" => EnchantmentEntityEffect::ApplyImpulse(tag.clone()),
            "minecraft:apply_exhaustion" => EnchantmentEntityEffect::ApplyExhaustion { amount: value("amount")? },
            "minecraft:play_sound" => EnchantmentEntityEffect::PlaySound {
                sound_events: codec::decode_field(tag, "sound", |t| codec::compact_list_of(t, sound_event))?,
                volume: codec::decode_field(tag, "volume", |t| float_provider(t, 1.0e-5, 10.0))?,
                pitch: codec::decode_field(tag, "pitch", |t| float_provider(t, 1.0e-5, 2.0))?,
            },
            "minecraft:replace_block" => EnchantmentEntityEffect::ReplaceBlock(tag.clone()),
            "minecraft:replace_disk" => EnchantmentEntityEffect::ReplaceDisk(tag.clone()),
            "minecraft:run_function" => EnchantmentEntityEffect::RunFunction(codec::decode_field(tag, "function", codec::identifier)?),
            "minecraft:set_block_properties" => EnchantmentEntityEffect::SetBlockProperties(tag.clone()),
            "minecraft:spawn_particles" => EnchantmentEntityEffect::SpawnParticles(tag.clone()),
            "minecraft:summon_entity" => EnchantmentEntityEffect::SummonEntity {
                entity_types: codec::decode_field(tag, "entity", |t| HolderSet::decode("minecraft:entity_type", t))?,
                join_team: codec::decode_or(tag, "join_team", false, codec::bool_value)?,
            },
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_entity_effect_type]: {other}")),
        })
    }

    pub fn apply(&self, level: &mut ServerLevel, enchantment_level: i32, item: &EnchantedItemInUse, entity: EntityId, position: Vec3) {
        match self {
            EnchantmentEntityEffect::AllOf(effects) => {
                for effect in effects {
                    effect.apply(level, enchantment_level, item, entity, position);
                }
            }
            EnchantmentEntityEffect::ApplyMobEffect { to_apply, min_duration, max_duration, min_amplifier, max_amplifier } => {
                if !level.is_living(entity) {
                    return;
                }
                let contents = to_apply.contents();
                if contents.is_empty() {
                    return;
                }
                let random = &mut level.entity_mut(entity).random;
                let selected = MobEffect(contents[random.next_int_bounded(contents.len() as i32) as usize] as u8);
                let ticks = java_round_f(mth::random_between(random, min_duration.calculate(enchantment_level), max_duration.calculate(enchantment_level)) * 20.0);
                let amplifier = java_round_f(mth::random_between(random, min_amplifier.calculate(enchantment_level), max_amplifier.calculate(enchantment_level))).max(0);
                level.add_effect(entity, MobEffectInstance::new(selected, ticks, amplifier), None);
            }
            EnchantmentEntityEffect::ChangeItemDamage { amount } => {
                let change = amount.calculate(enchantment_level) as i32;
                item.hurt_and_break(level, change);
            }
            EnchantmentEntityEffect::DamageEntity { min_damage, max_damage, damage_type } => {
                let random = &mut level.entity_mut(entity).random;
                let damage = mth::random_between(random, min_damage.calculate(enchantment_level), max_damage.calculate(enchantment_level));
                let source = DamageSource { causing_entity: item.owner, direct_entity: item.owner, ..DamageSource::new(*damage_type) };
                level.hurt_server(entity, source, damage);
            }
            EnchantmentEntityEffect::Ignite { duration } => {
                level.ignite_for_seconds(entity, duration.calculate(enchantment_level));
            }
            EnchantmentEntityEffect::ApplyExhaustion { amount } => {
                if level.is_player(entity) {
                    level.cause_food_exhaustion(entity, amount.calculate(enchantment_level));
                }
            }
            EnchantmentEntityEffect::PlaySound { sound_events, volume, pitch } => {
                if level.entity(entity).is_silent() {
                    return;
                }
                let index = mth::clamp_i(enchantment_level - 1, 0, sound_events.len() as i32 - 1) as usize;
                let source = level.sound_source(entity);
                let random = &mut level.entity_mut(entity).random;
                let (volume, pitch) = (volume.sample(random), pitch.sample(random));
                level.play_sound_at(None, position.x, position.y, position.z, sound_events[index], source, volume, pitch);
            }
            EnchantmentEntityEffect::SummonEntity { entity_types, join_team } => {
                let block_pos = BlockPos::containing(position.x, position.y, position.z);
                if !is_in_spawnable_bounds(block_pos) {
                    return;
                }
                let contents = entity_types.contents();
                if contents.is_empty() {
                    return;
                }
                let entity_type = registry::ENTITY_TYPE[contents[level.random.next_int_bounded(contents.len() as i32) as usize] as usize];
                let Some(spawned) = level.spawn_entity_type(entity_type, block_pos) else { return };
                if let Some(owner) = item.owner.filter(|&owner| level.is_player(owner))
                    && let Some(LevelEntity::LightningBolt(bolt)) = level.entities.get_mut(&spawned)
                {
                    bolt.cause = Some(owner);
                }
                if *join_team && let Some(team) = level.team_of(entity).map(|team| team.name().to_string()) {
                    let name = level.scoreboard_name(spawned);
                    level.scoreboard.add_player_to_team(&name, &team);
                }
                let e = level.entity_mut(spawned);
                let (y_rot, x_rot) = (e.y_rot, e.x_rot);
                e.snap_to(position.x, position.y, position.z, y_rot, x_rot);
            }
            EnchantmentEntityEffect::Explode(_)
            | EnchantmentEntityEffect::ApplyImpulse(_)
            | EnchantmentEntityEffect::ReplaceBlock(_)
            | EnchantmentEntityEffect::ReplaceDisk(_)
            | EnchantmentEntityEffect::RunFunction(_)
            | EnchantmentEntityEffect::SetBlockProperties(_)
            | EnchantmentEntityEffect::SpawnParticles(_) => {}
        }
    }
}
