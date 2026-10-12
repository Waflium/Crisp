use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::advancement_holder::AdvancementHolder;
use super::advancement_requirements::AdvancementRequirements;
use super::advancement_rewards::AdvancementRewards;
use super::display_info::DisplayInfo;
use super::triggers::criterion::Criterion;
use crate::network::chat::component_utils;
use crate::network::chat::{Component, HoverEvent, Style};
use crate::util::codec::{self, DataResult};

pub struct Advancement {
    pub parent: Option<String>,
    pub display: Option<DisplayInfo>,
    pub rewards: AdvancementRewards,
    pub criteria: Vec<(String, Criterion)>,
    pub requirements: AdvancementRequirements,
    pub sends_telemetry_event: bool,
    pub name: Option<Component>,
}

impl Advancement {
    pub fn decode(tag: &Nbt) -> DataResult<Advancement> {
        codec::map(tag)?;
        let parent = codec::decode_optional(tag, "parent", codec::identifier)?;
        let display = codec::decode_optional(tag, "display", DisplayInfo::decode)?;
        let rewards = codec::decode_or(tag, "rewards", AdvancementRewards::default(), AdvancementRewards::decode)?;
        let criteria = codec::decode_field(tag, "criteria", |t| codec::unbounded_map(t, |name| Ok(name.to_string()), Criterion::decode))?;
        if criteria.is_empty() {
            return Err("Advancement criteria cannot be empty".into());
        }
        let requirements = match codec::decode_optional(tag, "requirements", AdvancementRequirements::decode)? {
            Some(requirements) => requirements,
            None => AdvancementRequirements::all_of(criteria.iter().map(|(name, _)| name.as_str())),
        };
        let sends_telemetry_event = codec::decode_or(tag, "sends_telemetry_event", false, codec::bool_value)?;
        let name = display.as_ref().map(Advancement::decorate_name);
        let advancement = Advancement { parent, display, rewards, criteria, requirements, sends_telemetry_event, name };
        advancement.validate()?;
        Ok(advancement)
    }

    fn validate(&self) -> DataResult<()> {
        let is_root = self.parent.is_none();
        let is_visible = self.display.is_some();
        let has_background = self.display.as_ref().is_some_and(|display| display.background.is_some());
        if is_root && is_visible && !has_background {
            return Err("Visible advancement roots must have background".into());
        }
        if !is_root && has_background {
            return Err("Only advancement roots can have background".into());
        }
        let names: Vec<&str> = self.criteria.iter().map(|(name, _)| name.as_str()).collect();
        self.requirements.validate(&names)
    }

    pub fn write(&self, output: &mut Writer) {
        output.bool(self.parent.is_some());
        if let Some(parent) = &self.parent {
            output.string(parent);
        }
        output.bool(self.display.is_some());
        if let Some(display) = &self.display {
            display.serialize_to_network(output);
        }
        self.requirements.write(output);
        output.bool(self.sends_telemetry_event);
    }

    fn decorate_name(display: &DisplayInfo) -> Component {
        let display_title = &display.title;
        let color = display.ty.get_chat_color();
        let tooltip = component_utils::merge_styles(display_title.clone(), &Style { color: Some(color.color()), ..Style::default() })
            .append(Component::literal("\n"))
            .append(display.description.clone());
        let mut title = display_title.clone();
        title.style = Style { hover_event: Some(HoverEvent::ShowText(Box::new(tooltip))), ..Style::default() }.apply_to(&title.style);
        component_utils::wrap_in_square_brackets(title).with_color(color)
    }

    pub fn name(holder: &AdvancementHolder) -> Component {
        holder.value.name.clone().unwrap_or_else(|| Component::literal(holder.id.clone()))
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn criterion(&self, name: &str) -> Option<&Criterion> {
        self.criteria.iter().find(|(criterion, _)| criterion == name).map(|(_, criterion)| criterion)
    }
}
