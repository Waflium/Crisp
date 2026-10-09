use crisp_protocol::nbt::Nbt;

use super::team::{CollisionRule, Visibility};
use super::team_color::TeamColor;
use crate::network::chat::{Component, HoverEvent, Style};
use crate::util::java_collections::JavaHashSet;

pub struct PlayerTeam {
    pub(super) name: String,
    pub(super) players: JavaHashSet,
    pub(super) display_name: Component,
    pub(super) player_prefix: Component,
    pub(super) player_suffix: Component,
    pub(super) allow_friendly_fire: bool,
    pub(super) see_friendly_invisibles: bool,
    pub(super) name_tag_visibility: Visibility,
    pub(super) death_message_visibility: Visibility,
    pub(super) color: Option<TeamColor>,
    pub(super) collision_rule: CollisionRule,
    display_name_style: Style,
}

impl PlayerTeam {
    pub fn new(name: &str) -> PlayerTeam {
        PlayerTeam {
            name: name.to_string(),
            players: JavaHashSet::default(),
            display_name: Component::literal(name),
            player_prefix: Component::empty(),
            player_suffix: Component::empty(),
            allow_friendly_fire: true,
            see_friendly_invisibles: true,
            name_tag_visibility: Visibility::Always,
            death_message_visibility: Visibility::Always,
            color: None,
            collision_rule: CollisionRule::Always,
            display_name_style: Style {
                insertion: Some(name.to_string()),
                hover_event: Some(HoverEvent::ShowText(Box::new(Component::literal(name)))),
                ..Style::default()
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn players(&self) -> impl Iterator<Item = &str> {
        self.players.iter()
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn display_name(&self) -> &Component {
        &self.display_name
    }

    pub fn player_prefix(&self) -> &Component {
        &self.player_prefix
    }

    pub fn player_suffix(&self) -> &Component {
        &self.player_suffix
    }

    pub fn is_allow_friendly_fire(&self) -> bool {
        self.allow_friendly_fire
    }

    pub fn can_see_friendly_invisibles(&self) -> bool {
        self.see_friendly_invisibles
    }

    pub fn name_tag_visibility(&self) -> Visibility {
        self.name_tag_visibility
    }

    pub fn death_message_visibility(&self) -> Visibility {
        self.death_message_visibility
    }

    pub fn color(&self) -> Option<TeamColor> {
        self.color
    }

    pub fn collision_rule(&self) -> CollisionRule {
        self.collision_rule
    }

    fn apply_color(&self, result: Component) -> Component {
        match self.color {
            Some(color) => result.with_text_color(color.text_color()),
            None => result,
        }
    }

    pub fn formatted_display_name(&self) -> Component {
        let inner = self.display_name.clone().with_style(&self.display_name_style);
        self.apply_color(Component::translatable_with("chat.square_brackets", vec![inner.into()]))
    }

    pub fn formatted_name(&self, member_name: Component) -> Component {
        self.apply_color(Component::empty().append(self.player_prefix.clone()).append(member_name).append(self.player_suffix.clone()))
    }

    pub fn format_name_for_team(team: Option<&PlayerTeam>, name: Component) -> Component {
        match team {
            Some(team) => team.formatted_name(name),
            None => name,
        }
    }

    pub fn is_allied_to(&self, other: Option<&PlayerTeam>) -> bool {
        other.is_some_and(|other| other.name == self.name)
    }

    pub fn pack_options(&self) -> u8 {
        self.allow_friendly_fire as u8 | (self.see_friendly_invisibles as u8) << 1
    }

    pub fn pack(&self) -> Nbt {
        let mut fields = vec![("Name".to_string(), Nbt::String(self.name.clone())), ("DisplayName".into(), self.display_name.to_nbt())];
        if let Some(color) = self.color {
            fields.push(("TeamColor".into(), Nbt::String(color.serialized_name().into())));
        }
        if !self.allow_friendly_fire {
            fields.push(("AllowFriendlyFire".into(), Nbt::Byte(0)));
        }
        if !self.see_friendly_invisibles {
            fields.push(("SeeFriendlyInvisibles".into(), Nbt::Byte(0)));
        }
        if self.player_prefix != Component::empty() {
            fields.push(("MemberNamePrefix".into(), self.player_prefix.to_nbt()));
        }
        if self.player_suffix != Component::empty() {
            fields.push(("MemberNameSuffix".into(), self.player_suffix.to_nbt()));
        }
        if self.name_tag_visibility != Visibility::Always {
            fields.push(("NameTagVisibility".into(), Nbt::String(self.name_tag_visibility.name().into())));
        }
        if self.death_message_visibility != Visibility::Always {
            fields.push(("DeathMessageVisibility".into(), Nbt::String(self.death_message_visibility.name().into())));
        }
        if self.collision_rule != CollisionRule::Always {
            fields.push(("CollisionRule".into(), Nbt::String(self.collision_rule.name().into())));
        }
        if !self.players.is_empty() {
            fields.push(("Players".into(), Nbt::List(self.players.iter().map(|p| Nbt::String(p.into())).collect())));
        }
        Nbt::Compound(fields)
    }
}

pub struct PackedTeam {
    pub name: String,
    pub display_name: Option<Component>,
    pub color: Option<TeamColor>,
    pub allow_friendly_fire: bool,
    pub see_friendly_invisibles: bool,
    pub member_name_prefix: Component,
    pub member_name_suffix: Component,
    pub name_tag_visibility: Visibility,
    pub death_message_visibility: Visibility,
    pub collision_rule: CollisionRule,
    pub players: Vec<String>,
}

impl PackedTeam {
    pub fn from_nbt(tag: &Nbt) -> Result<PackedTeam, String> {
        use crate::network::chat::component_serialization::decode;
        let name = tag.get("Name").and_then(Nbt::as_str).ok_or("No key Name")?.to_string();
        let component = |key: &str| tag.get(key).map(decode).transpose();
        let flag = |key: &str| tag.get(key).and_then(Nbt::as_f64).is_none_or(|v| v != 0.0);
        let visibility = |key: &str| tag.get(key).and_then(Nbt::as_str).map_or(Ok(Visibility::Always), |v| Visibility::by_name(v).ok_or(format!("Unknown element name:{v}")));
        Ok(PackedTeam {
            name,
            display_name: component("DisplayName")?,
            color: tag.get("TeamColor").and_then(Nbt::as_str).and_then(TeamColor::by_name),
            allow_friendly_fire: flag("AllowFriendlyFire"),
            see_friendly_invisibles: flag("SeeFriendlyInvisibles"),
            member_name_prefix: component("MemberNamePrefix")?.unwrap_or_else(Component::empty),
            member_name_suffix: component("MemberNameSuffix")?.unwrap_or_else(Component::empty),
            name_tag_visibility: visibility("NameTagVisibility")?,
            death_message_visibility: visibility("DeathMessageVisibility")?,
            collision_rule: tag.get("CollisionRule").and_then(Nbt::as_str).and_then(CollisionRule::by_name).unwrap_or(CollisionRule::Always),
            players: tag.get("Players").map(|p| p.as_list().iter().filter_map(|n| n.as_str().map(String::from)).collect()).unwrap_or_default(),
        })
    }
}
