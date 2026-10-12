use crisp_protocol::nbt::Nbt;

use crate::network::chat::{ClickEvent, Component, Contents};
use crate::world::item::dye_color::DyeColor;

pub const LINES: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct SignText {
    messages: Vec<Component>,
    filtered_messages: Vec<Component>,
    color: DyeColor,
    has_glowing_text: bool,
}

impl Default for SignText {
    fn default() -> SignText {
        SignText::empty()
    }
}

fn black() -> DyeColor {
    DyeColor::by_name("black").expect("black is a dye color")
}

impl SignText {
    pub fn empty() -> SignText {
        let empty_messages = vec![Component::empty(); LINES];
        SignText { messages: empty_messages.clone(), filtered_messages: empty_messages, color: black(), has_glowing_text: false }
    }

    pub fn new(messages: Vec<Component>, filtered_messages: Vec<Component>, color: DyeColor, has_glowing_text: bool) -> SignText {
        SignText { messages, filtered_messages, color, has_glowing_text }
    }

    pub fn from_nbt(tag: &Nbt) -> Option<SignText> {
        let lines = |key: &str| -> Option<Vec<Component>> { tag.get(key).map(|list| list.as_list().iter().map(Component::from_nbt).collect()) };
        let messages = lines("messages")?;
        let filtered_messages = lines("filtered_messages").unwrap_or_else(|| messages.clone());
        let color = tag.get("color").and_then(Nbt::as_str).and_then(DyeColor::by_name).unwrap_or_else(black);
        let has_glowing_text = tag.get("has_glowing_text").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        Some(SignText { messages, filtered_messages, color, has_glowing_text })
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("messages".to_string(), Nbt::List(self.messages.iter().map(Component::to_nbt).collect()))];
        if self.filtered_messages != self.messages {
            fields.push(("filtered_messages".into(), Nbt::List(self.filtered_messages.iter().map(Component::to_nbt).collect())));
        }
        fields.push(("color".into(), Nbt::String(self.color.name.into())));
        fields.push(("has_glowing_text".into(), Nbt::Byte(self.has_glowing_text as i8)));
        Nbt::Compound(fields)
    }

    pub fn has_glowing_text(&self) -> bool {
        self.has_glowing_text
    }

    pub fn with_glowing_text(&self, has_glowing_text: bool) -> Option<SignText> {
        (has_glowing_text != self.has_glowing_text).then(|| SignText { has_glowing_text, ..self.clone() })
    }

    pub fn get_color(&self) -> DyeColor {
        self.color
    }

    pub fn with_color(&self, color: DyeColor) -> Option<SignText> {
        (color != self.color).then(|| SignText { color, ..self.clone() })
    }

    pub fn has_message(&self, should_filter: bool) -> bool {
        self.get_messages(should_filter).iter().any(|component| !component.get_string().is_empty())
    }

    pub fn get_messages(&self, should_filter: bool) -> &[Component] {
        if should_filter { &self.filtered_messages } else { &self.messages }
    }

    pub fn has_any_click_commands(&self, should_filter: bool) -> bool {
        self.get_messages(should_filter).iter().any(|message| matches!(message.style.click_event, Some(ClickEvent::RunCommand(_))))
    }

    pub fn has_editable_text(&self, should_filter: bool) -> bool {
        self.get_messages(should_filter).iter().all(|message| *message == Component::empty() || matches!(message.contents, Contents::Literal(_)))
    }

    pub fn as_mutable(&self) -> Mutable {
        Mutable { messages: self.messages.clone(), filtered_messages: self.messages.clone(), color: self.color, is_text_glowing: self.has_glowing_text }
    }
}

pub struct Mutable {
    messages: Vec<Component>,
    filtered_messages: Vec<Component>,
    color: DyeColor,
    is_text_glowing: bool,
}

impl Mutable {
    pub fn as_immutable(self) -> SignText {
        SignText { messages: self.messages, filtered_messages: self.filtered_messages, color: self.color, has_glowing_text: self.is_text_glowing }
    }

    pub fn set_line_filtered(&mut self, index: usize, raw: Component, filtered: Component) -> &mut Mutable {
        self.messages[index] = raw;
        self.filtered_messages[index] = filtered;
        self
    }

    pub fn set_line(&mut self, index: usize, text: Component) -> &mut Mutable {
        self.set_line_filtered(index, text.clone(), text)
    }
}
