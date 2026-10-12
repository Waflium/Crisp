use crisp_data::{Item, registry, registry_id};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::world::item::component::codec;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{DataComponentPatch, DataComponentType, patch_of};

#[derive(Clone, Debug)]
pub enum SlotDisplay {
    Empty,
    AnyFuel,
    WithAnyPotion(Box<SlotDisplay>),
    OnlyWithComponent(Box<SlotDisplay>, DataComponentType),
    ItemSlotDisplay(Item),
    ItemStackSlotDisplay(ItemStackTemplate),
    TagSlotDisplay(HolderSet),
    DyedSlotDemo { dye: Box<SlotDisplay>, target: Box<SlotDisplay> },
    SmithingTrimDemoSlotDisplay { base: Box<SlotDisplay>, material: Box<SlotDisplay>, pattern: Nbt },
    WithRemainder { input: Box<SlotDisplay>, remainder: Box<SlotDisplay> },
    Composite(Vec<SlotDisplay>),
}

pub fn write_holder_set(w: &mut Writer, values: &HolderSet) {
    match values {
        HolderSet::Named(key, _) => {
            w.varint(0);
            w.string(key);
        }
        HolderSet::Direct(ids) => {
            w.varint(ids.len() as i32 + 1);
            ids.iter().for_each(|&id| w.varint(id));
        }
    }
}

pub fn write_item_stack_template(w: &mut Writer, template: &ItemStackTemplate) {
    w.varint(template.item().0 as i32);
    w.varint(template.count);
    let mut patch = DataComponentPatch::default();
    for (ty, value) in patch_of(&template.components).iter() {
        patch.put(ty, value.map(|value| ty.decode(value).unwrap_or_else(|_| value.clone())));
    }
    patch.write(w, false).expect("component value encodes");
}

impl SlotDisplay {
    fn type_name(&self) -> &'static str {
        match self {
            SlotDisplay::Empty => "minecraft:empty",
            SlotDisplay::AnyFuel => "minecraft:any_fuel",
            SlotDisplay::WithAnyPotion(_) => "minecraft:with_any_potion",
            SlotDisplay::OnlyWithComponent(..) => "minecraft:only_with_component",
            SlotDisplay::ItemSlotDisplay(_) => "minecraft:item",
            SlotDisplay::ItemStackSlotDisplay(_) => "minecraft:item_stack",
            SlotDisplay::TagSlotDisplay(_) => "minecraft:tag",
            SlotDisplay::DyedSlotDemo { .. } => "minecraft:dyed",
            SlotDisplay::SmithingTrimDemoSlotDisplay { .. } => "minecraft:smithing_trim",
            SlotDisplay::WithRemainder { .. } => "minecraft:with_remainder",
            SlotDisplay::Composite(_) => "minecraft:composite",
        }
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(registry_id(registry::SLOT_DISPLAY, self.type_name()));
        match self {
            SlotDisplay::Empty | SlotDisplay::AnyFuel => {}
            SlotDisplay::WithAnyPotion(display) => display.write(w),
            SlotDisplay::OnlyWithComponent(source, component) => {
                source.write(w);
                w.varint(component.0 as i32);
            }
            SlotDisplay::ItemSlotDisplay(item) => w.varint(item.0 as i32),
            SlotDisplay::ItemStackSlotDisplay(stack) => write_item_stack_template(w, stack),
            SlotDisplay::TagSlotDisplay(tag) => write_holder_set(w, tag),
            SlotDisplay::DyedSlotDemo { dye, target } => {
                dye.write(w);
                target.write(w);
            }
            SlotDisplay::SmithingTrimDemoSlotDisplay { base, material, pattern } => {
                base.write(w);
                material.write(w);
                match pattern.as_str().and_then(|id| codec::registry_id("minecraft:trim_pattern", id).ok()) {
                    Some(id) => w.varint(id + 1),
                    None => w.varint(0),
                }
            }
            SlotDisplay::WithRemainder { input, remainder } => {
                input.write(w);
                remainder.write(w);
            }
            SlotDisplay::Composite(contents) => {
                w.varint(contents.len() as i32);
                contents.iter().for_each(|display| display.write(w));
            }
        }
    }
}
