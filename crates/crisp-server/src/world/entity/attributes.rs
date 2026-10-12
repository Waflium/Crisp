use std::borrow::Cow;

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_core::random::java_string_hash;
use crisp_data::{ATTRIBUTES, AttributeInfo};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::parse_identifier;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Attribute {
    AirDragModifier,
    Armor,
    ArmorToughness,
    AttackDamage,
    AttackKnockback,
    AttackSpeed,
    BelowNameDistance,
    BlockBreakSpeed,
    BlockInteractionRange,
    Bounciness,
    BurningTime,
    CameraDistance,
    ExplosionKnockbackResistance,
    EntityInteractionRange,
    FallDamageMultiplier,
    FlyingSpeed,
    FollowRange,
    FrictionModifier,
    Gravity,
    JumpStrength,
    KnockbackResistance,
    Luck,
    MaxAbsorption,
    MaxHealth,
    MiningEfficiency,
    MovementEfficiency,
    MovementSpeed,
    NameTagDistance,
    OxygenBonus,
    SafeFallDistance,
    Scale,
    SneakingSpeed,
    SpawnReinforcements,
    StepHeight,
    SubmergedMiningSpeed,
    SweepingDamageRatio,
    TemptRange,
    WaterMovementEfficiency,
    WaypointTransmitRange,
    WaypointReceiveRange,
}

impl Attribute {
    pub const COUNT: usize = 40;

    pub const ALL: [Attribute; Attribute::COUNT] = {
        use Attribute::*;
        [
            AirDragModifier, Armor, ArmorToughness, AttackDamage, AttackKnockback, AttackSpeed, BelowNameDistance, BlockBreakSpeed, BlockInteractionRange,
            Bounciness, BurningTime, CameraDistance, ExplosionKnockbackResistance, EntityInteractionRange, FallDamageMultiplier, FlyingSpeed, FollowRange,
            FrictionModifier, Gravity, JumpStrength, KnockbackResistance, Luck, MaxAbsorption, MaxHealth, MiningEfficiency, MovementEfficiency,
            MovementSpeed, NameTagDistance, OxygenBonus, SafeFallDistance, Scale, SneakingSpeed, SpawnReinforcements, StepHeight, SubmergedMiningSpeed,
            SweepingDamageRatio, TemptRange, WaterMovementEfficiency, WaypointTransmitRange, WaypointReceiveRange,
        ]
    };

    pub fn by_id(id: u8) -> Attribute {
        Attribute::ALL[id as usize]
    }

    fn info(self) -> &'static AttributeInfo {
        &ATTRIBUTES[self as usize]
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn description_id(self) -> &'static str {
        self.info().description_id
    }

    pub fn default_value(self) -> f64 {
        self.info().default_value
    }

    fn sanitize(self, value: f64) -> f64 {
        let info = self.info();
        if value.is_nan() { info.min_value } else { value.clamp(info.min_value, info.max_value) }
    }

    pub fn syncable(self) -> bool {
        self.info().syncable
    }

    pub fn by_name(name: &str) -> Option<Attribute> {
        ATTRIBUTES.iter().position(|a| a.name == name).map(|id| Attribute::ALL[id])
    }

    pub fn protocol_id(self) -> i32 {
        self as i32
    }

    fn bit(self) -> u64 {
        1 << self as u64
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Operation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

impl Operation {
    pub fn by_id(id: u8) -> Operation {
        match id {
            1 => Operation::AddMultipliedBase,
            2 => Operation::AddMultipliedTotal,
            _ => Operation::AddValue,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Operation::AddValue => "add_value",
            Operation::AddMultipliedBase => "add_multiplied_base",
            Operation::AddMultipliedTotal => "add_multiplied_total",
        }
    }

    pub fn by_name(name: &str) -> Option<Operation> {
        [Operation::AddValue, Operation::AddMultipliedBase, Operation::AddMultipliedTotal].into_iter().find(|o| o.name() == name)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AttributeModifier {
    pub id: Cow<'static, str>,
    pub amount: f64,
    pub operation: Operation,
}

impl AttributeModifier {
    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String(self.id.to_string())),
            ("amount".into(), Nbt::Double(self.amount)),
            ("operation".into(), Nbt::String(self.operation.name().into())),
        ])
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<AttributeModifier> {
        let id = parse_identifier(nbt.get("id")?.as_str()?)?;
        let amount = nbt.get("amount")?.as_f64()?;
        let operation = Operation::by_name(nbt.get("operation")?.as_str()?)?;
        Some(AttributeModifier { id: Cow::Owned(id), amount, operation })
    }
}

pub const SPEED_MODIFIER_SPRINTING: AttributeModifier =
    AttributeModifier { id: Cow::Borrowed("minecraft:sprinting"), amount: 0.3f32 as f64, operation: Operation::AddMultipliedTotal };

fn identifier_hash(id: &str) -> i32 {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    java_string_hash(namespace).wrapping_mul(31).wrapping_add(java_string_hash(path))
}

type ModifierTable = Option<Object2ObjectOpenHashMap<f64>>;

fn put(table: &mut ModifierTable, id: &str, amount: f64) {
    table.get_or_insert_with(|| Object2ObjectOpenHashMap::with_hash(16, 0.75, identifier_hash)).insert(id.to_owned(), amount);
}

fn amounts(table: &ModifierTable) -> impl Iterator<Item = f64> + '_ {
    table.iter().flat_map(|t| t.iter().map(|(_, amount)| *amount))
}

#[derive(Clone, Debug)]
pub struct AttributeInstance {
    pub attribute: Attribute,
    base_value: f64,
    modifiers: Vec<(AttributeModifier, bool)>,
    modifiers_by_operation: [ModifierTable; 3],
    value: f64,
    to_sync: bool,
    to_update: bool,
}

impl AttributeInstance {
    fn new(attribute: Attribute, base_value: f64) -> AttributeInstance {
        let mut instance = AttributeInstance {
            attribute,
            base_value,
            modifiers: Vec::new(),
            modifiers_by_operation: Default::default(),
            value: 0.0,
            to_sync: false,
            to_update: false,
        };
        instance.set_dirty();
        instance
    }

    pub fn base_value(&self) -> f64 {
        self.base_value
    }

    pub fn set_base_value(&mut self, base_value: f64) {
        if base_value != self.base_value {
            self.base_value = base_value;
            self.set_dirty();
        }
    }

    pub fn modifiers(&self) -> impl Iterator<Item = &AttributeModifier> {
        self.modifiers.iter().map(|(m, _)| m)
    }

    pub fn modifier(&self, id: &str) -> Option<&AttributeModifier> {
        self.modifiers().find(|m| m.id == id)
    }

    pub fn has_modifier(&self, id: &str) -> bool {
        self.modifier(id).is_some()
    }

    fn add_modifier(&mut self, modifier: AttributeModifier, permanent: bool) {
        assert!(!self.has_modifier(&modifier.id), "Modifier is already applied on this attribute!");
        put(&mut self.modifiers_by_operation[modifier.operation as usize], &modifier.id, modifier.amount);
        self.modifiers.push((modifier, permanent));
        self.set_dirty();
    }

    pub fn add_transient_modifier(&mut self, modifier: AttributeModifier) {
        self.add_modifier(modifier, false);
    }

    pub fn add_or_update_transient_modifier(&mut self, modifier: AttributeModifier) {
        match self.modifiers.iter_mut().find(|(m, _)| m.id == modifier.id) {
            Some((existing, _)) if *existing == modifier => return,
            Some(entry) => entry.0 = modifier.clone(),
            None => self.modifiers.push((modifier.clone(), false)),
        }
        put(&mut self.modifiers_by_operation[modifier.operation as usize], &modifier.id, modifier.amount);
        self.set_dirty();
    }

    pub fn add_permanent_modifier(&mut self, modifier: AttributeModifier) {
        self.add_modifier(modifier, true);
    }

    pub fn add_permanent_modifiers(&mut self, modifiers: Vec<AttributeModifier>) {
        for modifier in modifiers {
            self.add_permanent_modifier(modifier);
        }
    }

    pub fn permanent_modifiers(&self) -> impl Iterator<Item = &AttributeModifier> {
        self.modifiers.iter().filter(|(_, permanent)| *permanent).map(|(m, _)| m)
    }

    pub fn remove_modifier(&mut self, id: &str) -> bool {
        let Some(index) = self.modifiers.iter().position(|(m, _)| m.id == id) else { return false };
        let (modifier, _) = self.modifiers.remove(index);
        if let Some(table) = &mut self.modifiers_by_operation[modifier.operation as usize] {
            table.remove(id);
        }
        self.set_dirty();
        true
    }

    fn set_dirty(&mut self) {
        self.value = self.calculate_value();
        self.to_update = true;
        self.to_sync = self.attribute.syncable();
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    fn calculate_value(&self) -> f64 {
        let [add_value, add_multiplied_base, add_multiplied_total] = &self.modifiers_by_operation;
        let mut base = self.base_value;
        for amount in amounts(add_value) {
            base += amount;
        }
        let mut result = base;
        for amount in amounts(add_multiplied_base) {
            result += base * amount;
        }
        for amount in amounts(add_multiplied_total) {
            result *= 1.0 + amount;
        }
        self.attribute.sanitize(result)
    }

    fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("id".into(), Nbt::String(self.attribute.name().into())), ("base".into(), Nbt::Double(self.base_value))];
        let modifiers: Vec<Nbt> = self.modifiers.iter().filter(|(_, permanent)| *permanent).map(|(m, _)| m.to_nbt()).collect();
        if !modifiers.is_empty() {
            fields.push(("modifiers".into(), Nbt::List(modifiers)));
        }
        Nbt::Compound(fields)
    }

    fn apply(&mut self, base_value: f64, modifiers: Vec<AttributeModifier>) {
        self.base_value = base_value;
        for modifier in modifiers {
            put(&mut self.modifiers_by_operation[modifier.operation as usize], &modifier.id, modifier.amount);
            match self.modifiers.iter_mut().find(|(m, _)| m.id == modifier.id) {
                Some(entry) => *entry = (modifier, true),
                None => self.modifiers.push((modifier, true)),
            }
        }
        self.set_dirty();
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.attribute.protocol_id());
        w.f64(self.base_value);
        w.varint(self.modifiers.len() as i32);
        for (m, _) in &self.modifiers {
            w.string(&m.id);
            w.f64(m.amount);
            w.varint(m.operation as i32);
        }
    }
}

pub struct AttributeSupplier {
    attributes: u64,
    base_values: &'static [(Attribute, f64)],
}

impl AttributeSupplier {
    const fn of(attributes: &[Attribute], base_values: &'static [(Attribute, f64)]) -> AttributeSupplier {
        let mut bits = 0u64;
        let mut i = 0;
        while i < attributes.len() {
            bits |= 1 << attributes[i] as u64;
            i += 1;
        }
        AttributeSupplier { attributes: bits, base_values }
    }

    fn has_attribute(&self, attribute: Attribute) -> bool {
        self.attributes & attribute.bit() != 0
    }

    fn base_value(&self, attribute: Attribute) -> f64 {
        self.base_values.iter().find(|(a, _)| *a == attribute).map_or(attribute.default_value(), |(_, v)| *v)
    }
}

pub static PLAYER_ATTRIBUTES: AttributeSupplier = {
    use Attribute::*;
    AttributeSupplier::of(
        &[
            MaxHealth, KnockbackResistance, MovementSpeed, Armor, ArmorToughness, MaxAbsorption, StepHeight, Scale, Gravity, SafeFallDistance,
            FallDamageMultiplier, JumpStrength, EntityInteractionRange, OxygenBonus, BurningTime, ExplosionKnockbackResistance, WaterMovementEfficiency,
            MovementEfficiency, AttackKnockback, CameraDistance, WaypointTransmitRange, Bounciness, AirDragModifier, FrictionModifier, NameTagDistance,
            BelowNameDistance, AttackDamage, AttackSpeed, Luck, BlockInteractionRange, BlockBreakSpeed, SubmergedMiningSpeed, SneakingSpeed,
            MiningEfficiency, SweepingDamageRatio, WaypointReceiveRange,
        ],
        &[(AttackDamage, 1.0), (MovementSpeed, 0.1f32 as f64), (WaypointTransmitRange, 6.0e7), (WaypointReceiveRange, 6.0e7)],
    )
};

pub static CHICKEN_ATTRIBUTES: AttributeSupplier = {
    use Attribute::*;
    AttributeSupplier::of(
        &[
            MaxHealth, KnockbackResistance, MovementSpeed, Armor, ArmorToughness, MaxAbsorption, StepHeight, Scale, Gravity, SafeFallDistance,
            FallDamageMultiplier, JumpStrength, EntityInteractionRange, OxygenBonus, BurningTime, ExplosionKnockbackResistance, WaterMovementEfficiency,
            MovementEfficiency, AttackKnockback, CameraDistance, WaypointTransmitRange, Bounciness, AirDragModifier, FrictionModifier, NameTagDistance,
            BelowNameDistance, FollowRange, TemptRange,
        ],
        &[(FollowRange, 16.0), (TemptRange, 10.0), (MaxHealth, 4.0), (MovementSpeed, 0.25)],
    )
};

pub struct AttributeMap {
    supplier: &'static AttributeSupplier,
    instances: Vec<AttributeInstance>,
    index: [u8; Attribute::COUNT],
}

impl AttributeMap {
    pub fn new(supplier: &'static AttributeSupplier) -> AttributeMap {
        AttributeMap { supplier, instances: Vec::new(), index: [u8::MAX; Attribute::COUNT] }
    }

    pub fn get(&self, attribute: Attribute) -> Option<&AttributeInstance> {
        self.instances.get(self.index[attribute as usize] as usize)
    }

    pub fn get_mut(&mut self, attribute: Attribute) -> Option<&mut AttributeInstance> {
        self.instances.get_mut(self.index[attribute as usize] as usize)
    }

    pub fn instance(&mut self, attribute: Attribute) -> Option<&mut AttributeInstance> {
        let index = self.index[attribute as usize];
        if index != u8::MAX {
            return Some(&mut self.instances[index as usize]);
        }
        if !self.supplier.has_attribute(attribute) {
            return None;
        }
        self.index[attribute as usize] = self.instances.len() as u8;
        self.instances.push(AttributeInstance::new(attribute, self.supplier.base_value(attribute)));
        self.instances.last_mut()
    }

    pub fn has_attribute(&self, attribute: Attribute) -> bool {
        self.get(attribute).is_some() || self.supplier.has_attribute(attribute)
    }

    pub fn has_modifier(&self, attribute: Attribute, id: &str) -> bool {
        self.get(attribute).is_some_and(|i| i.has_modifier(id))
    }

    pub fn value(&self, attribute: Attribute) -> f64 {
        match self.get(attribute) {
            Some(instance) => instance.value(),
            None => attribute.sanitize(self.supplier.base_value(attribute)),
        }
    }

    pub fn base_value(&self, attribute: Attribute) -> f64 {
        match self.get(attribute) {
            Some(instance) => instance.base_value(),
            None => self.supplier.base_value(attribute),
        }
    }

    pub fn reset_base_value(&mut self, attribute: Attribute) -> bool {
        if !self.supplier.has_attribute(attribute) {
            return false;
        }
        let base_value = self.supplier.base_value(attribute);
        if let Some(instance) = self.get_mut(attribute) {
            instance.set_base_value(base_value);
        }
        true
    }

    pub fn assign_permanent_modifiers(&mut self, other: &AttributeMap) {
        for other in &other.instances {
            if let Some(instance) = self.instance(other.attribute) {
                instance.add_permanent_modifiers(other.permanent_modifiers().cloned().collect());
            }
        }
    }

    pub fn assign_base_values(&mut self, other: &AttributeMap) {
        for other in &other.instances {
            if let Some(instance) = self.instance(other.attribute) {
                instance.set_base_value(other.base_value());
            }
        }
    }

    pub fn syncable_attributes(&self) -> impl Iterator<Item = &AttributeInstance> {
        self.instances.iter().filter(|i| i.attribute.syncable())
    }

    pub fn write_syncable(&self, w: &mut Writer, entity_id: i32) {
        w.varint(entity_id);
        w.varint(self.syncable_attributes().count() as i32);
        self.syncable_attributes().for_each(|instance| instance.write(w));
    }

    pub fn has_to_sync(&self) -> bool {
        self.instances.iter().any(|i| i.to_sync)
    }

    pub fn write_to_sync(&mut self, w: &mut Writer, entity_id: i32) {
        w.varint(entity_id);
        w.varint(self.instances.iter().filter(|i| i.to_sync).count() as i32);
        for instance in self.instances.iter_mut().filter(|i| i.to_sync) {
            instance.write(w);
            instance.to_sync = false;
        }
    }

    pub fn take_to_update(&mut self) -> u64 {
        let mut bits = 0;
        for instance in &mut self.instances {
            if instance.to_update {
                instance.to_update = false;
                bits |= instance.attribute.bit();
            }
        }
        bits
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::List(self.instances.iter().map(AttributeInstance::to_nbt).collect())
    }

    pub fn load(&mut self, nbt: &Nbt) {
        for entry in nbt.as_list() {
            let Some(attribute) = entry.get("id").and_then(Nbt::as_str).and_then(parse_identifier).and_then(|id| Attribute::by_name(&id)) else { continue };
            let base_value = match entry.get("base") {
                None => 0.0,
                Some(base) => match base.as_f64() {
                    Some(v) => v,
                    None => continue,
                },
            };
            let modifiers = match entry.get("modifiers") {
                None => Vec::new(),
                Some(Nbt::List(list)) => match list.iter().map(AttributeModifier::from_nbt).collect::<Option<Vec<_>>>() {
                    Some(modifiers) => modifiers,
                    None => continue,
                },
                Some(_) => continue,
            };
            if let Some(instance) = self.instance(attribute) {
                instance.apply(base_value, modifiers);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_are_in_registry_order() {
        for attribute in Attribute::ALL {
            let snake: String = format!("{attribute:?}")
                .chars()
                .enumerate()
                .flat_map(|(i, c)| if c.is_ascii_uppercase() && i > 0 { vec!['_', c.to_ascii_lowercase()] } else { vec![c.to_ascii_lowercase()] })
                .collect();
            assert_eq!(attribute.name(), format!("minecraft:{snake}"));
            assert_eq!(crisp_data::registry_id(crisp_data::registry::ATTRIBUTE, attribute.name()), attribute.protocol_id());
        }
        assert_eq!(ATTRIBUTES.len(), Attribute::COUNT);
    }
}
