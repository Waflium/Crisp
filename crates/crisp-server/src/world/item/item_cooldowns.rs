use crisp_data::packets::play::clientbound;
use crisp_protocol::nbt::Nbt;

use super::ItemStack;
use super::component as dc;
use crate::network::connection::Outbound;
use crate::util::java_collections::{JavaHashMap, identifier_hash};

#[derive(Clone, Copy)]
struct CooldownInstance {
    start_time: i32,
    end_time: i32,
}

pub struct ItemCooldowns {
    cooldowns: JavaHashMap<CooldownInstance>,
    tick_count: i32,
}

impl Default for ItemCooldowns {
    fn default() -> ItemCooldowns {
        ItemCooldowns { cooldowns: JavaHashMap::new(identifier_hash), tick_count: 0 }
    }
}

impl ItemCooldowns {
    pub fn is_on_cooldown(&self, item: &ItemStack) -> bool {
        self.get_cooldown_percent(item, 0.0) > 0.0
    }

    pub fn get_cooldown_percent(&self, item: &ItemStack, a: f32) -> f32 {
        let group = get_cooldown_group(item);
        match self.cooldowns.get(&group) {
            Some(cooldown) => {
                let duration = (cooldown.end_time - cooldown.start_time) as f32;
                let remaining = cooldown.end_time as f32 - (self.tick_count as f32 + a);
                crisp_core::mth::clamp_f(remaining / duration, 0.0, 1.0)
            }
            None => 0.0,
        }
    }

    pub fn tick(&mut self, connection: &Outbound) {
        self.tick_count += 1;
        if self.cooldowns.is_empty() {
            return;
        }
        let ended: Vec<String> = self.cooldowns.iter().filter(|(_, cooldown)| cooldown.end_time <= self.tick_count).map(|(group, _)| group.to_string()).collect();
        for group in ended {
            self.cooldowns.remove(&group);
            on_cooldown_ended(connection, &group);
        }
    }

    pub fn add_cooldown(&mut self, item: &ItemStack, time: i32, connection: &Outbound) {
        self.add_cooldown_group(get_cooldown_group(item), time, connection);
    }

    pub fn add_cooldown_group(&mut self, cooldown_group: String, time: i32, connection: &Outbound) {
        self.cooldowns.insert(cooldown_group.clone(), CooldownInstance { start_time: self.tick_count, end_time: self.tick_count + time });
        on_cooldown_started(connection, &cooldown_group, time);
    }

    pub fn remove_cooldown(&mut self, cooldown_group: &str, connection: &Outbound) {
        self.cooldowns.remove(cooldown_group);
        on_cooldown_ended(connection, cooldown_group);
    }
}

pub fn get_cooldown_group(item: &ItemStack) -> String {
    let default_item_group = item.get_item().name().to_string();
    match item.get(dc::USE_COOLDOWN) {
        Some(use_cooldown) => use_cooldown.get("cooldown_group").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier).unwrap_or(default_item_group),
        None => default_item_group,
    }
}

pub fn use_cooldown_ticks(use_cooldown: &Nbt) -> i32 {
    (use_cooldown.get("seconds").and_then(Nbt::as_f64).unwrap_or(0.0) as f32 * 20.0) as i32
}

fn on_cooldown_started(connection: &Outbound, cooldown_group: &str, duration: i32) {
    connection.send(clientbound::COOLDOWN, |w| {
        w.string(cooldown_group);
        w.varint(duration);
    });
}

fn on_cooldown_ended(connection: &Outbound, cooldown_group: &str) {
    connection.send(clientbound::COOLDOWN, |w| {
        w.string(cooldown_group);
        w.varint(0);
    });
}
