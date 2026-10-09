use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InsideBlockEffectType {
    Freeze,
    ClearFreeze,
    FireIgnite,
    LavaIgnite,
    Extinguish,
}

impl InsideBlockEffectType {
    pub const VALUES: [InsideBlockEffectType; 5] = [
        InsideBlockEffectType::Freeze,
        InsideBlockEffectType::ClearFreeze,
        InsideBlockEffectType::FireIgnite,
        InsideBlockEffectType::LavaIgnite,
        InsideBlockEffectType::Extinguish,
    ];
}

pub type EntityEffect = Box<dyn FnOnce(&mut ServerLevel, EntityId) + Send>;

pub enum QueuedEffect {
    Type(InsideBlockEffectType),
    Callback(EntityEffect),
}

#[derive(Default)]
pub struct InsideBlockEffects {
    effects_in_step: Vec<InsideBlockEffectType>,
    before: Vec<(InsideBlockEffectType, EntityEffect)>,
    after: Vec<(InsideBlockEffectType, EntityEffect)>,
    final_effects: Vec<QueuedEffect>,
    last_step: Option<i32>,
}

impl InsideBlockEffects {
    pub fn apply(&mut self, ty: InsideBlockEffectType) {
        if !self.effects_in_step.contains(&ty) {
            self.effects_in_step.push(ty);
        }
    }

    pub fn run_before(&mut self, ty: InsideBlockEffectType, effect: EntityEffect) {
        self.before.push((ty, effect));
    }

    pub fn run_after(&mut self, ty: InsideBlockEffectType, effect: EntityEffect) {
        self.after.push((ty, effect));
    }

    pub fn advance_step(&mut self, step: i32) {
        if self.last_step != Some(step) {
            self.last_step = Some(step);
            self.flush_step();
        }
    }

    fn flush_step(&mut self) {
        for ty in InsideBlockEffectType::VALUES {
            for (_, effect) in extract(&mut self.before, ty) {
                self.final_effects.push(QueuedEffect::Callback(effect));
            }
            if let Some(i) = self.effects_in_step.iter().position(|&t| t == ty) {
                self.effects_in_step.remove(i);
                self.final_effects.push(QueuedEffect::Type(ty));
            }
            for (_, effect) in extract(&mut self.after, ty) {
                self.final_effects.push(QueuedEffect::Callback(effect));
            }
        }
    }

    pub fn take_final(&mut self) -> Vec<QueuedEffect> {
        self.flush_step();
        self.last_step = None;
        std::mem::take(&mut self.final_effects)
    }
}

fn extract(list: &mut Vec<(InsideBlockEffectType, EntityEffect)>, ty: InsideBlockEffectType) -> Vec<(InsideBlockEffectType, EntityEffect)> {
    let mut taken = Vec::new();
    let mut i = 0;
    while i < list.len() {
        if list[i].0 == ty {
            taken.push(list.remove(i));
        } else {
            i += 1;
        }
    }
    taken
}
