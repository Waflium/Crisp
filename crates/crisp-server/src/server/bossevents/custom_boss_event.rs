use crisp_core::mth;
use crisp_protocol::nbt::Nbt;

use crate::network::chat::{Component, HoverEvent};
use crate::server::level::ServerLevel;
use crate::server::level::server_boss_event::{BossBarColor, BossBarOverlay, ServerBossEvent};
use crate::world::entity::EntityId;
use crate::world::entity::entity_save::uuid_from_nbt;
use crate::world::level::storage::uuid_ints;

const DEFAULT_MAX: i32 = 100;

fn player_in(levels: &[ServerLevel], player: EntityId) -> &crate::server::level::server_player::ServerPlayer {
    levels.iter().find_map(|level| level.players.get(&player)).expect("player on this server")
}

pub struct CustomBossEvent {
    pub event: ServerBossEvent,
    pub custom_id: String,
    pub players: Vec<u128>,
    pub value: i32,
    pub max: i32,
    pub dirty: bool,
}

impl CustomBossEvent {
    pub fn new(id: u128, custom_id: String, name: Component) -> CustomBossEvent {
        let mut event = ServerBossEvent::new(id, name, BossBarColor::White, BossBarOverlay::Progress);
        event.progress = 0.0;
        CustomBossEvent { event, custom_id, players: Vec::new(), value: 0, max: DEFAULT_MAX, dirty: false }
    }

    pub fn add_player(&mut self, levels: &[ServerLevel], player: EntityId) {
        self.event.add_player(levels, player);
        let uuid = player_in(levels, player).profile.uuid;
        if !self.players.contains(&uuid) {
            self.players.push(uuid);
            self.dirty = true;
        }
    }

    pub fn remove_player(&mut self, levels: &[ServerLevel], player: EntityId) {
        self.event.remove_player(levels, player);
        let uuid = player_in(levels, player).profile.uuid;
        if let Some(i) = self.players.iter().position(|&p| p == uuid) {
            self.players.remove(i);
            self.dirty = true;
        }
    }

    pub fn remove_all_players(&mut self, levels: &[ServerLevel]) {
        for player in self.event.players.clone() {
            self.event.remove_player(levels, player);
        }
        if !self.players.is_empty() {
            self.players.clear();
            self.dirty = true;
        }
    }

    pub fn set_value(&mut self, levels: &[ServerLevel], value: i32) {
        self.value = value;
        self.event.set_progress(levels, mth::clamp_f(value as f32 / self.max as f32, 0.0, 1.0));
        self.dirty = true;
    }

    pub fn set_max(&mut self, levels: &[ServerLevel], max: i32) {
        self.max = max;
        self.event.set_progress(levels, mth::clamp_f(self.value as f32 / max as f32, 0.0, 1.0));
        self.dirty = true;
    }

    pub fn display_name(&self) -> Component {
        Component::translatable_with("chat.square_brackets", vec![self.event.name.clone().into()])
            .with_color(self.event.color.formatting())
            .with_hover(HoverEvent::ShowText(Box::new(Component::literal(self.custom_id.clone()))))
            .with_insertion(&self.custom_id)
    }

    pub fn set_players(&mut self, levels: &[ServerLevel], players: &[EntityId]) -> bool {
        let uuids: Vec<(EntityId, u128)> = players.iter().map(|&p| (p, player_in(levels, p).profile.uuid)).collect();
        let to_remove: Vec<u128> = self.players.iter().copied().filter(|uuid| !uuids.iter().any(|(_, u)| u == uuid)).collect();
        let to_add: Vec<EntityId> = uuids.iter().filter(|(_, u)| !self.players.contains(u)).map(|&(p, _)| p).collect();
        for &uuid in &to_remove {
            if let Some(&player) = self.event.players.iter().find(|&&p| player_in(levels, p).profile.uuid == uuid) {
                self.remove_player(levels, player);
            }
            self.players.retain(|&p| p != uuid);
        }
        for &player in &to_add {
            self.add_player(levels, player);
        }
        let changed = !to_remove.is_empty() || !to_add.is_empty();
        if changed {
            self.dirty = true;
        }
        changed
    }

    pub fn on_player_connect(&mut self, levels: &[ServerLevel], player: EntityId) {
        if self.players.contains(&player_in(levels, player).profile.uuid) {
            self.add_player(levels, player);
        }
    }

    pub fn on_player_disconnect(&mut self, levels: &[ServerLevel], player: EntityId) {
        self.event.remove_player(levels, player);
    }

    pub fn load(id: u128, custom_id: String, packed: &Nbt) -> Option<CustomBossEvent> {
        let flag = |key: &str| packed.get(key).map_or(Some(false), |v| v.as_i64().map(|v| v != 0));
        let int = |key: &str, default: i32| packed.get(key).map_or(Some(default), |v| v.as_i64().map(|v| v as i32));
        let mut custom = CustomBossEvent::new(id, custom_id, Component::from_nbt(packed.get("Name")?));
        custom.value = int("Value", 0)?;
        custom.max = int("Max", DEFAULT_MAX)?;
        let event = &mut custom.event;
        event.visible = flag("Visible")?;
        event.progress = mth::clamp_f(custom.value as f32 / custom.max as f32, 0.0, 1.0);
        event.color = match packed.get("Color") {
            Some(color) => BossBarColor::by_name(color.as_str()?)?,
            None => BossBarColor::White,
        };
        event.overlay = match packed.get("Overlay") {
            Some(overlay) => BossBarOverlay::by_name(overlay.as_str()?)?,
            None => BossBarOverlay::Progress,
        };
        event.darken_screen = flag("DarkenScreen")?;
        event.play_boss_music = flag("PlayBossMusic")?;
        event.create_world_fog = flag("CreateWorldFog")?;
        for uuid in packed.get("Players").map_or(&[][..], Nbt::as_list) {
            let uuid = uuid_from_nbt(uuid)?;
            if !custom.players.contains(&uuid) {
                custom.players.push(uuid);
            }
        }
        Some(custom)
    }

    pub fn pack(&self) -> Nbt {
        let e = &self.event;
        let mut fields = vec![("Name".to_string(), e.name.to_nbt())];
        let defaults = [
            ("Visible", e.visible.then_some(Nbt::Byte(1))),
            ("Value", (self.value != 0).then_some(Nbt::Int(self.value))),
            ("Max", (self.max != DEFAULT_MAX).then_some(Nbt::Int(self.max))),
            ("Color", (e.color != BossBarColor::White).then(|| Nbt::String(e.color.name().into()))),
            ("Overlay", (e.overlay != BossBarOverlay::Progress).then(|| Nbt::String(e.overlay.name().into()))),
            ("DarkenScreen", e.darken_screen.then_some(Nbt::Byte(1))),
            ("PlayBossMusic", e.play_boss_music.then_some(Nbt::Byte(1))),
            ("CreateWorldFog", e.create_world_fog.then_some(Nbt::Byte(1))),
            ("Players", (!self.players.is_empty()).then(|| Nbt::List(self.players.iter().map(|&u| uuid_ints(u)).collect()))),
        ];
        fields.extend(defaults.into_iter().filter_map(|(key, value)| Some((key.to_string(), value?))));
        Nbt::Compound(fields)
    }
}
