use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::get_item_enchantment_level;
use crate::world::level::storage::loot::loot_context::LootContext;

enum Formula {
    BinomialWithBonusCount { extra_rounds: i32, probability: f32 },
    OreDrops,
    UniformBonusCount { bonus_multiplier: i32 },
}

impl Formula {
    fn calculate_new_count(&self, random: &mut impl RandomSource, mut count: i32, level: i32) -> i32 {
        match self {
            Formula::BinomialWithBonusCount { extra_rounds, probability } => {
                for _ in 0..level + extra_rounds {
                    if random.next_float() < *probability {
                        count += 1;
                    }
                }
                count
            }
            Formula::OreDrops => {
                if level > 0 {
                    let bonus = (random.next_int_bounded(level + 2) - 1).max(0);
                    count * (bonus + 1)
                } else {
                    count
                }
            }
            Formula::UniformBonusCount { bonus_multiplier } => count + random.next_int_bounded(bonus_multiplier * level + 1),
        }
    }
}

pub struct ApplyBonusCount {
    enchantment: String,
    formula: Formula,
}

impl ApplyBonusCount {
    pub fn decode(tag: &Nbt) -> DataResult<ApplyBonusCount> {
        let formula_type = codec::decode_field(tag, "formula", codec::identifier)?;
        let parameters = codec::optional_field(tag, "parameters");
        let formula = match formula_type.as_str() {
            "minecraft:binomial_with_bonus_count" => {
                let parameters = parameters.ok_or("No key parameters")?;
                Formula::BinomialWithBonusCount {
                    extra_rounds: codec::decode_field(parameters, "extra", codec::int)?,
                    probability: codec::decode_field(parameters, "probability", codec::float)?,
                }
            }
            "minecraft:ore_drops" => Formula::OreDrops,
            "minecraft:uniform_bonus_count" => {
                let parameters = parameters.ok_or("No key parameters")?;
                Formula::UniformBonusCount { bonus_multiplier: codec::decode_field(parameters, "bonusMultiplier", codec::int)? }
            }
            other => return Err(format!("No formula type with id: '{other}'")),
        };
        Ok(ApplyBonusCount { enchantment: codec::decode_field(tag, "enchantment", Enchantment::decode_holder)?, formula })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if let Some(tool) = context.params.tool {
            let level = get_item_enchantment_level(&self.enchantment, tool);
            item_stack.count = self.formula.calculate_new_count(&mut context.random(),item_stack.count, level);
        }
        item_stack
    }
}
