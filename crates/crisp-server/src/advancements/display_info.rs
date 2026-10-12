use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::advancement_type::AdvancementType;
use crate::network::chat::{Component, component_serialization};
use crate::util::codec::{self, DataResult};
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::crafting::display::slot_display::write_item_stack_template;

#[derive(Debug)]
pub struct DisplayInfo {
    pub icon: ItemStackTemplate,
    pub title: Component,
    pub description: Component,
    pub background: Option<String>,
    pub ty: AdvancementType,
    pub show_toast: bool,
    pub announce_to_chat: bool,
    pub hidden: bool,
}

impl DisplayInfo {
    pub fn decode(tag: &Nbt) -> DataResult<DisplayInfo> {
        codec::map(tag)?;
        Ok(DisplayInfo {
            icon: codec::decode_field(tag, "icon", ItemStackTemplate::decode)?,
            title: codec::decode_field(tag, "title", component_serialization::decode)?,
            description: codec::decode_field(tag, "description", component_serialization::decode)?,
            background: codec::decode_optional(tag, "background", codec::identifier)?,
            ty: codec::decode_or(tag, "frame", AdvancementType::Task, AdvancementType::decode)?,
            show_toast: codec::decode_or(tag, "show_toast", true, codec::bool_value)?,
            announce_to_chat: codec::decode_or(tag, "announce_to_chat", true, codec::bool_value)?,
            hidden: codec::decode_or(tag, "hidden", false, codec::bool_value)?,
        })
    }

    pub fn serialize_to_network(&self, output: &mut Writer) {
        self.title.write(output);
        self.description.write(output);
        write_item_stack_template(output, &self.icon);
        output.varint(self.ty.id());
        let mut flags = 0;
        if self.background.is_some() {
            flags |= 1;
        }
        if self.show_toast {
            flags |= 2;
        }
        if self.hidden {
            flags |= 4;
        }
        output.i32(flags);
        if let Some(background) = &self.background {
            output.string(background);
        }
    }
}
