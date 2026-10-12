use crisp_protocol::nbt::Nbt;

use super::*;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance as Instance;
use crate::util::codec::DataResult;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CriterionTrigger {
    Impossible,
    PlayerKilledEntity,
    EntityKilledPlayer,
    EnterBlock,
    InventoryChanged,
    RecipeUnlocked,
    PlayerHurtEntity,
    EntityHurtPlayer,
    EnchantedItem,
    FilledBucket,
    BrewedPotion,
    ConstructBeacon,
    UsedEnderEye,
    SummonedEntity,
    BredAnimals,
    Location,
    SleptInBed,
    CuredZombieVillager,
    VillagerTrade,
    ItemDurabilityChanged,
    Levitation,
    ChangedDimension,
    Tick,
    TameAnimal,
    PlacedBlock,
    ConsumeItem,
    EffectsChanged,
    UsedTotem,
    NetherTravel,
    FishingRodHooked,
    ChanneledLightning,
    ShotCrossbow,
    SpearMobs,
    KilledByArrow,
    HeroOfTheVillage,
    VoluntaryExile,
    SlideDownBlock,
    BeeNestDestroyed,
    TargetHit,
    ItemUsedOnBlock,
    DefaultBlockUse,
    AnyBlockUse,
    PlayerGeneratesContainerLoot,
    ThrownItemPickedUpByEntity,
    ThrownItemPickedUpByPlayer,
    PlayerInteractedWithEntity,
    PlayerShearedEquipment,
    StartedRiding,
    LightningStrike,
    UsingItem,
    FallFromHeight,
    RideEntityInLava,
    KillMobNearSculkCatalyst,
    AllayDropItemOnBlock,
    AvoidVibration,
    RecipeCrafted,
    CrafterRecipeCrafted,
    FallAfterExplosion,
}

impl CriterionTrigger {
    pub const COUNT: usize = 58;

    pub const VALUES: [CriterionTrigger; CriterionTrigger::COUNT] = [
        CriterionTrigger::Impossible,
        CriterionTrigger::PlayerKilledEntity,
        CriterionTrigger::EntityKilledPlayer,
        CriterionTrigger::EnterBlock,
        CriterionTrigger::InventoryChanged,
        CriterionTrigger::RecipeUnlocked,
        CriterionTrigger::PlayerHurtEntity,
        CriterionTrigger::EntityHurtPlayer,
        CriterionTrigger::EnchantedItem,
        CriterionTrigger::FilledBucket,
        CriterionTrigger::BrewedPotion,
        CriterionTrigger::ConstructBeacon,
        CriterionTrigger::UsedEnderEye,
        CriterionTrigger::SummonedEntity,
        CriterionTrigger::BredAnimals,
        CriterionTrigger::Location,
        CriterionTrigger::SleptInBed,
        CriterionTrigger::CuredZombieVillager,
        CriterionTrigger::VillagerTrade,
        CriterionTrigger::ItemDurabilityChanged,
        CriterionTrigger::Levitation,
        CriterionTrigger::ChangedDimension,
        CriterionTrigger::Tick,
        CriterionTrigger::TameAnimal,
        CriterionTrigger::PlacedBlock,
        CriterionTrigger::ConsumeItem,
        CriterionTrigger::EffectsChanged,
        CriterionTrigger::UsedTotem,
        CriterionTrigger::NetherTravel,
        CriterionTrigger::FishingRodHooked,
        CriterionTrigger::ChanneledLightning,
        CriterionTrigger::ShotCrossbow,
        CriterionTrigger::SpearMobs,
        CriterionTrigger::KilledByArrow,
        CriterionTrigger::HeroOfTheVillage,
        CriterionTrigger::VoluntaryExile,
        CriterionTrigger::SlideDownBlock,
        CriterionTrigger::BeeNestDestroyed,
        CriterionTrigger::TargetHit,
        CriterionTrigger::ItemUsedOnBlock,
        CriterionTrigger::DefaultBlockUse,
        CriterionTrigger::AnyBlockUse,
        CriterionTrigger::PlayerGeneratesContainerLoot,
        CriterionTrigger::ThrownItemPickedUpByEntity,
        CriterionTrigger::ThrownItemPickedUpByPlayer,
        CriterionTrigger::PlayerInteractedWithEntity,
        CriterionTrigger::PlayerShearedEquipment,
        CriterionTrigger::StartedRiding,
        CriterionTrigger::LightningStrike,
        CriterionTrigger::UsingItem,
        CriterionTrigger::FallFromHeight,
        CriterionTrigger::RideEntityInLava,
        CriterionTrigger::KillMobNearSculkCatalyst,
        CriterionTrigger::AllayDropItemOnBlock,
        CriterionTrigger::AvoidVibration,
        CriterionTrigger::RecipeCrafted,
        CriterionTrigger::CrafterRecipeCrafted,
        CriterionTrigger::FallAfterExplosion,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            CriterionTrigger::Impossible => "minecraft:impossible",
            CriterionTrigger::PlayerKilledEntity => "minecraft:player_killed_entity",
            CriterionTrigger::EntityKilledPlayer => "minecraft:entity_killed_player",
            CriterionTrigger::EnterBlock => "minecraft:enter_block",
            CriterionTrigger::InventoryChanged => "minecraft:inventory_changed",
            CriterionTrigger::RecipeUnlocked => "minecraft:recipe_unlocked",
            CriterionTrigger::PlayerHurtEntity => "minecraft:player_hurt_entity",
            CriterionTrigger::EntityHurtPlayer => "minecraft:entity_hurt_player",
            CriterionTrigger::EnchantedItem => "minecraft:enchanted_item",
            CriterionTrigger::FilledBucket => "minecraft:filled_bucket",
            CriterionTrigger::BrewedPotion => "minecraft:brewed_potion",
            CriterionTrigger::ConstructBeacon => "minecraft:construct_beacon",
            CriterionTrigger::UsedEnderEye => "minecraft:used_ender_eye",
            CriterionTrigger::SummonedEntity => "minecraft:summoned_entity",
            CriterionTrigger::BredAnimals => "minecraft:bred_animals",
            CriterionTrigger::Location => "minecraft:location",
            CriterionTrigger::SleptInBed => "minecraft:slept_in_bed",
            CriterionTrigger::CuredZombieVillager => "minecraft:cured_zombie_villager",
            CriterionTrigger::VillagerTrade => "minecraft:villager_trade",
            CriterionTrigger::ItemDurabilityChanged => "minecraft:item_durability_changed",
            CriterionTrigger::Levitation => "minecraft:levitation",
            CriterionTrigger::ChangedDimension => "minecraft:changed_dimension",
            CriterionTrigger::Tick => "minecraft:tick",
            CriterionTrigger::TameAnimal => "minecraft:tame_animal",
            CriterionTrigger::PlacedBlock => "minecraft:placed_block",
            CriterionTrigger::ConsumeItem => "minecraft:consume_item",
            CriterionTrigger::EffectsChanged => "minecraft:effects_changed",
            CriterionTrigger::UsedTotem => "minecraft:used_totem",
            CriterionTrigger::NetherTravel => "minecraft:nether_travel",
            CriterionTrigger::FishingRodHooked => "minecraft:fishing_rod_hooked",
            CriterionTrigger::ChanneledLightning => "minecraft:channeled_lightning",
            CriterionTrigger::ShotCrossbow => "minecraft:shot_crossbow",
            CriterionTrigger::SpearMobs => "minecraft:spear_mobs",
            CriterionTrigger::KilledByArrow => "minecraft:killed_by_arrow",
            CriterionTrigger::HeroOfTheVillage => "minecraft:hero_of_the_village",
            CriterionTrigger::VoluntaryExile => "minecraft:voluntary_exile",
            CriterionTrigger::SlideDownBlock => "minecraft:slide_down_block",
            CriterionTrigger::BeeNestDestroyed => "minecraft:bee_nest_destroyed",
            CriterionTrigger::TargetHit => "minecraft:target_hit",
            CriterionTrigger::ItemUsedOnBlock => "minecraft:item_used_on_block",
            CriterionTrigger::DefaultBlockUse => "minecraft:default_block_use",
            CriterionTrigger::AnyBlockUse => "minecraft:any_block_use",
            CriterionTrigger::PlayerGeneratesContainerLoot => "minecraft:player_generates_container_loot",
            CriterionTrigger::ThrownItemPickedUpByEntity => "minecraft:thrown_item_picked_up_by_entity",
            CriterionTrigger::ThrownItemPickedUpByPlayer => "minecraft:thrown_item_picked_up_by_player",
            CriterionTrigger::PlayerInteractedWithEntity => "minecraft:player_interacted_with_entity",
            CriterionTrigger::PlayerShearedEquipment => "minecraft:player_sheared_equipment",
            CriterionTrigger::StartedRiding => "minecraft:started_riding",
            CriterionTrigger::LightningStrike => "minecraft:lightning_strike",
            CriterionTrigger::UsingItem => "minecraft:using_item",
            CriterionTrigger::FallFromHeight => "minecraft:fall_from_height",
            CriterionTrigger::RideEntityInLava => "minecraft:ride_entity_in_lava",
            CriterionTrigger::KillMobNearSculkCatalyst => "minecraft:kill_mob_near_sculk_catalyst",
            CriterionTrigger::AllayDropItemOnBlock => "minecraft:allay_drop_item_on_block",
            CriterionTrigger::AvoidVibration => "minecraft:avoid_vibration",
            CriterionTrigger::RecipeCrafted => "minecraft:recipe_crafted",
            CriterionTrigger::CrafterRecipeCrafted => "minecraft:crafter_recipe_crafted",
            CriterionTrigger::FallAfterExplosion => "minecraft:fall_after_explosion",
        }
    }

    pub fn by_name(name: &str) -> Option<CriterionTrigger> {
        CriterionTrigger::VALUES.into_iter().find(|trigger| trigger.name() == name)
    }

    pub fn decode_instance(self, tag: &Nbt) -> DataResult<Instance> {
        Ok(match self {
            CriterionTrigger::Impossible => Instance::Impossible,
            CriterionTrigger::PlayerKilledEntity | CriterionTrigger::EntityKilledPlayer | CriterionTrigger::KillMobNearSculkCatalyst => {
                Instance::Killed(killed_trigger::TriggerInstance::decode(tag)?)
            }
            CriterionTrigger::EnterBlock => Instance::EnterBlock(enter_block_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::InventoryChanged => Instance::InventoryChange(inventory_change_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::RecipeUnlocked => Instance::RecipeUnlocked(recipe_unlocked_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::PlayerHurtEntity => Instance::PlayerHurtEntity(player_hurt_entity_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::EntityHurtPlayer => Instance::EntityHurtPlayer(entity_hurt_player_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::EnchantedItem => Instance::EnchantedItem(enchanted_item_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::FilledBucket => Instance::FilledBucket(filled_bucket_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::BrewedPotion => Instance::BrewedPotion(brewed_potion_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::ConstructBeacon => Instance::ConstructBeacon(construct_beacon_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::UsedEnderEye => Instance::UsedEnderEye(used_ender_eye_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::SummonedEntity => Instance::SummonedEntity(summoned_entity_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::BredAnimals => Instance::BredAnimals(bred_animals_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::Location
            | CriterionTrigger::SleptInBed
            | CriterionTrigger::Tick
            | CriterionTrigger::HeroOfTheVillage
            | CriterionTrigger::VoluntaryExile
            | CriterionTrigger::AvoidVibration => Instance::Player(player_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::CuredZombieVillager => Instance::CuredZombieVillager(cured_zombie_villager_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::VillagerTrade => Instance::Trade(trade_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::ItemDurabilityChanged => Instance::ItemDurability(item_durability_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::Levitation => Instance::Levitation(levitation_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::ChangedDimension => Instance::ChangeDimension(change_dimension_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::TameAnimal => Instance::TameAnimal(tame_animal_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::PlacedBlock | CriterionTrigger::ItemUsedOnBlock | CriterionTrigger::AllayDropItemOnBlock => {
                Instance::ItemUsedOnLocation(item_used_on_location_trigger::TriggerInstance::decode(tag)?)
            }
            CriterionTrigger::ConsumeItem => Instance::ConsumeItem(consume_item_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::EffectsChanged => Instance::EffectsChanged(effects_changed_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::UsedTotem => Instance::UsedTotem(used_totem_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::NetherTravel | CriterionTrigger::FallFromHeight | CriterionTrigger::RideEntityInLava => {
                Instance::Distance(distance_trigger::TriggerInstance::decode(tag)?)
            }
            CriterionTrigger::FishingRodHooked => Instance::FishingRodHooked(fishing_rod_hooked_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::ChanneledLightning => Instance::ChanneledLightning(channeled_lightning_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::ShotCrossbow => Instance::ShotCrossbow(shot_crossbow_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::SpearMobs => Instance::SpearMobs(spear_mobs_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::KilledByArrow => Instance::KilledByArrow(killed_by_arrow_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::SlideDownBlock => Instance::SlideDownBlock(slide_down_block_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::BeeNestDestroyed => Instance::BeeNestDestroyed(bee_nest_destroyed_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::TargetHit => Instance::TargetBlock(target_block_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::DefaultBlockUse => Instance::DefaultBlockInteraction(default_block_interaction_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::AnyBlockUse => Instance::AnyBlockInteraction(any_block_interaction_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::PlayerGeneratesContainerLoot => Instance::LootTable(loot_table_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::ThrownItemPickedUpByEntity | CriterionTrigger::ThrownItemPickedUpByPlayer => {
                Instance::PickedUpItem(picked_up_item_trigger::TriggerInstance::decode(tag)?)
            }
            CriterionTrigger::PlayerInteractedWithEntity | CriterionTrigger::PlayerShearedEquipment => {
                Instance::PlayerInteract(player_interact_trigger::TriggerInstance::decode(tag)?)
            }
            CriterionTrigger::StartedRiding => Instance::StartRiding(start_riding_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::LightningStrike => Instance::LightningStrike(lightning_strike_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::UsingItem => Instance::UsingItem(using_item_trigger::TriggerInstance::decode(tag)?),
            CriterionTrigger::RecipeCrafted | CriterionTrigger::CrafterRecipeCrafted => {
                Instance::RecipeCrafted(recipe_crafted_trigger::TriggerInstance::decode(tag)?)
            }
            CriterionTrigger::FallAfterExplosion => Instance::FallAfterExplosion(fall_after_explosion_trigger::TriggerInstance::decode(tag)?),
        })
    }
}
