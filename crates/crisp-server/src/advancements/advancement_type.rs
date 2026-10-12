use crisp_protocol::nbt::Nbt;

use super::advancement::Advancement;
use super::advancement_holder::AdvancementHolder;
use crate::network::chat::{ChatFormatting, Component};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdvancementType {
    Task,
    Challenge,
    Goal,
}

impl AdvancementType {
    pub fn decode(tag: &Nbt) -> DataResult<AdvancementType> {
        codec::string_enum(tag, &[("task", AdvancementType::Task), ("challenge", AdvancementType::Challenge), ("goal", AdvancementType::Goal)])
    }

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn get_chat_color(self) -> ChatFormatting {
        match self {
            AdvancementType::Task => ChatFormatting::Green,
            AdvancementType::Challenge => ChatFormatting::DarkPurple,
            AdvancementType::Goal => ChatFormatting::Green,
        }
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            AdvancementType::Task => "task",
            AdvancementType::Challenge => "challenge",
            AdvancementType::Goal => "goal",
        }
    }

    pub fn create_announcement(self, level: &ServerLevel, holder: &AdvancementHolder, player: EntityId) -> Component {
        Component::translatable_with(
            &format!("chat.type.advancement.{}", self.get_serialized_name()),
            vec![level.display_name(player).into(), Advancement::name(holder).into()],
        )
    }
}
