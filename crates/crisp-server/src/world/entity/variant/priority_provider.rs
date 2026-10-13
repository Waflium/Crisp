use crisp_core::random::RandomSource;

use super::biome_check::BiomeCheck;

pub struct Selector {
    pub condition: Option<BiomeCheck>,
    pub priority: i32,
}

pub fn select<T: Copy>(entries: &[(T, Selector)], biome: i32) -> Vec<T> {
    let mut unpacked_entries: Vec<&(T, Selector)> = entries.iter().collect();
    unpacked_entries.sort_by_key(|(_, selector)| std::cmp::Reverse(selector.priority));
    let mut highest_matched_priority = i32::MIN;
    unpacked_entries.retain(|(_, selector)| {
        if selector.priority < highest_matched_priority {
            false
        } else if selector.condition.as_ref().is_none_or(|condition| condition.test(biome)) {
            highest_matched_priority = selector.priority;
            true
        } else {
            false
        }
    });
    unpacked_entries.into_iter().map(|(entry, _)| *entry).collect()
}

pub fn pick<T: Copy>(entries: &[(T, Selector)], random: &mut impl RandomSource, biome: i32) -> Option<T> {
    let selected = select(entries, biome);
    if selected.is_empty() {
        return None;
    }
    Some(selected[random.next_int_bounded(selected.len() as i32) as usize])
}
