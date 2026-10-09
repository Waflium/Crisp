use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;

use super::ServerLevel;
use crate::network::chat::{ChatFormatting, Component};
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossBarColor {
    Pink,
    Blue,
    Red,
    Green,
    Yellow,
    Purple,
    White,
}

impl BossBarColor {
    pub const VALUES: [BossBarColor; 7] =
        [BossBarColor::Pink, BossBarColor::Blue, BossBarColor::Red, BossBarColor::Green, BossBarColor::Yellow, BossBarColor::Purple, BossBarColor::White];

    pub fn name(self) -> &'static str {
        match self {
            BossBarColor::Pink => "pink",
            BossBarColor::Blue => "blue",
            BossBarColor::Red => "red",
            BossBarColor::Green => "green",
            BossBarColor::Yellow => "yellow",
            BossBarColor::Purple => "purple",
            BossBarColor::White => "white",
        }
    }

    pub fn by_name(name: &str) -> Option<BossBarColor> {
        BossBarColor::VALUES.into_iter().find(|c| c.name() == name)
    }

    pub fn formatting(self) -> ChatFormatting {
        match self {
            BossBarColor::Pink => ChatFormatting::Red,
            BossBarColor::Blue => ChatFormatting::Blue,
            BossBarColor::Red => ChatFormatting::DarkRed,
            BossBarColor::Green => ChatFormatting::Green,
            BossBarColor::Yellow => ChatFormatting::Yellow,
            BossBarColor::Purple => ChatFormatting::DarkBlue,
            BossBarColor::White => ChatFormatting::White,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossBarOverlay {
    Progress,
    Notched6,
    Notched10,
    Notched12,
    Notched20,
}

impl BossBarOverlay {
    pub const VALUES: [BossBarOverlay; 5] =
        [BossBarOverlay::Progress, BossBarOverlay::Notched6, BossBarOverlay::Notched10, BossBarOverlay::Notched12, BossBarOverlay::Notched20];

    pub fn name(self) -> &'static str {
        match self {
            BossBarOverlay::Progress => "progress",
            BossBarOverlay::Notched6 => "notched_6",
            BossBarOverlay::Notched10 => "notched_10",
            BossBarOverlay::Notched12 => "notched_12",
            BossBarOverlay::Notched20 => "notched_20",
        }
    }

    pub fn by_name(name: &str) -> Option<BossBarOverlay> {
        BossBarOverlay::VALUES.into_iter().find(|o| o.name() == name)
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Add,
    Remove,
    UpdateProgress,
    UpdateName,
    UpdateStyle,
    UpdateProperties,
}

pub struct ServerBossEvent {
    pub id: u128,
    pub name: Component,
    pub progress: f32,
    pub color: BossBarColor,
    pub overlay: BossBarOverlay,
    pub darken_screen: bool,
    pub play_boss_music: bool,
    pub create_world_fog: bool,
    pub players: Vec<EntityId>,
    pub visible: bool,
}

impl ServerBossEvent {
    pub fn new(id: u128, name: Component, color: BossBarColor, overlay: BossBarOverlay) -> ServerBossEvent {
        ServerBossEvent {
            id,
            name,
            progress: 1.0,
            color,
            overlay,
            darken_screen: false,
            play_boss_music: false,
            create_world_fog: false,
            players: Vec::new(),
            visible: true,
        }
    }

    fn write(&self, w: &mut Writer, operation: Operation) {
        w.uuid(self.id);
        w.varint(operation as i32);
        let properties = |w: &mut Writer| w.u8(self.darken_screen as u8 | (self.play_boss_music as u8) << 1 | (self.create_world_fog as u8) << 2);
        match operation {
            Operation::Add => {
                self.name.write(w);
                w.f32(self.progress);
                w.varint(self.color as i32);
                w.varint(self.overlay as i32);
                properties(w);
            }
            Operation::Remove => {}
            Operation::UpdateProgress => w.f32(self.progress),
            Operation::UpdateName => self.name.write(w),
            Operation::UpdateStyle => {
                w.varint(self.color as i32);
                w.varint(self.overlay as i32);
            }
            Operation::UpdateProperties => properties(w),
        }
    }

    fn send(&self, level: &ServerLevel, player: EntityId, operation: Operation) {
        if let Some(player) = level.players.get(&player) {
            player.connection.send(clientbound::BOSS_EVENT, |w| self.write(w, operation));
        }
    }

    fn broadcast(&self, level: &ServerLevel, operation: Operation) {
        if self.visible {
            for &player in &self.players {
                self.send(level, player, operation);
            }
        }
    }

    pub fn set_progress(&mut self, level: &ServerLevel, progress: f32) -> bool {
        if progress == self.progress {
            return false;
        }
        self.progress = progress;
        self.broadcast(level, Operation::UpdateProgress);
        true
    }

    pub fn set_color(&mut self, level: &ServerLevel, color: BossBarColor) -> bool {
        if color == self.color {
            return false;
        }
        self.color = color;
        self.broadcast(level, Operation::UpdateStyle);
        true
    }

    pub fn set_overlay(&mut self, level: &ServerLevel, overlay: BossBarOverlay) -> bool {
        if overlay == self.overlay {
            return false;
        }
        self.overlay = overlay;
        self.broadcast(level, Operation::UpdateStyle);
        true
    }

    pub fn set_darken_screen(&mut self, level: &ServerLevel, darken_screen: bool) -> bool {
        if darken_screen == self.darken_screen {
            return false;
        }
        self.darken_screen = darken_screen;
        self.broadcast(level, Operation::UpdateProperties);
        true
    }

    pub fn set_play_boss_music(&mut self, level: &ServerLevel, play_boss_music: bool) -> bool {
        if play_boss_music == self.play_boss_music {
            return false;
        }
        self.play_boss_music = play_boss_music;
        self.broadcast(level, Operation::UpdateProperties);
        true
    }

    pub fn set_create_world_fog(&mut self, level: &ServerLevel, create_world_fog: bool) -> bool {
        if create_world_fog == self.create_world_fog {
            return false;
        }
        self.create_world_fog = create_world_fog;
        self.broadcast(level, Operation::UpdateProperties);
        true
    }

    pub fn set_name(&mut self, level: &ServerLevel, name: Component) -> bool {
        if name == self.name {
            return false;
        }
        self.name = name;
        self.broadcast(level, Operation::UpdateName);
        true
    }

    pub fn add_player(&mut self, level: &ServerLevel, player: EntityId) {
        if !self.players.contains(&player) {
            self.players.push(player);
            if self.visible {
                self.send(level, player, Operation::Add);
            }
        }
    }

    pub fn remove_player(&mut self, level: &ServerLevel, player: EntityId) {
        if let Some(i) = self.players.iter().position(|&p| p == player) {
            self.players.remove(i);
            if self.visible {
                self.send(level, player, Operation::Remove);
            }
        }
    }

    pub fn set_visible(&mut self, level: &ServerLevel, visible: bool) -> bool {
        if visible == self.visible {
            return false;
        }
        self.visible = visible;
        for &player in &self.players {
            self.send(level, player, if visible { Operation::Add } else { Operation::Remove });
        }
        true
    }
}
