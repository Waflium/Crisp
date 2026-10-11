use crisp_protocol::nbt::Nbt;

use super::slot_collection::SlotCollection;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::{Holder, RegistryElement};
use crate::util::codec::{self, DataResult};
use crate::world::inventory::slot_ranges::{self, SlotRange};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::container_component_manipulator::ContainerComponentManipulator;
use crate::world::level::storage::loot::container_component_manipulators;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_context_arg::{ArgCodecBuilder, LootContextArg};
use crate::world::level::storage::loot::loot_params::SlotProvider;

pub enum SlotSource {
    Group(Vec<Holder<SlotSource>>),
    Filtered { slot_source: Holder<SlotSource>, filter: ItemPredicate },
    LimitSlots { slot_source: Holder<SlotSource>, limit: i32 },
    SlotRange { source: LootContextArg, slot_range: &'static SlotRange },
    Contents { slot_source: Holder<SlotSource>, component: ContainerComponentManipulator },
    Empty,
}

impl RegistryElement for SlotSource {
    const REGISTRY: &'static str = "minecraft:slot_source";

    fn contains_key(_key: &str) -> bool {
        false
    }

    fn lookup(_key: &str) -> Option<&'static SlotSource> {
        None
    }
}

const SOURCE_CODEC: ArgCodecBuilder = ArgCodecBuilder { any_entity: true, any_block_entity: true, any_item_stack: false, container: true };

fn get_slots_from_range(context: &mut LootContext, slot_provider: SlotProvider, slots: &[i32]) -> SlotCollection {
    let items = match slot_provider {
        SlotProvider::Entity(id) => slots.iter().filter_map(|&slot| context.params.level.get_slot(id, slot)).collect(),
        SlotProvider::BlockEntity(block_entity) => match block_entity.container_base() {
            Some(container) => slots.iter().filter_map(|&slot| usize::try_from(slot).ok().and_then(|slot| container.items.get(slot)).cloned()).collect(),
            None => return SlotCollection::EMPTY,
        },
    };
    SlotCollection::of(items)
}

impl SlotSource {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<SlotSource>> {
        Holder::decode(tag, |t| match codec::either(t, SlotSource::decode_typed, SlotSource::decode_list)? {
            Ok(source) => Ok(source),
            Err(terms) => Ok(SlotSource::Group(terms)),
        })
    }

    fn decode_list(tag: &Nbt) -> DataResult<Vec<Holder<SlotSource>>> {
        codec::compact_list_of(tag, |t| Holder::decode(t, SlotSource::decode_typed))
    }

    fn decode_typed(tag: &Nbt) -> DataResult<SlotSource> {
        codec::map(tag)?;
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        let slot_source = || codec::decode_field(tag, "slot_source", SlotSource::decode_holder);
        Ok(match ty.as_str() {
            "minecraft:group" => SlotSource::Group(codec::decode_field(tag, "terms", SlotSource::decode_list)?),
            "minecraft:filtered" => SlotSource::Filtered { slot_source: slot_source()?, filter: codec::decode_field(tag, "item_filter", ItemPredicate::decode)? },
            "minecraft:limit_slots" => SlotSource::LimitSlots {
                slot_source: slot_source()?,
                limit: codec::decode_field(tag, "limit", |t| {
                    let value = codec::int(t)?;
                    if value <= 0 {
                        return Err(format!("Value must be positive: {value}"));
                    }
                    Ok(value)
                })?,
            },
            "minecraft:slot_range" => SlotSource::SlotRange {
                source: codec::decode_or(tag, "source", LootContextArg::Container, |t| LootContextArg::decode(t, SOURCE_CODEC))?,
                slot_range: codec::decode_field(tag, "slots", |t| {
                    let name = codec::string(t)?;
                    slot_ranges::name_to_ids(name).ok_or_else(|| format!("Unknown slot range: {name}"))
                })?,
            },
            "minecraft:contents" => SlotSource::Contents { slot_source: slot_source()?, component: codec::decode_field(tag, "component", container_component_manipulators::decode)? },
            "minecraft:empty" => SlotSource::Empty,
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:slot_source_type]: {other}")),
        })
    }

    pub fn provide(&self, context: &mut LootContext) -> SlotCollection {
        match self {
            SlotSource::Group(terms) => SlotCollection::concat(terms.iter().map(|term| term.value().provide(context)).collect()),
            SlotSource::Filtered { slot_source, filter } => slot_source.value().provide(context).filter(|item| filter.test(item)),
            SlotSource::LimitSlots { slot_source, limit } => slot_source.value().provide(context).limit(*limit as usize),
            SlotSource::SlotRange { source, slot_range } => {
                let slot_provider = match *source {
                    LootContextArg::Entity(target) => target.get(&context.params).map(SlotProvider::Entity),
                    LootContextArg::BlockEntity => context.params.block_entity.map(SlotProvider::BlockEntity),
                    LootContextArg::Container => context.params.container,
                    LootContextArg::Tool => None,
                };
                match slot_provider {
                    Some(slot_provider) => get_slots_from_range(context, slot_provider, &slot_range.slots),
                    None => SlotCollection::EMPTY,
                }
            }
            SlotSource::Contents { slot_source, component } => slot_source.value().provide(context).flat_map(|item: ItemStack| match item.get(component.type_()) {
                Some(contents) => SlotCollection::of(component.item_copies(contents)),
                None => SlotCollection::EMPTY,
            }),
            SlotSource::Empty => SlotCollection::EMPTY,
        }
    }
}
