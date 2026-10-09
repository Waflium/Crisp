use crate::{Block, EffectInstanceTemplate, ITEMS, MobEffect, Tag};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Item(pub u16);

pub struct ItemInfo {
    pub name: &'static str,
    pub description_id: &'static str,
    pub max_stack_size: u8,
    pub max_damage: i32,
    pub tool: Option<ToolInfo>,
    pub equipment_slot: Option<EquipmentSlot>,
    pub attribute_modifiers: &'static [ItemAttributeModifier],
    pub food: Option<FoodProperties>,
    pub consumable: Option<Consumable>,
    pub use_remainder: Option<Item>,
    pub damage_on_hurt: bool,
    pub damage_resistant: Option<&'static Tag>,
    pub death_protection: Option<&'static [ConsumeEffect]>,
    pub block: Option<Block>,
    pub classes: &'static [&'static str],
    pub fields: &'static str,
    pub rarity: Rarity,
    pub components: &'static [&'static str],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl Rarity {
    pub const VALUES: [Rarity; 4] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic];

    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EquipmentSlot {
    Mainhand,
    Offhand,
    Feet,
    Legs,
    Chest,
    Head,
    Body,
    Saddle,
}

impl EquipmentSlot {
    pub fn by_name(name: &str) -> Option<EquipmentSlot> {
        Some(match name {
            "mainhand" => EquipmentSlot::Mainhand,
            "offhand" => EquipmentSlot::Offhand,
            "feet" => EquipmentSlot::Feet,
            "legs" => EquipmentSlot::Legs,
            "chest" => EquipmentSlot::Chest,
            "head" => EquipmentSlot::Head,
            "body" => EquipmentSlot::Body,
            "saddle" => EquipmentSlot::Saddle,
            _ => return None,
        })
    }
}

impl EquipmentSlot {
    pub const VALUES: [EquipmentSlot; 8] = [
        EquipmentSlot::Mainhand,
        EquipmentSlot::Offhand,
        EquipmentSlot::Feet,
        EquipmentSlot::Legs,
        EquipmentSlot::Chest,
        EquipmentSlot::Head,
        EquipmentSlot::Body,
        EquipmentSlot::Saddle,
    ];

    pub fn is_armor(self) -> bool {
        matches!(self, EquipmentSlot::Feet | EquipmentSlot::Legs | EquipmentSlot::Chest | EquipmentSlot::Head | EquipmentSlot::Body)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EquipmentSlotGroup {
    Any,
    Mainhand,
    Offhand,
    Hand,
    Feet,
    Legs,
    Chest,
    Head,
    Armor,
    Body,
    Saddle,
}

impl EquipmentSlotGroup {
    pub fn test(self, slot: EquipmentSlot) -> bool {
        use EquipmentSlot as S;
        match self {
            EquipmentSlotGroup::Any => true,
            EquipmentSlotGroup::Mainhand => slot == S::Mainhand,
            EquipmentSlotGroup::Offhand => slot == S::Offhand,
            EquipmentSlotGroup::Hand => matches!(slot, S::Mainhand | S::Offhand),
            EquipmentSlotGroup::Feet => slot == S::Feet,
            EquipmentSlotGroup::Legs => slot == S::Legs,
            EquipmentSlotGroup::Chest => slot == S::Chest,
            EquipmentSlotGroup::Head => slot == S::Head,
            EquipmentSlotGroup::Armor => slot.is_armor(),
            EquipmentSlotGroup::Body => slot == S::Body,
            EquipmentSlotGroup::Saddle => slot == S::Saddle,
        }
    }
}

#[derive(Debug)]
pub struct ItemAttributeModifier {
    pub attribute: &'static str,
    pub id: &'static str,
    pub amount: f64,
    pub operation: u8,
    pub slot: EquipmentSlotGroup,
}

#[derive(Clone, Copy, Debug)]
pub struct FoodProperties {
    pub nutrition: i32,
    pub saturation: f32,
    pub can_always_eat: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemUseAnimation {
    None,
    Eat,
    Drink,
    Block,
    Bow,
    Spear,
    Crossbow,
    Spyglass,
    TootHorn,
    Brush,
    Bundle,
    Trident,
}

#[derive(Debug)]
pub enum ConsumeEffect {
    ApplyEffects { effects: &'static [EffectInstanceTemplate], probability: f32 },
    RemoveEffects { effects: &'static [MobEffect] },
    ClearAllEffects,
    TeleportRandomly { diameter: f32 },
    PlaySound { sound: &'static str },
}

#[derive(Debug)]
pub struct Consumable {
    pub consume_seconds: f32,
    pub animation: ItemUseAnimation,
    pub sound: &'static str,
    pub has_consume_particles: bool,
    pub on_consume_effects: &'static [ConsumeEffect],
}

impl Consumable {
    pub fn consume_ticks(&self) -> i32 {
        (self.consume_seconds * 20.0) as i32
    }

    pub fn should_emit_particles_and_sounds(&self, use_item_remaining_ticks: i32) -> bool {
        let item_used_for_ticks = self.consume_ticks() - use_item_remaining_ticks;
        let wait_ticks_before_use_effects = (self.consume_ticks() as f32 * 0.21875) as i32;
        item_used_for_ticks > wait_ticks_before_use_effects && use_item_remaining_ticks % 4 == 0
    }
}

pub struct ToolRule {
    pub blocks: ToolRuleBlocks,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}

pub enum ToolRuleBlocks {
    Blocks(&'static [Block]),
    Tag(&'static Tag),
}

impl ToolRule {
    pub fn matches(&self, block: Block) -> bool {
        match self.blocks {
            ToolRuleBlocks::Blocks(list) => list.contains(&block),
            ToolRuleBlocks::Tag(tag) => block.is(tag),
        }
    }
}

pub struct ToolInfo {
    pub default_mining_speed: f32,
    pub damage_per_block: i32,
    pub rules: &'static [ToolRule],
}

impl ToolInfo {
    pub fn mining_speed(&self, block: Block) -> f32 {
        self.rules.iter().find(|r| r.speed.is_some() && r.matches(block)).and_then(|r| r.speed).unwrap_or(self.default_mining_speed)
    }

    pub fn is_correct_for_drops(&self, block: Block) -> bool {
        self.rules.iter().find(|r| r.correct_for_drops.is_some() && r.matches(block)).and_then(|r| r.correct_for_drops).unwrap_or(false)
    }
}

impl Item {
    pub fn crafting_remainder(self) -> Option<Item> {
        let remainder = crate::CRAFTING_REMAINDERS[self.0 as usize];
        (remainder != u16::MAX).then_some(Item(remainder))
    }

    pub fn info(self) -> &'static ItemInfo {
        &ITEMS[self.0 as usize]
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn by_name(name: &str) -> Option<Item> {
        ITEMS.iter().position(|i| i.name == name).map(|i| Item(i as u16))
    }

    pub fn is(self, tag: &Tag) -> bool {
        tag.contains(self.0 as usize)
    }

    pub fn is_instance_of(self, class: &str) -> bool {
        self.info().classes.contains(&class)
    }

    pub fn count() -> usize {
        ITEMS.len()
    }
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
