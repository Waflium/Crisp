use crisp_data::{BlockState, Item, Rarity, items};
use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use super::component::swing_animation::SwingAnimation;
use super::component::values::{self, ItemEnchantments, Tool};
use super::component::{self as dc, DataComponentPatch, DataComponentType, PatchedDataComponentMap, codec, item_components};
use crate::network::chat::{ChatFormatting, Component, HoverEvent};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Debug)]
pub struct ItemStack {
    pub item: Item,
    pub count: i32,
    components: PatchedDataComponentMap,
}

impl PartialEq for ItemStack {
    fn eq(&self, other: &ItemStack) -> bool {
        ItemStack::matches(self, other)
    }
}

pub fn rarity_color(rarity: Rarity) -> ChatFormatting {
    match rarity {
        Rarity::Common => ChatFormatting::White,
        Rarity::Uncommon => ChatFormatting::Yellow,
        Rarity::Rare => ChatFormatting::Aqua,
        Rarity::Epic => ChatFormatting::LightPurple,
    }
}

impl ItemStack {
    pub fn new(item: Item, count: i32) -> ItemStack {
        ItemStack { item, count, components: PatchedDataComponentMap::new(item_components(item)) }
    }

    pub fn with_patch(item: Item, count: i32, patch: &DataComponentPatch) -> ItemStack {
        ItemStack { item, count, components: PatchedDataComponentMap::from_patch(item_components(item), patch) }
    }

    pub fn from_template(item: Item, count: i32, patch: &DataComponentPatch) -> Option<ItemStack> {
        let stack = ItemStack::with_patch(item, count, patch);
        stack.validate_strict().ok().map(|()| stack)
    }

    pub fn empty() -> ItemStack {
        ItemStack::new(items::AIR, 0)
    }

    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item == items::AIR
    }

    pub fn get_item(&self) -> Item {
        if self.is_empty() { items::AIR } else { self.item }
    }

    pub fn is(&self, item: Item) -> bool {
        self.get_item() == item
    }

    pub fn copy(&self) -> ItemStack {
        if self.is_empty() { ItemStack::empty() } else { self.clone() }
    }

    pub fn grow(&mut self, amount: i32) {
        self.count += amount;
    }

    pub fn shrink(&mut self, amount: i32) {
        self.count -= amount;
    }

    pub fn consume(&mut self, amount: i32, level: &ServerLevel, owner: Option<EntityId>) {
        if owner.is_none_or(|owner| !level.has_infinite_materials(owner)) {
            self.shrink(amount);
        }
    }

    pub fn hurt_and_break(&mut self, amount: i32, level: &mut ServerLevel) {
        let new_amount = if !self.is_damageable_item() {
            0
        } else if amount > 0 {
            level.process_durability_change(self, amount)
        } else {
            amount
        };
        if new_amount != 0 {
            self.set_damage_value(self.damage_value() + new_amount);
            if self.is_broken() {
                self.shrink(1);
            }
        }
    }

    pub fn limit_size(&mut self, max: i32) {
        if !self.is_empty() && self.count > max {
            self.count = max;
        }
    }

    pub fn components(&self) -> &PatchedDataComponentMap {
        &self.components
    }

    pub fn components_patch(&self) -> &DataComponentPatch {
        self.components.as_patch()
    }

    pub fn get(&self, ty: DataComponentType) -> Option<&Nbt> {
        if self.is_empty() { None } else { self.components.get(ty) }
    }

    pub fn has(&self, ty: DataComponentType) -> bool {
        self.get(ty).is_some()
    }

    pub fn attack_animation(&self) -> SwingAnimation {
        self.get(dc::ATTACK_ANIMATION).map_or(SwingAnimation::DEFAULT, SwingAnimation::from_nbt)
    }

    pub fn interact_animation(&self) -> SwingAnimation {
        self.get(dc::INTERACT_ANIMATION).map_or(SwingAnimation::DEFAULT, SwingAnimation::from_nbt)
    }

    pub fn can_break_block_in_adventure_mode(&self, state: BlockState) -> bool {
        let Some(predicates) = self.get(dc::CAN_BREAK) else { return false };
        let single = std::slice::from_ref(predicates);
        let list = match predicates {
            Nbt::List(items) => items.as_slice(),
            _ => single,
        };
        list.iter().any(|predicate| {
            let blocks = predicate.get("blocks").map(values::HolderSet::from_nbt);
            ["state", "nbt", "components", "predicates"].iter().all(|key| predicate.get(key).is_none())
                && blocks.is_none_or(|blocks| blocks.contains("minecraft:block", state.block().name(), state.block().0 as i32))
        })
    }

    pub fn can_be_hurt_by(&self, source: &crate::world::damagesource::DamageSource) -> bool {
        let Some(resistant) = self.get(dc::DAMAGE_RESISTANT) else { return true };
        let tag = resistant.get("types").and_then(Nbt::as_str).map(|t| t.trim_start_matches('#'));
        match tag.and_then(|t| crate::commands::arguments::registry_tag("minecraft:damage_type", t)) {
            Some(ids) => !ids.contains(&(source.ty.0 as i32)),
            None => true,
        }
    }

    pub fn set(&mut self, ty: DataComponentType, value: Option<Nbt>) -> Option<Nbt> {
        self.components.set(ty, value)
    }

    pub fn set_value(&mut self, ty: DataComponentType, value: Nbt) -> Option<Nbt> {
        let value = ty.decode(&value).unwrap_or(value);
        self.components.set(ty, Some(value))
    }

    pub fn remove(&mut self, ty: DataComponentType) -> Option<Nbt> {
        self.components.remove(ty)
    }

    pub fn apply_components(&mut self, patch: &DataComponentPatch) {
        self.components.apply_patch(patch);
    }

    pub fn apply_components_and_validate(&mut self, patch: &DataComponentPatch) -> Result<(), String> {
        let old = self.components.as_patch().clone();
        self.components.apply_patch(patch);
        let result = self.validate_strict();
        if result.is_err() {
            self.components.restore_patch(old);
        }
        result
    }

    pub fn get_int(&self, ty: DataComponentType, default: i32) -> i32 {
        self.get(ty).map_or(default, codec::as_i32)
    }

    pub fn max_stack_size(&self) -> i32 {
        self.get_int(dc::MAX_STACK_SIZE, 1)
    }

    pub fn max_damage(&self) -> i32 {
        self.get_int(dc::MAX_DAMAGE, 0)
    }

    pub fn damage_value(&self) -> i32 {
        self.get_int(dc::DAMAGE, 0).clamp(0, self.max_damage().max(0))
    }

    pub fn set_damage_value(&mut self, value: i32) {
        let value = value.clamp(0, self.max_damage().max(0));
        self.set(dc::DAMAGE, Some(Nbt::Int(value)));
    }

    pub fn is_damageable_item(&self) -> bool {
        self.has(dc::MAX_DAMAGE) && !self.has(dc::UNBREAKABLE) && self.has(dc::DAMAGE)
    }

    pub fn is_valid_repair_item(&self, repair_item: &ItemStack) -> bool {
        self.get(dc::REPAIRABLE)
            .and_then(|repairable| repairable.get("items"))
            .and_then(|items| crate::core::holder_set::HolderSet::decode("minecraft:item", items).ok())
            .is_some_and(|items| items.contains(repair_item.get_item().0 as i32))
    }

    pub fn is_damaged(&self) -> bool {
        self.is_damageable_item() && self.damage_value() > 0
    }

    pub fn is_broken(&self) -> bool {
        self.is_damageable_item() && self.damage_value() >= self.max_damage()
    }

    pub fn is_stackable(&self) -> bool {
        self.max_stack_size() > 1 && (!self.is_damageable_item() || !self.is_damaged())
    }

    pub fn hurt(&mut self, amount: i32) -> bool {
        if !self.is_damageable_item() || amount == 0 {
            return false;
        }
        let damage = self.damage_value() + amount;
        self.set_damage_value(damage);
        self.is_broken()
    }

    pub fn tool(&self) -> Option<Tool> {
        self.get(dc::TOOL).map(Tool::from_nbt)
    }

    pub fn destroy_speed(&self, state: BlockState) -> f32 {
        if !self.components.has_non_default(dc::TOOL) {
            return self.item.info().tool.as_ref().map_or(1.0, |t| t.mining_speed(state.block()));
        }
        self.tool().map_or(1.0, |t| t.mining_speed(state.block()))
    }

    pub fn is_correct_tool_for_drops(&self, state: BlockState) -> bool {
        if !self.components.has_non_default(dc::TOOL) {
            return self.item.info().tool.as_ref().is_some_and(|t| t.is_correct_for_drops(state.block()));
        }
        self.tool().is_some_and(|t| t.is_correct_for_drops(state.block()))
    }

    pub fn enchantments(&self) -> ItemEnchantments {
        self.get(dc::ENCHANTMENTS).map(ItemEnchantments::from_nbt).unwrap_or_default()
    }

    pub fn is_enchantable(&self) -> bool {
        if !self.has(dc::ENCHANTABLE) {
            return false;
        }
        self.get(dc::ENCHANTMENTS).is_some_and(|e| matches!(e, Nbt::Compound(f) if f.is_empty()))
    }

    pub fn is_enchanted(&self) -> bool {
        self.get(dc::ENCHANTMENTS).is_some_and(|e| matches!(e, Nbt::Compound(f) if !f.is_empty()))
    }

    pub fn rarity(&self) -> Rarity {
        let base = self.get(dc::RARITY).map_or(Rarity::Common, values::rarity);
        if !self.is_enchanted() {
            return base;
        }
        match base {
            Rarity::Common | Rarity::Uncommon => Rarity::Rare,
            Rarity::Rare => Rarity::Epic,
            other => other,
        }
    }

    pub fn custom_name(&self) -> Option<Component> {
        if let Some(name) = self.get(dc::CUSTOM_NAME) {
            return Some(values::text(name));
        }
        let title = values::WrittenBookContent::from_nbt(self.get(dc::WRITTEN_BOOK_CONTENT)?).title;
        (!title.trim().is_empty()).then(|| Component::literal(title))
    }

    pub fn item_name(&self) -> Component {
        let info = self.item.info();
        let class = |c: &str| info.classes.contains(&c);
        if class("CompassItem") && self.has(dc::LODESTONE_TRACKER) {
            return Component::translatable("item.minecraft.lodestone_compass");
        }
        if class("PlayerHeadItem")
            && let Some((Some(name), _)) = self.get(dc::PROFILE).map(values::profile)
        {
            return Component::translatable_with(&format!("{}.named", info.description_id), vec![name.into()]);
        }
        if (class("PotionItem") || class("TippedArrowItem"))
            && let Some(potion) = self.get(dc::POTION_CONTENTS)
        {
            let contents = values::PotionContents::from_nbt(potion);
            let suffix = contents.custom_name.or_else(|| contents.potion.map(|p| potion_name(&p).to_string())).unwrap_or_else(|| "empty".into());
            return Component::translatable(&format!("{}.effect.{suffix}", info.description_id));
        }
        if class("ShieldItem")
            && let Some(color) = self.get(dc::BASE_COLOR).and_then(Nbt::as_str)
        {
            return Component::translatable(&format!("{}.{color}", info.description_id));
        }
        self.get(dc::ITEM_NAME).map_or_else(Component::empty, values::text)
    }

    pub fn hover_name(&self) -> Component {
        self.custom_name().unwrap_or_else(|| self.item_name())
    }

    pub fn display_name(&self) -> Component {
        let mut hover_name = Component::empty().append(self.hover_name());
        if self.has(dc::CUSTOM_NAME) {
            hover_name = hover_name.italic();
        }
        let mut result = Component::translatable_with("chat.square_brackets", vec![hover_name.into()]);
        if !self.is_empty() {
            result = result.with_color(rarity_color(self.rarity())).with_hover(self.show_item());
        }
        result
    }

    pub fn show_item(&self) -> HoverEvent {
        let components = match self.components_patch().to_nbt() {
            Nbt::Compound(fields) => fields,
            _ => Vec::new(),
        };
        HoverEvent::ShowItem { id: self.item.name(), count: self.count, components }
    }

    pub fn copy_with_count(&self, count: i32) -> ItemStack {
        ItemStack { count, ..self.clone() }
    }

    pub fn split(&mut self, amount: i32) -> ItemStack {
        let amount = amount.min(self.count);
        let result = self.copy_with_count(amount);
        self.count -= amount;
        result
    }

    pub fn transmute_copy(&self, item: Item, count: i32) -> ItemStack {
        ItemStack::with_patch(item, count, self.components_patch())
    }

    pub fn is_same_item(a: &ItemStack, b: &ItemStack) -> bool {
        a.get_item() == b.get_item()
    }

    pub fn is_same_item_same_components(a: &ItemStack, b: &ItemStack) -> bool {
        a.get_item() == b.get_item() && ((a.is_empty() && b.is_empty()) || a.components == b.components)
    }

    pub fn matches(a: &ItemStack, b: &ItemStack) -> bool {
        a.count == b.count && ItemStack::is_same_item_same_components(a, b)
    }

    pub fn validate_strict(&self) -> Result<(), String> {
        if self.has(dc::MAX_DAMAGE) && self.get_int(dc::MAX_STACK_SIZE, 1) > 1 {
            return Err("Item cannot be both damageable and stackable".into());
        }
        let check = |templates: Vec<values::ItemStackTemplate>| -> Result<(), String> {
            for t in templates {
                let Some(item) = Item::by_name(&t.item) else { continue };
                let max = ItemStack::with_patch(item, t.count, &dc::patch_of(&t.components)).max_stack_size();
                if t.count > max {
                    return Err(format!("Item stack with count of {} was larger than maximum: {max}", t.count));
                }
            }
            Ok(())
        };
        if let Some(container) = self.get(dc::CONTAINER) {
            check(values::container_slots(container).into_iter().map(|(_, t)| t).collect())?;
        }
        if let Some(bundle) = self.get(dc::BUNDLE_CONTENTS) {
            check(values::templates(bundle))?;
        }
        if let Some(projectiles) = self.get(dc::CHARGED_PROJECTILES) {
            check(values::templates(projectiles))?;
        }
        if self.count > self.max_stack_size() {
            return Err(format!("Item stack with stack size of {} was larger than maximum: {}", self.count, self.max_stack_size()));
        }
        Ok(())
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("id".to_string(), Nbt::String(self.item.name().into())), ("count".into(), Nbt::Int(self.count))];
        if !self.components_patch().is_empty() {
            fields.push(("components".into(), self.components_patch().to_nbt()));
        }
        Nbt::Compound(fields)
    }

    pub fn from_nbt(tag: &Nbt) -> Result<ItemStack, String> {
        codec::map(tag)?;
        let Some(id) = tag.get("id") else { return Err(format!("No key id in MapLike[{}]", crate::nbt::to_string(tag))) };
        let id = codec::Codec::Element("minecraft:item").decode(id)?;
        if id == Nbt::String("minecraft:air".into()) {
            return Err("Item must not be minecraft:air".into());
        }
        let item = Item::by_name(id.as_str().unwrap_or("")).ok_or("unknown item")?;
        let count = match tag.get("count") {
            Some(count) => codec::as_i32(&codec::Codec::Int(1, 99, codec::RangeError::Within).decode(count)?),
            None => 1,
        };
        let patch = match tag.get("components") {
            Some(components) => DataComponentPatch::from_nbt(components)?,
            None => DataComponentPatch::default(),
        };
        Ok(ItemStack::with_patch(item, count, &patch))
    }

    pub fn from_optional_nbt(tag: &Nbt) -> Result<Option<ItemStack>, String> {
        match tag {
            Nbt::Compound(fields) if fields.is_empty() => Ok(None),
            other => ItemStack::from_nbt(other).map(Some),
        }
    }
}

fn potion_name(id: &str) -> &str {
    let path = id.strip_prefix("minecraft:").unwrap_or(id);
    path.strip_prefix("long_").or_else(|| path.strip_prefix("strong_")).unwrap_or(path)
}

pub fn write_slot(w: &mut Writer, stack: Option<&ItemStack>) {
    match stack {
        Some(s) if !s.is_empty() => {
            w.varint(s.count);
            w.varint(s.item.0 as i32);
            s.components_patch().write(w, false).expect("component value encodes");
        }
        _ => w.varint(0),
    }
}

pub fn read_slot(r: &mut Reader, untrusted: bool) -> Result<Option<ItemStack>, String> {
    let count = r.varint().map_err(|e| e.to_string())?;
    if count <= 0 {
        return Ok(None);
    }
    let id = r.varint().map_err(|e| e.to_string())?;
    if id < 0 || id as usize >= Item::count() {
        return Err(format!("Unknown item id {id}"));
    }
    let patch = DataComponentPatch::read(r, untrusted)?;
    Ok(Some(ItemStack::with_patch(Item(id as u16), count, &patch)))
}

pub fn validate_encodable(stack: &ItemStack) -> Result<(), String> {
    if stack.is_empty() {
        return Ok(());
    }
    if !(1..=99).contains(&stack.count) {
        return Err(format!("Value must be within range [1;99]: {}", stack.count));
    }
    for (ty, value) in stack.components_patch().iter() {
        if let Some(value) = value
            && !ty.is_transient()
        {
            ty.decode(value)?;
        }
    }
    Ok(())
}
