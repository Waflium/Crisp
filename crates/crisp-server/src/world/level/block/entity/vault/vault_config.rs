use std::borrow::Cow;

use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::parse_identifier;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::trialspawner::player_detector::{EntitySelector, PlayerDetector};

pub const TAG_NAME: &str = "config";
const DEFAULT_LOOT_TABLE: &str = "minecraft:chests/trial_chambers/reward";
const DEFAULT_ACTIVATION_RANGE: f64 = 4.0;
pub const DEFAULT_DEACTIVATION_RANGE: f64 = 4.5;

#[derive(Clone, Debug)]
pub struct VaultConfig {
    pub loot_table: Cow<'static, str>,
    pub activation_range: f64,
    pub deactivation_range: f64,
    pub key_item: ItemStack,
    pub override_loot_table_to_display: Option<String>,
    pub player_detector: PlayerDetector,
    pub entity_selector: EntitySelector,
}

impl Default for VaultConfig {
    fn default() -> VaultConfig {
        VaultConfig {
            loot_table: Cow::Borrowed(DEFAULT_LOOT_TABLE),
            activation_range: DEFAULT_ACTIVATION_RANGE,
            deactivation_range: DEFAULT_DEACTIVATION_RANGE,
            key_item: ItemStack::new(items::TRIAL_KEY, 1),
            override_loot_table_to_display: None,
            player_detector: PlayerDetector::IncludingCreativePlayers,
            entity_selector: EntitySelector::SelectFromLevel,
        }
    }
}

fn loot_table_key(tag: Option<&Nbt>) -> Option<String> {
    tag.and_then(Nbt::as_str).and_then(parse_identifier)
}

pub fn double(tag: Option<&Nbt>, default: f64) -> f64 {
    tag.and_then(Nbt::as_f64).unwrap_or(default)
}

pub fn lenient_item_stack(tag: Option<&Nbt>) -> ItemStack {
    tag.and_then(|tag| ItemStack::from_nbt(tag).ok()).unwrap_or_else(ItemStack::empty)
}

impl VaultConfig {
    pub fn decode(tag: &Nbt) -> Result<VaultConfig, String> {
        let Nbt::Compound(_) = tag else { return Err("Not a map".into()) };
        let config = VaultConfig {
            loot_table: loot_table_key(tag.get("loot_table")).map_or(Cow::Borrowed(DEFAULT_LOOT_TABLE), Cow::Owned),
            activation_range: double(tag.get("activation_range"), DEFAULT_ACTIVATION_RANGE),
            deactivation_range: double(tag.get("deactivation_range"), DEFAULT_DEACTIVATION_RANGE),
            key_item: lenient_item_stack(tag.get("key_item")),
            override_loot_table_to_display: loot_table_key(tag.get("override_loot_table_to_display")),
            ..VaultConfig::default()
        };
        config.validate()
    }

    fn validate(self) -> Result<VaultConfig, String> {
        if self.activation_range > self.deactivation_range {
            Err(format!("Activation range must ({}) be less or equal to deactivation range ({})", self.activation_range, self.deactivation_range))
        } else {
            Ok(self)
        }
    }

    pub fn encode(&self) -> Nbt {
        let mut output = Vec::new();
        if self.loot_table != DEFAULT_LOOT_TABLE {
            output.push(("loot_table".into(), Nbt::String(self.loot_table.to_string())));
        }
        if self.activation_range != DEFAULT_ACTIVATION_RANGE {
            output.push(("activation_range".into(), Nbt::Double(self.activation_range)));
        }
        if self.deactivation_range != DEFAULT_DEACTIVATION_RANGE {
            output.push(("deactivation_range".into(), Nbt::Double(self.deactivation_range)));
        }
        if !self.key_item.is_empty() {
            output.push(("key_item".into(), self.key_item.to_nbt()));
        }
        if let Some(override_loot_table_to_display) = &self.override_loot_table_to_display {
            output.push(("override_loot_table_to_display".into(), Nbt::String(override_loot_table_to_display.clone())));
        }
        Nbt::Compound(output)
    }
}
