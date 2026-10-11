use crisp_core::random::RandomSource;
use crisp_data::{MobEffect, items};
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

struct EffectEntry {
    effect: MobEffect,
    duration: Holder<ContextIntProvider>,
}

impl EffectEntry {
    fn decode(tag: &Nbt) -> DataResult<EffectEntry> {
        Ok(EffectEntry {
            effect: codec::decode_field(tag, "type", |t| {
                let id = codec::identifier(t)?;
                MobEffect::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:mob_effect]: {id}"))
            })?,
            duration: codec::decode_field(tag, "duration", ContextIntProvider::decode_holder)?,
        })
    }
}

pub struct SetStewEffectFunction {
    effects: Vec<EffectEntry>,
}

impl SetStewEffectFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetStewEffectFunction> {
        let effects = codec::decode_or(tag, "effects", Vec::new(), |t| codec::list_of(t, EffectEntry::decode))?;
        for (i, entry) in effects.iter().enumerate() {
            if effects[..i].iter().any(|seen| seen.effect == entry.effect) {
                return Err(format!("Encountered duplicate mob effect: '{}'", entry.effect.info().name));
            }
        }
        Ok(SetStewEffectFunction { effects })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if !item_stack.is(items::SUSPICIOUS_STEW) || self.effects.is_empty() {
            return item_stack;
        }
        let entry = &self.effects[context.random().next_int_bounded(self.effects.len() as i32) as usize];
        let effect = entry.effect;
        let mut duration = entry.duration.value().get_int(context);
        if !effect.info().instantaneous {
            duration = duration.wrapping_mul(20);
        }
        let mut effects = item_stack.get(dc::SUSPICIOUS_STEW_EFFECTS).map_or(Vec::new(), |e| e.as_list().to_vec());
        effects.push(Nbt::Compound(vec![("id".into(), Nbt::String(effect.info().name.to_string())), ("duration".into(), Nbt::Int(duration))]));
        let value = Nbt::List(effects);
        let value = dc::SUSPICIOUS_STEW_EFFECTS.decode(&value).unwrap_or(value);
        item_stack.set(dc::SUSPICIOUS_STEW_EFFECTS, Some(value));
        item_stack
    }
}
