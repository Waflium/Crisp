use crisp_core::random::RandomSource;
use crisp_data::packets::play::clientbound;
use crisp_protocol::nbt::Nbt;

use crate::server::level::server_player::ServerPlayer;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::entity::data::{DataValue, ids};
use crate::world::entity::sound_event;
use crate::world::level::block::entity::{float_value, int_value};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Experience {
    pub level: i32,
    pub total: i32,
    pub progress: f32,
    pub enchantment_seed: i32,
    pub take_xp_delay: i32,
    pub last_level_up_time: i32,
}

impl Experience {
    pub fn xp_needed_for_next_level(&self) -> i32 {
        let level = self.level;
        if level >= 30 {
            112i32.wrapping_add((level - 30).wrapping_mul(9))
        } else if level >= 15 {
            37 + (level - 15) * 5
        } else {
            7 + level * 2
        }
    }

    pub fn save(&self, out: &mut Vec<(String, Nbt)>) {
        out.push(("XpP".into(), Nbt::Float(self.progress)));
        out.push(("XpLevel".into(), Nbt::Int(self.level)));
        out.push(("XpTotal".into(), Nbt::Int(self.total)));
        out.push(("XpSeed".into(), Nbt::Int(self.enchantment_seed)));
    }

    pub fn load(&mut self, nbt: &Nbt, random: &mut impl RandomSource) {
        self.progress = nbt.get("XpP").and_then(float_value).unwrap_or(0.0);
        self.level = nbt.get("XpLevel").and_then(int_value).unwrap_or(0);
        self.total = nbt.get("XpTotal").and_then(int_value).unwrap_or(0);
        self.enchantment_seed = nbt.get("XpSeed").and_then(int_value).unwrap_or(0);
        if self.enchantment_seed == 0 {
            self.enchantment_seed = random.next_int();
        }
    }
}

impl ServerPlayer {
    pub fn score(&self) -> i32 {
        match self.entity.data.get(ids::PLAYER_SCORE) {
            DataValue::Int(score) => *score,
            _ => 0,
        }
    }

    pub fn set_score(&mut self, score: i32) {
        self.entity.data.set(ids::PLAYER_SCORE, DataValue::Int(score));
    }

    pub fn send_experience(&self) {
        let xp = self.experience;
        self.connection.send(clientbound::SET_EXPERIENCE, |w| {
            w.f32(xp.progress);
            w.varint(xp.level);
            w.varint(xp.total);
        });
    }

    pub fn set_experience_points(&mut self, amount: i32) {
        let limit = self.experience.xp_needed_for_next_level() as f32;
        let max = (limit - 1.0) / limit;
        let progress = crisp_core::mth::clamp_f(amount as f32 / limit, 0.0, max);
        if progress != self.experience.progress {
            self.experience.progress = progress;
            self.last_sent_exp = -1;
        }
    }

    pub fn set_experience_levels(&mut self, amount: i32) {
        if amount != self.experience.level {
            self.experience.level = amount;
            self.last_sent_exp = -1;
        }
    }
}

impl ServerLevel {
    pub fn give_experience_points(&mut self, id: EntityId, i: i32) {
        if i == 0 {
            return;
        }
        let p = self.player_mut(id);
        p.set_score(p.score().wrapping_add(i));
        let xp = &mut p.experience;
        xp.progress += i as f32 / xp.xp_needed_for_next_level() as f32;
        xp.total = xp.total.wrapping_add(i).max(0);
        p.last_sent_exp = -1;
        while self.player(id).experience.progress < 0.0 {
            let xp = self.player(id).experience;
            let remaining = xp.progress * xp.xp_needed_for_next_level() as f32;
            self.give_experience_levels(id, -1);
            let progress = if xp.level > 0 { 1.0 + remaining / self.player(id).experience.xp_needed_for_next_level() as f32 } else { 0.0 };
            self.player_mut(id).experience.progress = progress;
        }
        while self.player(id).experience.progress >= 1.0 {
            let xp = &mut self.player_mut(id).experience;
            xp.progress = (xp.progress - 1.0) * xp.xp_needed_for_next_level() as f32;
            self.give_experience_levels(id, 1);
            let xp = &mut self.player_mut(id).experience;
            xp.progress /= xp.xp_needed_for_next_level() as f32;
        }
    }

    pub fn give_experience_levels(&mut self, id: EntityId, amount: i32) {
        if amount == 0 {
            return;
        }
        let p = self.player_mut(id);
        p.last_sent_exp = -1;
        let tick_count = p.entity.tick_count;
        let xp = &mut p.experience;
        xp.level = xp.level.saturating_add(amount);
        if xp.level < 0 {
            xp.level = 0;
            xp.progress = 0.0;
            xp.total = 0;
        }
        if amount > 0 && xp.level % 5 == 0 && (xp.last_level_up_time as f32) < tick_count as f32 - 100.0 {
            let volume = if xp.level > 30 { 1.0 } else { xp.level as f32 / 30.0 };
            xp.last_level_up_time = tick_count;
            let e = &p.entity;
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.player.levelup"), SoundSource::Players, volume * 0.75, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::Vec3;

    use crate::server::level::testing;

    #[test]
    fn points_and_levels_match_vanilla() {
        let mut level = testing::level();
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let expected: [(&str, i32, i32, u32, i32, i32, i32); 23] = [
            ("p", 5, 0, 1060559726, 5, 5, 5),
            ("p", 10, 1, 1063489080, 15, 15, 7),
            ("p", 100, 8, 1040552117, 115, 115, 3),
            ("p", 1000, 27, 1049181377, 1115, 1115, 26),
            ("p", 12345, 71, 1044774038, 13460, 13460, 92),
            ("p", -50, 71, 1035408737, 13410, 13410, 42),
            ("p", -3000, 64, 1052870620, 10410, 10410, 157),
            ("l", 7, 71, 1052870620, 10410, 10410, 181),
            ("l", 25, 96, 1052870620, 10410, 10410, 266),
            ("l", -3, 93, 1052870620, 10410, 10410, 256),
            ("s", 5, 93, 1005669320, 10410, 10410, 5),
            ("p", 1, 93, 1007732498, 10411, 10411, 6),
            ("l", -1000, 0, 0, 0, 10411, 0),
            ("p", 3, 0, 1054567863, 3, 10414, 3),
            ("p", -10, 0, 0, 0, 10404, 0),
            ("l", i32::MAX, i32::MAX, 0, 0, 10404, 0),
            ("l", 5, i32::MAX, 0, 0, 10404, 0),
            ("p", i32::MAX, i32::MAX, 872415232, i32::MAX, -2147473245, 255),
            ("l", -i32::MAX, 0, 872415232, i32::MAX, -2147473245, 0),
            ("l", 31, 31, 872415232, i32::MAX, -2147473245, 0),
            ("s", 200, 31, 872415232, i32::MAX, -2147473245, 0),
            ("s", 0, 31, 0, i32::MAX, -2147473245, 0),
            ("p", 7, 31, 1030550892, 0, -2147473238, 7),
        ];
        for (op, amount, xp_level, progress, total, score, points) in expected {
            match op {
                "p" => level.give_experience_points(id, amount),
                "l" => level.give_experience_levels(id, amount),
                _ => {
                    let p = level.player_mut(id);
                    if amount < p.experience.xp_needed_for_next_level() {
                        p.set_experience_points(amount);
                    }
                }
            }
            let p = level.player(id);
            let xp = p.experience;
            let query = crisp_core::mth::floor((xp.progress * xp.xp_needed_for_next_level() as f32) as f64);
            assert_eq!((xp.level, xp.progress.to_bits(), xp.total, p.score(), query), (xp_level, progress, total, score, points), "{op} {amount}");
        }
    }
}
