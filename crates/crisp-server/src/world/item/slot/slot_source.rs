use std::rc::Rc;

use crisp_protocol::nbt::Nbt;

use super::slot_collection::SlotCollection;
use crate::world::entity::slot_access::LevelSlot;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
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

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static SlotSource> {
        reloadable_server_registries::get().slot_sources.get(key)
    }
}

const SOURCE_CODEC: ArgCodecBuilder = ArgCodecBuilder { any_entity: true, any_block_entity: true, any_item_stack: false, container: true };

fn get_slots_from_range<'s>(context: &mut LootContext, slot_provider: SlotProvider, slots: &[i32]) -> SlotCollection<'s> {
    let slot_list = match slot_provider {
        SlotProvider::Entity(id) => slots.iter().filter_map(|&slot| context.params.level.get_slot(id, slot)).collect(),
        SlotProvider::BlockEntity(block_entity) => match block_entity.container_base() {
            Some(container) => {
                slots.iter().filter_map(|&slot| usize::try_from(slot).ok().and_then(|slot| container.items.get(slot)).cloned().map(LevelSlot::Detached)).collect()
            }
            None => return SlotCollection::Empty,
        },
        SlotProvider::Container(container) => slots.iter().filter_map(|&slot| context.params.level.get_container_slot(container, slot)).collect(),
    };
    SlotCollection::of(slot_list)
}

impl SlotSource {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<SlotSource>> {
        Holder::decode(tag, SlotSource::decode_direct)
    }

    pub fn decode_direct(tag: &Nbt) -> DataResult<SlotSource> {
        match codec::either_or_partial(tag, SlotSource::decode_typed, SlotSource::decode_holder_set)? {
            Ok(source) => Ok(source),
            Err(terms) => Ok(SlotSource::Group(terms)),
        }
    }

    fn decode_list(tag: &Nbt) -> DataResult<Vec<Holder<SlotSource>>> {
        SlotSource::decode_holder_set(tag).map_err(|error| error.message)
    }

    fn decode_holder_set(tag: &Nbt) -> Result<Vec<Holder<SlotSource>>, codec::DecodeError> {
        codec::holder_set(tag, Self::REGISTRY, |t| Holder::decode(t, SlotSource::decode_typed))
    }

    pub fn decode_typed(tag: &Nbt) -> DataResult<SlotSource> {
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

    pub fn provide(&self, context: &mut LootContext) -> SlotCollection<'_> {
        match self {
            SlotSource::Group(terms) => SlotCollection::concat(terms.iter().map(|term| term.value().provide(context)).collect()),
            SlotSource::Filtered { slot_source, filter } => slot_source.value().provide(context).filter(Rc::new(|item: &ItemStack| filter.test(item))),
            SlotSource::LimitSlots { slot_source, limit } => slot_source.value().provide(context).limit(*limit),
            SlotSource::SlotRange { source, slot_range } => {
                let slot_provider = match *source {
                    LootContextArg::Entity(target) => target.get(&context.params).map(SlotProvider::Entity),
                    LootContextArg::BlockEntity => context.params.block_entity.map(SlotProvider::BlockEntity),
                    LootContextArg::Container => context.params.container,
                    LootContextArg::Tool => None,
                };
                match slot_provider {
                    Some(slot_provider) => get_slots_from_range(context, slot_provider, &slot_range.slots),
                    None => SlotCollection::Empty,
                }
            }
            SlotSource::Contents { slot_source, component } => slot_source.value().provide(context).flat_map(*component),
            SlotSource::Empty => SlotCollection::Empty,
        }
    }
}
