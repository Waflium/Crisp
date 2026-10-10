use crisp_data::{BlockState, DeathMessageType, block_tags, blocks, damage_type_tags};

use super::DamageSource;
use crate::network::chat::{ClickEvent, Component, HoverEvent};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

const RESET_DAMAGE_STATUS_TIME: i32 = 100;
const RESET_COMBAT_STATUS_TIME: i32 = 300;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FallLocation {
    Generic,
    Ladder,
    Vines,
    WeepingVines,
    TwistingVines,
    Scaffolding,
    OtherClimbable,
    Water,
}

impl FallLocation {
    pub fn block_to_fall_location(state: BlockState) -> FallLocation {
        if state.is_block(blocks::LADDER) || state.is(&block_tags::TRAPDOORS) {
            FallLocation::Ladder
        } else if state.is_block(blocks::VINE) {
            FallLocation::Vines
        } else if state.is_block(blocks::WEEPING_VINES) || state.is_block(blocks::WEEPING_VINES_PLANT) {
            FallLocation::WeepingVines
        } else if state.is_block(blocks::TWISTING_VINES) || state.is_block(blocks::TWISTING_VINES_PLANT) {
            FallLocation::TwistingVines
        } else if state.is_block(blocks::SCAFFOLDING) {
            FallLocation::Scaffolding
        } else {
            FallLocation::OtherClimbable
        }
    }

    fn id(self) -> &'static str {
        match self {
            FallLocation::Generic => "generic",
            FallLocation::Ladder => "ladder",
            FallLocation::Vines => "vines",
            FallLocation::WeepingVines => "weeping_vines",
            FallLocation::TwistingVines => "twisting_vines",
            FallLocation::Scaffolding => "scaffolding",
            FallLocation::OtherClimbable => "other_climbable",
            FallLocation::Water => "water",
        }
    }

    pub fn language_key(self) -> String {
        format!("death.fell.accident.{}", self.id())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CombatEntry {
    pub source: DamageSource,
    pub damage: f32,
    pub fall_location: Option<FallLocation>,
    pub fall_distance: f32,
}

#[derive(Default)]
pub struct CombatTracker {
    pub entries: Vec<CombatEntry>,
    last_damage_time: i32,
    combat_start_time: i32,
    combat_end_time: i32,
    pub in_combat: bool,
    taking_damage: bool,
}

impl CombatTracker {
    pub fn combat_duration(&self, tick_count: i32) -> i32 {
        if self.in_combat { tick_count - self.combat_start_time } else { self.combat_end_time - self.combat_start_time }
    }
}

impl ServerLevel {
    fn current_fall_location(&mut self, id: EntityId) -> Option<FallLocation> {
        match self.living(id).last_climbable_pos {
            Some(pos) => {
                let state = self.block_state(pos);
                Some(FallLocation::block_to_fall_location(state))
            }
            None => self.entity(id).is_in_water().then_some(FallLocation::Water),
        }
    }

    pub fn record_damage(&mut self, id: EntityId, source: DamageSource, damage: f32) {
        self.recheck_combat_status(id);
        let fall_location = self.current_fall_location(id);
        let alive = self.is_alive(id);
        let e = self.entity(id);
        let entry = CombatEntry { source, damage, fall_location, fall_distance: e.fall_distance as f32 };
        let tick_count = e.tick_count;
        let tracker = &mut self.living_mut(id).combat_tracker;
        tracker.entries.push(entry);
        tracker.last_damage_time = tick_count;
        tracker.taking_damage = true;
        let enters = source.causing_entity.is_some_and(|e| self.is_living(e));
        let tracker = &mut self.living_mut(id).combat_tracker;
        if !tracker.in_combat && alive && enters {
            tracker.in_combat = true;
            tracker.combat_start_time = tick_count;
            tracker.combat_end_time = tick_count;
            self.on_enter_combat(id);
        }
    }

    pub fn recheck_combat_status(&mut self, id: EntityId) {
        let alive = self.is_alive(id);
        let tick_count = self.entity(id).tick_count;
        let tracker = &mut self.living_mut(id).combat_tracker;
        let reset = if tracker.in_combat { RESET_COMBAT_STATUS_TIME } else { RESET_DAMAGE_STATUS_TIME };
        if tracker.taking_damage && (!alive || tick_count - tracker.last_damage_time > reset) {
            let was_in_combat = tracker.in_combat;
            tracker.taking_damage = false;
            tracker.in_combat = false;
            tracker.combat_end_time = tick_count;
            if was_in_combat {
                self.on_leave_combat(id);
            }
            self.living_mut(id).combat_tracker.entries.clear();
        }
    }

    fn on_enter_combat(&self, id: EntityId) {
        use crisp_data::packets::play::clientbound;
        let Some(p) = self.players.get(&id) else { return };
        p.connection.send(clientbound::PLAYER_COMBAT_ENTER, |_| {});
    }

    fn on_leave_combat(&self, id: EntityId) {
        use crisp_data::packets::play::clientbound;
        let Some(p) = self.players.get(&id) else { return };
        let duration = p.living.combat_tracker.combat_duration(p.entity.tick_count);
        p.connection.send(clientbound::PLAYER_COMBAT_END, |w| w.varint(duration));
    }

    fn message_for_assisted_fall(&self, id: EntityId, attacker_name: Component, without_item: &str) -> Component {
        Component::translatable_with(without_item, vec![self.display_name(id).into(), attacker_name.into()])
    }

    fn fall_message(&self, id: EntityId, knock_off: &CombatEntry, killing_entity: Option<EntityId>) -> Component {
        let source = &knock_off.source;
        if !source.is(&damage_type_tags::IS_FALL) && !source.is(&damage_type_tags::ALWAYS_MOST_SIGNIFICANT_FALL) {
            let killer_name = killing_entity.map(|e| self.display_name(e));
            let attacker_name = source.causing_entity.map(|e| self.display_name(e));
            match attacker_name {
                Some(attacker) if Some(&attacker) != killer_name.as_ref() => self.message_for_assisted_fall(id, attacker, "death.fell.assist"),
                _ => match killer_name {
                    Some(killer) => self.message_for_assisted_fall(id, killer, "death.fell.finish"),
                    None => Component::translatable_with("death.fell.killer", vec![self.display_name(id).into()]),
                },
            }
        } else {
            let location = knock_off.fall_location.unwrap_or(FallLocation::Generic);
            Component::translatable_with(&location.language_key(), vec![self.display_name(id).into()])
        }
    }

    pub fn death_message(&self, id: EntityId) -> Component {
        let entries = &self.living(id).combat_tracker.entries;
        let Some(killing_blow) = entries.last() else {
            return Component::translatable_with("death.attack.generic", vec![self.display_name(id).into()]);
        };
        let killing_source = killing_blow.source;
        let knock_off = most_significant_fall(entries);
        match (killing_source.ty.info().death_message_type, knock_off) {
            (DeathMessageType::FallVariants, Some(knock_off)) => self.fall_message(id, &knock_off, killing_source.causing_entity),
            (DeathMessageType::IntentionalGameDesign, _) => {
                let death_msg = format!("death.attack.{}", killing_source.msg_id());
                let link = Component::translatable_with("chat.square_brackets", vec![Component::translatable(&format!("{death_msg}.link")).into()])
                    .with_click(ClickEvent::OpenUrl("https://bugs.mojang.com/browse/MCPE-28723".into()))
                    .with_hover(HoverEvent::ShowText(Box::new(Component::literal("MCPE-28723"))));
                Component::translatable_with(&format!("{death_msg}.message"), vec![self.display_name(id).into(), link.into()])
            }
            _ => killing_source.localized_death_message(self, id),
        }
    }
}

fn most_significant_fall(entries: &[CombatEntry]) -> Option<CombatEntry> {
    let mut result: Option<CombatEntry> = None;
    let mut alternative: Option<CombatEntry> = None;
    let mut alt_damage = 0.0f32;
    let mut best_fall = 0.0f32;
    for (i, entry) in entries.iter().enumerate() {
        let previous = if i > 0 { Some(entries[i - 1]) } else { None };
        let source = &entry.source;
        let is_fake_fall = source.is(&damage_type_tags::ALWAYS_MOST_SIGNIFICANT_FALL);
        let fall_distance = if is_fake_fall { f32::MAX } else { entry.fall_distance };
        if (source.is(&damage_type_tags::IS_FALL) || is_fake_fall) && fall_distance > 0.0 && (result.is_none() || fall_distance > best_fall) {
            result = if i > 0 { previous } else { Some(*entry) };
            best_fall = fall_distance;
        }
        if entry.fall_location.is_some() && (alternative.is_none() || entry.damage > alt_damage) {
            alternative = Some(*entry);
            alt_damage = entry.damage;
        }
    }
    if best_fall > 5.0 && result.is_some() {
        result
    } else if alt_damage > 5.0 && alternative.is_some() {
        alternative
    } else {
        None
    }
}
