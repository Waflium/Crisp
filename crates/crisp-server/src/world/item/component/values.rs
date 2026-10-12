#![allow(dead_code)]

use crisp_data::{Block, Rarity};
use crisp_protocol::nbt::Nbt;

use super::codec::{as_f32, as_f64, as_i32, as_i64};
use super::types::compound;
use crate::commands::arguments::registry_tag;
use crate::network::chat::Component;

fn string(v: Option<&Nbt>) -> Option<String> {
    v.and_then(Nbt::as_str).map(str::to_string)
}

fn bool_or(v: Option<&Nbt>, default: bool) -> bool {
    v.map_or(default, |v| as_i64(v) != 0)
}

fn f32_or(v: Option<&Nbt>, default: f32) -> f32 {
    v.map_or(default, as_f32)
}

fn i32_or(v: Option<&Nbt>, default: i32) -> i32 {
    v.map_or(default, as_i32)
}

#[derive(Clone, Debug, PartialEq)]
pub enum HolderSet {
    Tag(String),
    Direct(Vec<String>),
}

impl HolderSet {
    pub fn from_nbt(v: &Nbt) -> HolderSet {
        match v {
            Nbt::String(s) => match s.strip_prefix('#') {
                Some(tag) => HolderSet::Tag(tag.to_string()),
                None => HolderSet::Direct(vec![s.clone()]),
            },
            other => HolderSet::Direct(other.as_list().iter().filter_map(Nbt::as_str).map(str::to_string).collect()),
        }
    }

    pub fn contains(&self, registry: &str, name: &str, id: i32) -> bool {
        match self {
            HolderSet::Tag(tag) => registry_tag(registry, tag).is_some_and(|ids| ids.contains(&id)),
            HolderSet::Direct(names) => names.iter().any(|n| n == name),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemEnchantments {
    pub enchantments: Vec<(String, i32)>,
}

impl ItemEnchantments {
    pub fn from_nbt(v: &Nbt) -> ItemEnchantments {
        let Nbt::Compound(fields) = v else { return ItemEnchantments::default() };
        ItemEnchantments { enchantments: fields.iter().map(|(k, v)| (k.clone(), as_i32(v))).collect() }
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(self.enchantments.iter().map(|(k, v)| (k.clone(), Nbt::Int(*v))).collect())
    }

    pub fn level(&self, enchantment: &str) -> i32 {
        self.enchantments.iter().find(|(e, _)| e == enchantment).map_or(0, |(_, l)| *l)
    }

    pub fn is_empty(&self) -> bool {
        self.enchantments.is_empty()
    }

    pub fn set(&mut self, enchantment: &str, level: i32) {
        self.enchantments.retain(|(e, _)| e != enchantment);
        if level > 0 {
            self.enchantments.push((enchantment.to_string(), level.min(255)));
        }
    }

    pub fn upgrade(&mut self, enchantment: &str, level: i32) {
        if level > 0 {
            let level = level.min(255).max(self.level(enchantment));
            self.set(enchantment, level);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodProperties {
    pub nutrition: i32,
    pub saturation: f32,
    pub can_always_eat: bool,
}

impl FoodProperties {
    pub fn from_nbt(v: &Nbt) -> FoodProperties {
        FoodProperties { nutrition: i32_or(v.get("nutrition"), 0), saturation: f32_or(v.get("saturation"), 0.0), can_always_eat: bool_or(v.get("can_always_eat"), false) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Consumable {
    pub consume_seconds: f32,
    pub animation: String,
    pub sound: Nbt,
    pub has_consume_particles: bool,
    pub on_consume_effects: Vec<Nbt>,
}

impl Consumable {
    pub fn from_nbt(v: &Nbt) -> Consumable {
        Consumable {
            consume_seconds: f32_or(v.get("consume_seconds"), 1.6),
            animation: string(v.get("animation")).unwrap_or_else(|| "eat".into()),
            sound: v.get("sound").cloned().unwrap_or_else(|| Nbt::String("minecraft:entity.generic.eat".into())),
            has_consume_particles: bool_or(v.get("has_consume_particles"), true),
            on_consume_effects: v.get("on_consume_effects").map(|e| e.as_list().to_vec()).unwrap_or_default(),
        }
    }

    pub fn consume_ticks(&self) -> i32 {
        (self.consume_seconds * 20.0) as i32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolRule {
    pub blocks: HolderSet,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub rules: Vec<ToolRule>,
    pub default_mining_speed: f32,
    pub damage_per_block: i32,
    pub can_destroy_blocks_in_creative: bool,
}

impl Tool {
    pub fn from_nbt(v: &Nbt) -> Tool {
        let rules = v
            .get("rules")
            .map(Nbt::as_list)
            .unwrap_or(&[])
            .iter()
            .map(|r| ToolRule {
                blocks: r.get("blocks").map(HolderSet::from_nbt).unwrap_or(HolderSet::Direct(Vec::new())),
                speed: r.get("speed").map(as_f32),
                correct_for_drops: r.get("correct_for_drops").map(|b| as_i64(b) != 0),
            })
            .collect();
        Tool {
            rules,
            default_mining_speed: f32_or(v.get("default_mining_speed"), 1.0),
            damage_per_block: i32_or(v.get("damage_per_block"), 1),
            can_destroy_blocks_in_creative: bool_or(v.get("can_destroy_blocks_in_creative"), true),
        }
    }

    fn matches(rule: &ToolRule, block: Block) -> bool {
        rule.blocks.contains("minecraft:block", block.name(), block.0 as i32)
    }

    pub fn mining_speed(&self, block: Block) -> f32 {
        self.rules.iter().find(|r| r.speed.is_some() && Tool::matches(r, block)).and_then(|r| r.speed).unwrap_or(self.default_mining_speed)
    }

    pub fn is_correct_for_drops(&self, block: Block) -> bool {
        self.rules.iter().find(|r| r.correct_for_drops.is_some() && Tool::matches(r, block)).and_then(|r| r.correct_for_drops).unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Equippable {
    pub slot: String,
    pub equip_sound: Nbt,
    pub asset_id: Option<String>,
    pub camera_overlay: Option<String>,
    pub allowed_entities: Option<HolderSet>,
    pub dispensable: bool,
    pub swappable: bool,
    pub damage_on_hurt: bool,
    pub equip_on_interact: bool,
    pub can_be_sheared: bool,
    pub shearing_sound: Nbt,
}

impl Equippable {
    pub fn from_nbt(v: &Nbt) -> Equippable {
        Equippable {
            slot: string(v.get("slot")).unwrap_or_default(),
            equip_sound: v.get("equip_sound").cloned().unwrap_or_else(|| Nbt::String("minecraft:item.armor.equip_generic".into())),
            asset_id: string(v.get("asset_id")),
            camera_overlay: string(v.get("camera_overlay")),
            allowed_entities: v.get("allowed_entities").map(HolderSet::from_nbt),
            dispensable: bool_or(v.get("dispensable"), true),
            swappable: bool_or(v.get("swappable"), true),
            damage_on_hurt: bool_or(v.get("damage_on_hurt"), true),
            equip_on_interact: bool_or(v.get("equip_on_interact"), false),
            can_be_sheared: bool_or(v.get("can_be_sheared"), false),
            shearing_sound: v.get("shearing_sound").cloned().unwrap_or_else(|| Nbt::String("minecraft:item.shears.snip".into())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UseCooldown {
    pub seconds: f32,
    pub cooldown_group: Option<String>,
}

impl UseCooldown {
    pub fn from_nbt(v: &Nbt) -> UseCooldown {
        UseCooldown { seconds: f32_or(v.get("seconds"), 0.0), cooldown_group: string(v.get("cooldown_group")) }
    }

    pub fn ticks(&self) -> i32 {
        (self.seconds * 20.0) as i32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemStackTemplate {
    pub item: String,
    pub count: i32,
    pub components: Nbt,
}

impl ItemStackTemplate {
    pub fn from_nbt(v: &Nbt) -> ItemStackTemplate {
        ItemStackTemplate {
            item: string(v.get("id")).unwrap_or_default(),
            count: i32_or(v.get("count"), 1),
            components: v.get("components").cloned().unwrap_or(Nbt::Compound(Vec::new())),
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("id".to_string(), Nbt::String(self.item.clone()))];
        if self.count != 1 {
            fields.push(("count".into(), Nbt::Int(self.count)));
        }
        if !matches!(&self.components, Nbt::Compound(c) if c.is_empty()) {
            fields.push(("components".into(), self.components.clone()));
        }
        Nbt::Compound(fields)
    }

    pub fn create(&self) -> Option<crate::world::item::ItemStack> {
        let item = crisp_data::Item::by_name(&self.item)?;
        let patch = super::custom::patch_of(&self.components);
        crate::world::item::ItemStack::from_template(item, self.count, &patch)
    }

    pub fn decode(tag: &Nbt) -> Result<ItemStackTemplate, String> {
        if let Nbt::String(id) = tag {
            let item = crate::commands::arguments::parse_identifier(id).filter(|id| crisp_data::Item::by_name(id).is_some()).ok_or_else(|| format!("Unknown item: {id}"))?;
            return Ok(ItemStackTemplate { item, count: 1, components: Nbt::Compound(Vec::new()) });
        }
        let id = string(tag.get("id")).ok_or("No key id in MapLike")?;
        let item = crate::commands::arguments::parse_identifier(&id).filter(|id| crisp_data::Item::by_name(id).is_some()).ok_or_else(|| format!("Unknown item: {id}"))?;
        let count = i32_or(tag.get("count"), 1);
        if !(1..=99).contains(&count) {
            return Err(format!("Value {count} outside of range [1:99]"));
        }
        Ok(ItemStackTemplate { item, count, components: tag.get("components").cloned().unwrap_or(Nbt::Compound(Vec::new())) })
    }

    pub fn item(&self) -> crisp_data::Item {
        crisp_data::Item::by_name(&self.item).unwrap_or(crisp_data::items::AIR)
    }

    pub fn apply(&self, count: i32, additional_patch: &crate::world::item::component::DataComponentPatch) -> crate::world::item::ItemStack {
        let mut result = crate::world::item::ItemStack::with_patch(self.item(), count, additional_patch);
        result.apply_components(&super::custom::patch_of(&self.components));
        self.validate(result)
    }

    fn validate(&self, result: crate::world::item::ItemStack) -> crate::world::item::ItemStack {
        match result.validate_strict() {
            Ok(()) => result,
            Err(error) => {
                warn!("Can't create item stack with properties {:?}, error: {error}", self);
                crate::world::item::ItemStack::empty()
            }
        }
    }

    pub fn from_non_empty_stack(item_stack: &crate::world::item::ItemStack, new_count: i32) -> ItemStackTemplate {
        ItemStackTemplate { item: item_stack.get_item().name().to_string(), count: new_count, components: item_stack.components_patch().to_nbt() }
    }
}

pub fn container_slots(v: &Nbt) -> Vec<(i32, ItemStackTemplate)> {
    v.as_list().iter().map(|s| (i32_or(s.get("slot"), 0), s.get("item").map(ItemStackTemplate::from_nbt).unwrap_or(ItemStackTemplate { item: String::new(), count: 0, components: Nbt::Compound(Vec::new()) }))).collect()
}

pub fn templates(v: &Nbt) -> Vec<ItemStackTemplate> {
    v.as_list().iter().map(ItemStackTemplate::from_nbt).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobEffectInstance {
    pub effect: String,
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}

impl MobEffectInstance {
    pub fn from_nbt(v: &Nbt) -> MobEffectInstance {
        let show_particles = bool_or(v.get("show_particles"), true);
        MobEffectInstance {
            effect: string(v.get("id")).unwrap_or_default(),
            amplifier: v.get("amplifier").map_or(0, |a| as_i64(a) as u8 as i32),
            duration: i32_or(v.get("duration"), 0),
            ambient: bool_or(v.get("ambient"), false),
            show_particles,
            show_icon: bool_or(v.get("show_icon"), show_particles),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PotionContents {
    pub potion: Option<String>,
    pub custom_color: Option<i32>,
    pub custom_effects: Vec<MobEffectInstance>,
    pub custom_name: Option<String>,
}

impl PotionContents {
    pub fn from_nbt(v: &Nbt) -> PotionContents {
        PotionContents {
            potion: string(v.get("potion")),
            custom_color: v.get("custom_color").map(as_i32),
            custom_effects: v.get("custom_effects").map(Nbt::as_list).unwrap_or(&[]).iter().map(MobEffectInstance::from_nbt).collect(),
            custom_name: string(v.get("custom_name")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeModifier {
    pub attribute: String,
    pub id: String,
    pub amount: f64,
    pub operation: String,
    pub slot: String,
}

pub fn attribute_modifiers(v: &Nbt) -> Vec<AttributeModifier> {
    v.as_list()
        .iter()
        .map(|e| AttributeModifier {
            attribute: string(e.get("type")).unwrap_or_default(),
            id: string(e.get("id")).unwrap_or_default(),
            amount: e.get("amount").map_or(0.0, as_f64),
            operation: string(e.get("operation")).unwrap_or_default(),
            slot: string(e.get("slot")).unwrap_or_else(|| "any".into()),
        })
        .collect()
}

pub fn typed_entity_data(v: &Nbt) -> (String, Nbt) {
    let Nbt::Compound(fields) = v else { return (String::new(), Nbt::Compound(Vec::new())) };
    let id = string(v.get("id")).unwrap_or_default();
    (id, Nbt::Compound(fields.iter().filter(|(k, _)| k != "id").cloned().collect()))
}

pub fn block_state_properties(v: &Nbt) -> Vec<(String, String)> {
    let Nbt::Compound(fields) = v else { return Vec::new() };
    fields.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect()
}

pub fn block_state_properties_nbt(properties: &[(&str, &str)]) -> Nbt {
    compound(&properties.iter().map(|(k, v)| (*k, Nbt::String(v.to_string()))).collect::<Vec<_>>())
}

#[derive(Clone, Debug, PartialEq)]
pub struct WrittenBookContent {
    pub title: String,
    pub author: String,
    pub generation: i32,
    pub pages: Vec<Nbt>,
    pub resolved: bool,
}

impl WrittenBookContent {
    pub fn from_nbt(v: &Nbt) -> WrittenBookContent {
        let raw = |f: Option<&Nbt>| f.and_then(|f| f.get("raw")).cloned();
        WrittenBookContent {
            title: raw(v.get("title")).and_then(|t| t.as_str().map(str::to_string)).unwrap_or_default(),
            author: string(v.get("author")).unwrap_or_default(),
            generation: i32_or(v.get("generation"), 0),
            pages: v.get("pages").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(|p| p.get("raw").cloned()).collect(),
            resolved: bool_or(v.get("resolved"), false),
        }
    }

    pub fn resolve_for_item(item_stack: &mut crate::world::item::ItemStack) -> bool {
        let Some(Nbt::Compound(content)) = item_stack.get(super::WRITTEN_BOOK_CONTENT) else { return false };
        if bool_or(content.iter().find(|(k, _)| k == "resolved").map(|(_, v)| v), false) {
            return false;
        }
        let mut resolved_content = content.clone();
        resolved_content.retain(|(k, _)| k != "resolved");
        resolved_content.push(("resolved".into(), Nbt::Byte(1)));
        item_stack.set(super::WRITTEN_BOOK_CONTENT, Some(Nbt::Compound(resolved_content)));
        true
    }
}

pub fn writable_book_pages(v: &Nbt) -> Vec<String> {
    v.get("pages").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(|p| p.get("raw").and_then(Nbt::as_str).map(str::to_string)).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArmorTrim {
    pub material: Nbt,
    pub pattern: Nbt,
}

impl ArmorTrim {
    pub fn from_nbt(v: &Nbt) -> ArmorTrim {
        ArmorTrim { material: v.get("material").cloned().unwrap_or(Nbt::String(String::new())), pattern: v.get("pattern").cloned().unwrap_or(Nbt::String(String::new())) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FireworkExplosion {
    pub shape: String,
    pub colors: Vec<i32>,
    pub fade_colors: Vec<i32>,
    pub has_trail: bool,
    pub has_twinkle: bool,
}

impl FireworkExplosion {
    pub fn from_nbt(v: &Nbt) -> FireworkExplosion {
        let ints = |f: Option<&Nbt>| f.map(Nbt::as_list).unwrap_or(&[]).iter().map(as_i32).collect();
        FireworkExplosion {
            shape: string(v.get("shape")).unwrap_or_else(|| "small_ball".into()),
            colors: ints(v.get("colors")),
            fade_colors: ints(v.get("fade_colors")),
            has_trail: bool_or(v.get("has_trail"), false),
            has_twinkle: bool_or(v.get("has_twinkle"), false),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fireworks {
    pub flight_duration: i32,
    pub explosions: Vec<FireworkExplosion>,
}

impl Fireworks {
    pub fn from_nbt(v: &Nbt) -> Fireworks {
        Fireworks {
            flight_duration: v.get("flight_duration").map_or(0, |d| as_i64(d) as u8 as i32),
            explosions: v.get("explosions").map(Nbt::as_list).unwrap_or(&[]).iter().map(FireworkExplosion::from_nbt).collect(),
        }
    }
}

pub fn banner_patterns(v: &Nbt) -> Vec<(Nbt, String)> {
    v.as_list().iter().map(|l| (l.get("pattern").cloned().unwrap_or(Nbt::String(String::new())), string(l.get("color")).unwrap_or_default())).collect()
}

pub fn profile(v: &Nbt) -> (Option<String>, Option<u128>) {
    let id = match v.get("id") {
        Some(Nbt::IntArray(ints)) if ints.len() == 4 => Some(ints.iter().fold(0u128, |acc, &i| (acc << 32) | i as u32 as u128)),
        _ => None,
    };
    (string(v.get("name")), id)
}

pub fn rarity(v: &Nbt) -> Rarity {
    v.as_str().and_then(|n| Rarity::VALUES.into_iter().find(|r| r.name() == n)).unwrap_or(Rarity::Common)
}

pub fn text(v: &Nbt) -> Component {
    Component::from_nbt(v)
}

pub fn dyed_color(v: &Nbt) -> i32 {
    as_i32(v)
}

pub fn apply_dyes(current_dye: Option<i32>, dyes: &[crate::world::item::dye_color::DyeColor]) -> i32 {
    use crate::util::argb;
    let (mut red_total, mut green_total, mut blue_total, mut intensity_total, mut color_count) = (0, 0, 0, 0, 0);
    let colors = current_dye.into_iter().chain(dyes.iter().map(|dye| dye.texture_diffuse_color()));
    for color in colors {
        let (red, green, blue) = (argb::red(color), argb::green(color), argb::blue(color));
        intensity_total += red.max(green.max(blue));
        red_total += red;
        green_total += green;
        blue_total += blue;
        color_count += 1;
    }
    let red = red_total / color_count;
    let green = green_total / color_count;
    let blue = blue_total / color_count;
    let average_intensity = intensity_total as f32 / color_count as f32;
    let result_intensity = red.max(green.max(blue)) as f32;
    let red = (red as f32 * average_intensity / result_intensity) as i32;
    let green = (green as f32 * average_intensity / result_intensity) as i32;
    let blue = (blue as f32 * average_intensity / result_intensity) as i32;
    argb::color(0, red, green, blue)
}
