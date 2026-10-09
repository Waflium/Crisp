use crate::{DAMAGE_TYPES, SyncedRegistry, Tag};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DamageScaling {
    Never,
    WhenCausedByLivingNonPlayer,
    Always,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DamageEffects {
    Hurt,
    Thorns,
    Drowning,
    Burning,
    Poking,
    Freezing,
}

impl DamageEffects {
    pub fn sound(self) -> &'static str {
        match self {
            DamageEffects::Hurt | DamageEffects::Thorns => "minecraft:entity.player.hurt",
            DamageEffects::Drowning => "minecraft:entity.player.hurt_drown",
            DamageEffects::Burning => "minecraft:entity.player.hurt_on_fire",
            DamageEffects::Poking => "minecraft:entity.player.hurt_sweet_berry_bush",
            DamageEffects::Freezing => "minecraft:entity.player.hurt_freeze",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeathMessageType {
    Default,
    FallVariants,
    IntentionalGameDesign,
}

#[derive(Debug)]
pub struct DamageTypeInfo {
    pub name: &'static str,
    pub msg_id: &'static str,
    pub scaling: DamageScaling,
    pub exhaustion: f32,
    pub effects: DamageEffects,
    pub death_message_type: DeathMessageType,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DamageType(pub u16);

impl DamageType {
    pub fn info(self) -> &'static DamageTypeInfo {
        &DAMAGE_TYPES[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<DamageType> {
        SyncedRegistry::get("minecraft:damage_type").id_of(name).map(|id| DamageType(id as u16))
    }

    pub fn is(self, tag: &Tag) -> bool {
        tag.contains(self.0 as usize)
    }
}
