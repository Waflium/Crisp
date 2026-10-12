use crisp_core::random::{LegacyRandomSource, RandomSource, XoroshiroRandomSource};
use crisp_protocol::nbt::Nbt;

use super::loot_params::LootParams;
use super::parameters::loot_context_params::LootContextParam;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::attribute::Value;
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

pub enum LootRandom<'r> {
    Legacy(&'r mut LegacyRandomSource),
    Xoroshiro(&'r mut XoroshiroRandomSource),
}

macro_rules! delegate {
    ($self:ident, $r:ident => $e:expr) => {
        match $self {
            LootRandom::Legacy($r) => $e,
            LootRandom::Xoroshiro($r) => $e,
        }
    };
}

impl RandomSource for LootRandom<'_> {
    fn set_seed(&mut self, seed: i64) {
        delegate!(self, r => r.set_seed(seed))
    }

    fn next_int(&mut self) -> i32 {
        delegate!(self, r => r.next_int())
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        delegate!(self, r => r.next_int_bounded(bound))
    }

    fn next_long(&mut self) -> i64 {
        delegate!(self, r => r.next_long())
    }

    fn next_boolean(&mut self) -> bool {
        delegate!(self, r => r.next_boolean())
    }

    fn next_float(&mut self) -> f32 {
        delegate!(self, r => r.next_float())
    }

    fn next_double(&mut self) -> f64 {
        delegate!(self, r => r.next_double())
    }

    fn next_gaussian(&mut self) -> f64 {
        delegate!(self, r => r.next_gaussian())
    }
}

enum ContextRandom {
    Owned(LegacyRandomSource),
    Level,
    Sequence(String, XoroshiroRandomSource),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LootDataType {
    Table,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VisitedEntry {
    ty: LootDataType,
    value: *const (),
}

impl VisitedEntry {
    pub fn new<T>(ty: LootDataType, value: &T) -> VisitedEntry {
        VisitedEntry { ty, value: value as *const T as *const () }
    }
}

pub struct LootContext<'a> {
    pub params: LootParams<'a>,
    random: ContextRandom,
    visited_elements: Vec<VisitedEntry>,
}

impl<'a> LootContext<'a> {
    pub fn has_parameter(&self, param: LootContextParam) -> bool {
        self.params.has(param)
    }

    pub fn add_dynamic_drops(&mut self, location: &str, output: &mut dyn FnMut(ItemStack)) {
        self.params.add_dynamic_drops(location, output);
    }

    pub fn push_visited_element(&mut self, element: VisitedEntry) -> bool {
        if self.visited_elements.contains(&element) {
            return false;
        }
        self.visited_elements.push(element);
        true
    }

    pub fn pop_visited_element(&mut self, element: VisitedEntry) {
        self.visited_elements.retain(|e| *e != element);
    }

    pub fn random(&mut self) -> LootRandom<'_> {
        match &mut self.random {
            ContextRandom::Owned(random) => LootRandom::Legacy(random),
            ContextRandom::Level => LootRandom::Legacy(&mut self.params.level.random),
            ContextRandom::Sequence(_, random) => LootRandom::Xoroshiro(random),
        }
    }

    pub fn luck(&self) -> f32 {
        self.params.luck
    }

    pub fn level(&mut self) -> &mut ServerLevel {
        self.params.level
    }

    pub fn environment_attribute_value(&self, attribute: &EnvironmentAttributeInfo) -> Value {
        self.params.level.environment_attribute_loot_value(attribute, self.params.origin)
    }
}

impl Drop for LootContext<'_> {
    fn drop(&mut self) {
        if let ContextRandom::Sequence(key, random) = &self.random {
            let seed = self.params.level.seed;
            *self.params.level.random_sequences.borrow_mut().get(key, seed) = random.clone();
        }
    }
}

pub struct LootContextBuilder<'a> {
    params: LootParams<'a>,
    random: Option<LegacyRandomSource>,
    level_random: bool,
}

impl<'a> LootContextBuilder<'a> {
    pub fn new(params: LootParams<'a>) -> LootContextBuilder<'a> {
        LootContextBuilder { params, random: None, level_random: false }
    }

    pub fn with_level_random_source(mut self) -> LootContextBuilder<'a> {
        self.level_random = true;
        self
    }

    pub fn with_optional_random_seed(mut self, seed: i64) -> LootContextBuilder<'a> {
        if seed != 0 {
            self.random = Some(LegacyRandomSource::new(seed));
        }
        self
    }

    pub fn create(self, random_sequence_key: Option<&str>) -> LootContext<'a> {
        let LootContextBuilder { params, random, level_random } = self;
        let random = match (random, random_sequence_key) {
            (Some(random), _) => ContextRandom::Owned(random),
            (None, _) if level_random => ContextRandom::Level,
            (None, Some(key)) => {
                let seed = params.level.seed;
                ContextRandom::Sequence(key.to_string(), params.level.random_sequences.borrow_mut().get(key, seed).clone())
            }
            (None, None) => ContextRandom::Level,
        };
        LootContext { params, random, visited_elements: Vec::new() }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
}

impl EntityTarget {
    pub const VALUES: [(&'static str, EntityTarget); 6] = [
        ("this", EntityTarget::This),
        ("attacker", EntityTarget::Attacker),
        ("direct_attacker", EntityTarget::DirectAttacker),
        ("attacking_player", EntityTarget::AttackingPlayer),
        ("target_entity", EntityTarget::TargetEntity),
        ("interacting_entity", EntityTarget::InteractingEntity),
    ];

    pub fn decode(tag: &Nbt) -> DataResult<EntityTarget> {
        codec::string_enum(tag, &EntityTarget::VALUES)
    }

    pub fn context_param(self) -> LootContextParam {
        match self {
            EntityTarget::This => LootContextParam::ThisEntity,
            EntityTarget::Attacker => LootContextParam::AttackingEntity,
            EntityTarget::DirectAttacker => LootContextParam::DirectAttackingEntity,
            EntityTarget::AttackingPlayer => LootContextParam::LastDamagePlayer,
            EntityTarget::TargetEntity => LootContextParam::TargetEntity,
            EntityTarget::InteractingEntity => LootContextParam::InteractingEntity,
        }
    }

    pub fn get(self, params: &LootParams) -> Option<EntityId> {
        match self {
            EntityTarget::This => params.this_entity,
            EntityTarget::Attacker => params.attacking_entity,
            EntityTarget::DirectAttacker => params.direct_attacking_entity,
            EntityTarget::AttackingPlayer => params.last_damage_player,
            EntityTarget::TargetEntity => params.target_entity,
            EntityTarget::InteractingEntity => params.interacting_entity,
        }
    }
}
