use crisp_data::EquipmentSlot;

use super::EntityId;
use super::conversion_params::ConversionParams;
use super::data::{DataValue, ids};
use super::flags;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConversionType {
    Single,
}

impl ConversionType {
    pub fn should_discard_after_conversion(self) -> bool {
        self == ConversionType::Single
    }

    pub fn convert(self, level: &mut ServerLevel, from: EntityId, to: EntityId, params: &ConversionParams) {
        if self == ConversionType::Single {
            let e = level.entity(from);
            let (pos, y_rot, x_rot, delta_movement) = (e.position(), e.y_rot, e.x_rot, e.delta_movement);
            let to_entity = level.entity_mut(to);
            to_entity.snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
            to_entity.set_delta_movement(delta_movement);
            if params.keep_equipment {
                for slot in EquipmentSlot::VALUES {
                    let Some(item_stack) = level.item_by_slot(from, slot) else { continue };
                    level.set_item_slot(from, slot, ItemStack::empty());
                    level.living_set_item_slot(to, slot, item_stack);
                    let drop_chance = level.mob(from).expect("a mob").drop_chances[slot as usize];
                    level.mob_mut(to).expect("a mob").drop_chances[slot as usize] = drop_chance;
                }
            }
            let from_mob = level.mob(from).expect("a mob");
            let (fall_distance, fall_flying) = (from_mob.entity.fall_distance, from_mob.entity.is_fall_flying());
            let (memory_time, hurt_time, y_body_rot) = (from_mob.living.last_hurt_by_player_memory_time, from_mob.living.hurt_time, from_mob.living.y_body_rot);
            let (on_ground, sleeping_pos) = (from_mob.entity.on_ground, from_mob.entity.data.get(ids::SLEEPING_POS).clone());
            let to_mob = level.mob_mut(to).expect("a mob");
            to_mob.entity.fall_distance = fall_distance;
            to_mob.entity.set_shared_flag(flags::FALL_FLYING, fall_flying);
            to_mob.living.last_hurt_by_player_memory_time = memory_time;
            to_mob.living.hurt_time = hurt_time;
            to_mob.living.y_body_rot = y_body_rot;
            level.set_on_ground(to, on_ground);
            if let DataValue::OptionalBlockPos(Some(_)) = sleeping_pos {
                level.entity_mut(to).data.set(ids::SLEEPING_POS, sleeping_pos);
            }
        }
        convert_common(level, from, to, params);
    }
}

fn convert_common(level: &mut ServerLevel, from: EntityId, to: EntityId, params: &ConversionParams) {
    let absorption = level.mob(from).expect("a mob").absorption;
    level.set_absorption_amount(to, absorption);
    for effect in level.mob(from).expect("a mob").living.active_effects.clone() {
        level.add_effect(to, effect, None);
    }
    if level.is_baby(from) {
        level.mob_mut(to).expect("a mob").set_baby(true);
    }
    let from_ageable = level.mob(from).and_then(|mob| mob.ageable()).map(|ageable| (ageable.age, ageable.forced_age, ageable.forced_age_timer));
    if let Some((age, forced_age, forced_age_timer)) = from_ageable
        && level.mob(to).is_some_and(|mob| mob.ageable().is_some())
    {
        level.set_age(to, age);
        let ageable = level.mob_mut(to).expect("a mob").ageable_mut().expect("an ageable mob");
        ageable.forced_age = forced_age;
        ageable.forced_age_timer = forced_age_timer;
    }
    let from_mob = level.mob(from).expect("a mob");
    let (can_pick_up_loot, left_handed, no_ai, persistence_required) = (from_mob.can_pick_up_loot, from_mob.is_left_handed(), from_mob.is_no_ai(), from_mob.persistence_required);
    let e = &from_mob.entity;
    let custom_name_visible = e.data.get(ids::CUSTOM_NAME_VISIBLE).clone();
    let (invulnerable, invulnerable_time, no_gravity, portal_cooldown) = (e.invulnerable, e.invulnerable_time, e.data.get(ids::NO_GRAVITY).clone(), e.portal_cooldown);
    let (silent, tags, custom_name, custom_data) = (e.data.get(ids::SILENT).clone(), e.tags.clone(), e.data.get(ids::CUSTOM_NAME).clone(), e.custom_data.clone());
    let on_fire = level.is_on_fire(from);
    let to_mob = level.mob_mut(to).expect("a mob");
    if params.preserve_can_pick_up_loot {
        to_mob.can_pick_up_loot = can_pick_up_loot;
    }
    to_mob.set_left_handed(left_handed);
    to_mob.set_no_ai(no_ai);
    if persistence_required {
        to_mob.persistence_required = true;
    }
    let e = &mut to_mob.entity;
    e.data.set(ids::CUSTOM_NAME_VISIBLE, custom_name_visible);
    e.set_shared_flag(flags::ON_FIRE, on_fire);
    e.invulnerable = invulnerable;
    e.invulnerable_time = invulnerable_time;
    e.data.set(ids::NO_GRAVITY, no_gravity);
    e.portal_cooldown = portal_cooldown;
    e.data.set(ids::SILENT, silent);
    for tag in &tags {
        e.add_tag(tag);
    }
    if let DataValue::OptionalComponent(Some(name)) = custom_name {
        e.apply_implicit_component(dc::CUSTOM_NAME, &name);
    }
    if let Some(custom_data) = custom_data {
        e.apply_implicit_component(dc::CUSTOM_DATA, &custom_data);
    }
    if let Some(team) = &params.team {
        let to_name = level.scoreboard_name(to);
        level.scoreboard.borrow_mut().add_player_to_team(&to_name, team);
        if level.team_of(from).is_some_and(|from_team| from_team.name() == team) {
            let from_name = level.scoreboard_name(from);
            level.scoreboard.borrow_mut().remove_player_from_team(&from_name, team);
        }
    }
}
