use crisp_core::Vec3;
use crisp_data::BlockState;

use super::parameters::loot_context_params::LootContextParam;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::BlockEntity;

pub type DynamicDrop<'a> = Box<dyn Fn(&mut dyn FnMut(ItemStack)) + 'a>;

#[derive(Clone, Copy)]
pub enum SlotProvider<'a> {
    Entity(EntityId),
    BlockEntity(&'a BlockEntity),
}

pub struct LootParams<'a> {
    pub level: &'a mut ServerLevel,
    pub this_entity: Option<EntityId>,
    pub interacting_entity: Option<EntityId>,
    pub target_entity: Option<EntityId>,
    pub last_damage_player: Option<EntityId>,
    pub damage_source: Option<DamageSource>,
    pub attacking_entity: Option<EntityId>,
    pub direct_attacking_entity: Option<EntityId>,
    pub origin: Option<Vec3>,
    pub block_state: Option<BlockState>,
    pub block_entity: Option<&'a BlockEntity>,
    pub tool: Option<&'a ItemStack>,
    pub explosion_radius: Option<f32>,
    pub enchantment_level: Option<i32>,
    pub enchantment_active: Option<bool>,
    pub additional_cost_component_allowed: bool,
    pub container: Option<SlotProvider<'a>>,
    pub dynamic_drops: Vec<(&'static str, DynamicDrop<'a>)>,
    pub luck: f32,
}

impl<'a> LootParams<'a> {
    pub fn new(level: &'a mut ServerLevel) -> LootParams<'a> {
        LootParams {
            level,
            this_entity: None,
            interacting_entity: None,
            target_entity: None,
            last_damage_player: None,
            damage_source: None,
            attacking_entity: None,
            direct_attacking_entity: None,
            origin: None,
            block_state: None,
            block_entity: None,
            tool: None,
            explosion_radius: None,
            enchantment_level: None,
            enchantment_active: None,
            additional_cost_component_allowed: false,
            container: None,
            dynamic_drops: Vec::new(),
            luck: 0.0,
        }
    }

    pub fn has(&self, param: LootContextParam) -> bool {
        match param {
            LootContextParam::ThisEntity => self.this_entity.is_some(),
            LootContextParam::InteractingEntity => self.interacting_entity.is_some(),
            LootContextParam::TargetEntity => self.target_entity.is_some(),
            LootContextParam::LastDamagePlayer => self.last_damage_player.is_some(),
            LootContextParam::DamageSource => self.damage_source.is_some(),
            LootContextParam::AttackingEntity => self.attacking_entity.is_some(),
            LootContextParam::DirectAttackingEntity => self.direct_attacking_entity.is_some(),
            LootContextParam::Origin => self.origin.is_some(),
            LootContextParam::BlockState => self.block_state.is_some(),
            LootContextParam::BlockEntity => self.block_entity.is_some(),
            LootContextParam::Tool => self.tool.is_some(),
            LootContextParam::ExplosionRadius => self.explosion_radius.is_some(),
            LootContextParam::EnchantmentLevel => self.enchantment_level.is_some(),
            LootContextParam::EnchantmentActive => self.enchantment_active.is_some(),
            LootContextParam::AdditionalCostComponentAllowed => self.additional_cost_component_allowed,
            LootContextParam::Container => self.container.is_some(),
        }
    }

    pub fn add_dynamic_drops(&self, location: &str, output: &mut dyn FnMut(ItemStack)) {
        if let Some((_, dynamic_drop)) = self.dynamic_drops.iter().find(|(name, _)| *name == location) {
            dynamic_drop(output);
        }
    }
}
