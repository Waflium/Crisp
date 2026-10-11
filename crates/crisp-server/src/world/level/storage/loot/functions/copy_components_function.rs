use crisp_protocol::nbt::Nbt;

use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::entity::entity_components::EntityComponents;
use crate::world::item::ItemStack;
use crate::world::item::component::DataComponentType;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_context_arg::{ArgCodecBuilder, LootContextArg};

const GETTER: ArgCodecBuilder = ArgCodecBuilder { any_entity: true, any_block_entity: true, any_item_stack: true, container: false };

pub struct CopyComponentsFunction {
    source: LootContextArg,
    include: Option<Vec<DataComponentType>>,
    exclude: Option<Vec<DataComponentType>>,
}

fn decode_component_type(tag: &Nbt) -> DataResult<DataComponentType> {
    let id = codec::identifier(tag)?;
    DataComponentType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"))
}

impl CopyComponentsFunction {
    pub fn decode(tag: &Nbt) -> DataResult<CopyComponentsFunction> {
        Ok(CopyComponentsFunction {
            source: codec::decode_field(tag, "source", |t| LootContextArg::decode(t, GETTER))?,
            include: codec::decode_optional(tag, "include", |t| codec::list_of(t, decode_component_type))?,
            exclude: codec::decode_optional(tag, "exclude", |t| codec::list_of(t, decode_component_type))?,
        })
    }

    fn baked_predicate(&self, ty: DataComponentType) -> bool {
        self.exclude.as_ref().is_none_or(|exclude| !exclude.contains(&ty)) && self.include.as_ref().is_none_or(|include| include.contains(&ty))
    }

    fn copy_typed(&self, item_stack: &mut ItemStack, data: &(impl DataComponentGetter + ?Sized)) {
        let all_types = || (0..).map_while(DataComponentType::by_id);
        let types: Vec<DataComponentType> = match &self.include {
            Some(include) => include.clone(),
            None => all_types().collect(),
        };
        for ty in types {
            if self.exclude.as_ref().is_some_and(|exclude| exclude.contains(&ty)) {
                continue;
            }
            if let Some(value) = data.get(ty) {
                item_stack.set(ty, Some(value.into_owned()));
            }
        }
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        match self.source {
            LootContextArg::Entity(target) => {
                if let Some(entity) = target.get(&context.params) {
                    let components = EntityComponents { level: context.params.level, id: entity };
                    self.copy_typed(&mut item_stack, &components);
                }
            }
            LootContextArg::BlockEntity => {
                if let Some(entity) = context.params.block_entity {
                    for (ty, value) in entity.collect_components() {
                        if self.baked_predicate(ty) {
                            item_stack.set(ty, value);
                        }
                    }
                }
            }
            LootContextArg::Tool => {
                if let Some(tool) = context.params.tool {
                    let tool = tool.clone();
                    self.copy_typed(&mut item_stack, &tool);
                }
            }
            LootContextArg::Container => {}
        }
        item_stack
    }
}
