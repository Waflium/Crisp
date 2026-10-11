use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;

pub type Selector<'a> = &'a dyn Fn(EntityId, &ServerLevel) -> bool;

#[derive(Clone, Copy)]
pub struct TargetingConditions<'a> {
    is_combat: bool,
    range: f64,
    check_line_of_sight: bool,
    test_invisible: bool,
    selector: Option<Selector<'a>>,
}

impl<'a> TargetingConditions<'a> {
    const fn new(is_combat: bool) -> TargetingConditions<'a> {
        TargetingConditions { is_combat, range: -1.0, check_line_of_sight: true, test_invisible: true, selector: None }
    }

    pub const fn for_combat() -> TargetingConditions<'a> {
        TargetingConditions::new(true)
    }

    pub const fn for_non_combat() -> TargetingConditions<'a> {
        TargetingConditions::new(false)
    }

    pub const fn range(mut self, range: f64) -> TargetingConditions<'a> {
        self.range = range;
        self
    }

    pub const fn ignore_line_of_sight(mut self) -> TargetingConditions<'a> {
        self.check_line_of_sight = false;
        self
    }

    pub const fn ignore_invisibility_testing(mut self) -> TargetingConditions<'a> {
        self.test_invisible = false;
        self
    }

    pub fn selector<'b>(self, selector: Selector<'b>) -> TargetingConditions<'b>
    where
        'a: 'b,
    {
        TargetingConditions { selector: Some(selector), ..self }
    }

    pub fn test(&self, level: &mut ServerLevel, targeter: Option<EntityId>, target: EntityId) -> bool {
        if targeter == Some(target) {
            return false;
        }
        if !level.can_be_seen_by_anyone(target) {
            return false;
        }
        if let Some(selector) = self.selector
            && !selector(target, level)
        {
            return false;
        }
        match targeter {
            None => {
                if self.is_combat && (!level.can_be_seen_as_enemy(target) || level.difficulty == Difficulty::Peaceful) {
                    return false;
                }
            }
            Some(targeter) => {
                if self.is_combat && (!level.can_attack(targeter, target) || level.is_allied_to(targeter, target)) {
                    return false;
                }
                if self.range > 0.0 {
                    let modifier = if self.test_invisible { level.get_visibility_percent(target, Some(targeter)) } else { 1.0 };
                    let visibility_distance = (self.range * modifier).max(2.0);
                    let t = level.entity(target);
                    let distance_to_sqr = level.entity(targeter).distance_to_sqr_xyz(t.x(), t.y(), t.z());
                    if distance_to_sqr > visibility_distance * visibility_distance {
                        return false;
                    }
                }
                if self.check_line_of_sight && level.is_mob(targeter) && !level.sensing_has_line_of_sight(targeter, target) {
                    return false;
                }
            }
        }
        true
    }
}
