use indexmap::IndexMap;

use super::stat::Stat;

pub trait StatsCounter {
    fn stats(&self) -> &IndexMap<Stat, i32>;

    fn set_value(&mut self, stat: Stat, count: i32);

    fn increment(&mut self, stat: Stat, count: i32) {
        let result = (self.get_value(stat) as i64 + count as i64).min(i32::MAX as i64) as i32;
        self.set_value(stat, result);
    }

    fn get_value(&self, stat: Stat) -> i32 {
        self.stats().get(&stat).copied().unwrap_or(0)
    }
}
