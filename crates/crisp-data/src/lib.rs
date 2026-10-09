mod block;
pub mod block_entity;
pub mod damage_type;
pub mod commands;
pub mod game_rules;
mod fluid;
mod item;
mod mob_effect;
pub mod properties;
mod registries;
mod tag;

pub use block::{Block, BlockInfo, BlockProperty, BlockState, PushReaction, SoundType, SupportType, VoxelShape};
pub use block_entity::BlockEntityType;
pub use fluid::{Fluid, FluidState};
pub use game_rules::{GameRule, GameRuleType};
pub use damage_type::{DamageEffects, DamageScaling, DamageType, DamageTypeInfo, DeathMessageType};
pub use item::{
    ConsumeEffect, Consumable, EquipmentSlot, EquipmentSlotGroup, FoodProperties, Item, ItemAttributeModifier, ItemInfo, ItemUseAnimation, Rarity, ToolInfo,
    ToolRule, ToolRuleBlocks,
};
pub use mob_effect::{AttributeInfo, EffectAttributeModifier, EffectInstanceTemplate, MobEffect, MobEffectCategory, MobEffectInfo, Sentiment};
pub use properties::{Property, PropertyValue};
pub use registries::{DimensionType, RegistryTags, SyncedRegistry, packet_name, registry_entries, registry_id, tag};
pub use tag::Tag;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
