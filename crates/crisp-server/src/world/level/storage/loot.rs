use std::collections::HashMap;
use std::sync::OnceLock;

use crisp_data::{BlockState, CONTEXT_FLOAT_PROVIDERS, CONTEXT_INT_PROVIDERS, Item, LOOT_TABLES, PREDICATES, tag};
use rand::{Rng, RngExt};
use serde_json::Value;

use crate::commands::arguments::parse_identifier;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::BlockEntity;
use crate::world::item::component::values::{HolderSet, ItemEnchantments};
use crate::world::item::component::{self as dc, DataComponentPatch, DataComponentType, codec, json_to_nbt};

#[derive(Default)]
pub struct LootContext<'a> {
    pub block_state: Option<BlockState>,
    pub tool: Option<&'a ItemStack>,
    pub explosion_radius: Option<f32>,
    pub block_entity: Option<&'a BlockEntity>,
    pub this_entity: Option<LootEntity>,
    pub attacking_entity: Option<LootEntity>,
    pub direct_attacking_entity: Option<LootEntity>,
    pub last_damage_player: Option<LootEntity>,
}

#[derive(Default)]
pub struct LootEntity {
    pub living: bool,
    pub on_fire: bool,
    pub baby: bool,
    pub mainhand: Option<ItemStack>,
    pub chicken_variant: Option<&'static str>,
}

fn parsed(source: &'static [(&'static str, &'static str)], cell: &'static OnceLock<HashMap<&'static str, Value>>) -> &'static HashMap<&'static str, Value> {
    cell.get_or_init(|| source.iter().map(|&(name, json)| (name, serde_json::from_str(json).expect("vanilla JSON"))).collect())
}

fn loot_table(name: &str) -> Option<&'static Value> {
    static TABLES: OnceLock<HashMap<&'static str, Value>> = OnceLock::new();
    parsed(LOOT_TABLES, &TABLES).get(name)
}

fn predicate(name: &str) -> Option<&'static Value> {
    static PREDICATE_FILES: OnceLock<HashMap<&'static str, Value>> = OnceLock::new();
    parsed(PREDICATES, &PREDICATE_FILES).get(name)
}

pub fn generate(table: &str, ctx: &LootContext, rng: &mut impl Rng) -> Vec<ItemStack> {
    let mut out = Vec::new();
    if let Some(table) = loot_table(table) {
        roll_table(table, ctx, rng, &mut out);
    }
    out
}

fn roll_table(table: &Value, ctx: &LootContext, rng: &mut impl Rng, out: &mut Vec<ItemStack>) {
    let start = out.len();
    for pool in table["pools"].as_array().into_iter().flatten() {
        if !condition(&pool["condition"], ctx, rng) {
            continue;
        }
        let rolls = int(&pool["rolls"], rng);
        for _ in 0..rolls {
            let before = out.len();
            if let Some(entry) = choose(pool["entries"].as_array().unwrap(), ctx, rng) {
                generate_entry(entry, ctx, rng, out);
            }
            apply_modifiers(&pool["modifier"], &mut out[before..], ctx, rng);
        }
    }
    apply_modifiers(&table["modifier"], &mut out[start..], ctx, rng);
    out.retain(|s| s.count > 0);
}

enum Candidate<'a> {
    Entry(&'a Value),
    TagItem(&'a Value, i32),
}

fn choose<'a>(entries: &'a [Value], ctx: &LootContext, rng: &mut impl Rng) -> Option<Candidate<'a>> {
    let mut candidates = Vec::new();
    for entry in entries {
        expand(entry, ctx, rng, &mut candidates);
    }
    let total: i32 = candidates.iter().map(|(w, _)| *w).sum();
    if total <= 0 {
        return None;
    }
    let mut pick = rng.random_range(0..total);
    for (weight, candidate) in candidates {
        if pick < weight {
            return Some(candidate);
        }
        pick -= weight;
    }
    None
}

fn expand<'a>(entry: &'a Value, ctx: &LootContext, rng: &mut impl Rng, out: &mut Vec<(i32, Candidate<'a>)>) -> bool {
    if !condition(&entry["condition"], ctx, rng) {
        return false;
    }
    let weight = entry["weight"].as_i64().unwrap_or(1) as i32;
    let children = entry["children"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    match entry["type"].as_str().unwrap_or("") {
        "minecraft:alternatives" => children.iter().any(|c| expand(c, ctx, rng, out)),
        "minecraft:group" => {
            children.iter().for_each(|c| {
                expand(c, ctx, rng, out);
            });
            true
        }
        "minecraft:sequence" => {
            for child in children {
                if !expand(child, ctx, rng, out) {
                    break;
                }
            }
            true
        }
        "minecraft:tag" if entry["expand"] == true => {
            for &item in tag("minecraft:item", entry["name"].as_str().unwrap_or("")) {
                out.push((weight, Candidate::TagItem(entry, item)));
            }
            true
        }
        _ => {
            out.push((weight, Candidate::Entry(entry)));
            true
        }
    }
}

fn generate_entry(candidate: Candidate, ctx: &LootContext, rng: &mut impl Rng, out: &mut Vec<ItemStack>) {
    let (entry, start) = (match &candidate {
        Candidate::Entry(e) | Candidate::TagItem(e, _) => *e,
    }, out.len());
    match (candidate, entry["type"].as_str().unwrap_or("")) {
        (Candidate::TagItem(_, item), _) => out.push(ItemStack::new(Item(item as u16), 1)),
        (_, "minecraft:item") => out.push(ItemStack::new(Item::by_name(entry["name"].as_str().unwrap()).expect("loot item"), 1)),
        (_, "minecraft:tag") => {
            for &item in tag("minecraft:item", entry["name"].as_str().unwrap_or("")) {
                out.push(ItemStack::new(Item(item as u16), 1));
            }
        }
        (_, "minecraft:loot_table") => match &entry["value"] {
            Value::String(name) => {
                if let Some(table) = loot_table(name) {
                    roll_table(table, ctx, rng, out);
                }
            }
            inline @ Value::Object(_) => roll_table(inline, ctx, rng, out),
            _ => {}
        },
        _ => {}
    }
    apply_modifiers(&entry["modifier"], &mut out[start..], ctx, rng);
}

fn apply_modifiers(modifiers: &Value, stacks: &mut [ItemStack], ctx: &LootContext, rng: &mut impl Rng) {
    let list = match modifiers {
        Value::Array(list) => list.as_slice(),
        Value::Object(_) => std::slice::from_ref(modifiers),
        _ => return,
    };
    for modifier in list {
        for stack in stacks.iter_mut() {
            if condition(&modifier["condition"], ctx, rng) {
                modify(modifier, stack, ctx, rng);
            }
        }
    }
}

fn modify(m: &Value, stack: &mut ItemStack, ctx: &LootContext, rng: &mut impl Rng) {
    match m["type"].as_str().unwrap_or("") {
        "minecraft:set_count" => {
            let count = int(&m["count"], rng);
            stack.count = if m["add"] == true { stack.count + count } else { count };
        }
        "minecraft:limit_count" => {
            let limit = &m["limit"];
            if !limit["min"].is_null() {
                stack.count = stack.count.max(int(&limit["min"], rng));
            }
            if !limit["max"].is_null() {
                stack.count = stack.count.min(int(&limit["max"], rng));
            }
        }
        "minecraft:explosion_decay" => {
            if let Some(radius) = ctx.explosion_radius {
                let keep = 1.0 / radius;
                stack.count = (0..stack.count).filter(|_| rng.random::<f32>() <= keep).count() as i32;
            }
        }
        "minecraft:apply_bonus" => {
            let level = enchantment_level(m["enchantment"].as_str().unwrap_or(""), ctx.tool);
            let parameters = &m["parameters"];
            match m["formula"].as_str().unwrap_or("") {
                "minecraft:binomial_with_bonus_count" => {
                    let extra = parameters["extra"].as_i64().unwrap_or(0) as i32;
                    let p = parameters["probability"].as_f64().unwrap_or(0.0) as f32;
                    stack.count += (0..level + extra).filter(|_| rng.random::<f32>() < p).count() as i32;
                }
                "minecraft:ore_drops" if level > 0 => {
                    let bonus = (rng.random_range(0..level + 2) - 1).max(0);
                    stack.count *= bonus + 1;
                }
                "minecraft:uniform_bonus_count" => {
                    let multiplier = parameters["bonusMultiplier"].as_i64().unwrap_or(0) as i32;
                    stack.count += rng.random_range(0..multiplier * level + 1);
                }
                _ => {}
            }
        }
        "minecraft:set_components" => {
            if let Ok(patch) = DataComponentPatch::from_nbt(&json_to_nbt(&m["components"])) {
                let _ = stack.apply_components_and_validate(&patch);
            }
        }
        "minecraft:set_damage" if stack.is_damageable_item() => {
            let max_damage = stack.max_damage();
            let base = if m["add"] == true { 1.0 - stack.damage_value() as f32 / max_damage as f32 } else { 0.0 };
            let pct = 1.0 - (float(&m["damage"], rng) + base).clamp(0.0, 1.0);
            stack.set_damage_value((pct * max_damage as f32).floor() as i32);
        }
        "minecraft:copy_components" if m["source"] == "block_entity" => {
            if let Some(entity) = ctx.block_entity {
                let listed = |key: &str| -> Option<Vec<DataComponentType>> {
                    m[key].as_array().map(|l| l.iter().filter_map(|t| t.as_str().and_then(parse_identifier)).filter_map(|t| DataComponentType::by_name(&t)).collect())
                };
                let (include, exclude) = (listed("include"), listed("exclude"));
                for (ty, value) in entity.collect_components() {
                    if include.as_ref().is_none_or(|i| i.contains(&ty)) && exclude.as_ref().is_none_or(|e| !e.contains(&ty))
                        && let Some(value) = value
                    {
                        stack.set(ty, Some(value));
                    }
                }
            }
        }
        "minecraft:copy_name" if m["source"] == "block_entity" => {
            if let Some(name) = ctx.block_entity.and_then(|e| e.container_base()).and_then(|b| b.name.clone()) {
                stack.set(dc::CUSTOM_NAME, Some(name));
            }
        }
        "minecraft:enchanted_count_increase" => {
            if let Some(killer) = ctx.attacking_entity.as_ref().filter(|e| e.living) {
                let level = enchantment_level(m["enchantment"].as_str().unwrap_or(""), killer.mainhand.as_ref());
                if level != 0 {
                    let addition = level as f32 * float(&m["count"], rng);
                    stack.grow(addition.round() as i32);
                    let limit = m["limit"].as_i64().unwrap_or(0) as i32;
                    if limit > 0 {
                        stack.limit_size(limit);
                    }
                }
            }
        }
        "minecraft:furnace_smelt" if !stack.is_empty() => {
            use crate::world::item::crafting::{CookingRecipeType, cooking_recipes, get_recipe_for};
            match get_recipe_for(CookingRecipeType::Smelting, stack, None).map(|i| cooking_recipes()[i].assemble()).filter(|r| !r.is_empty()) {
                Some(result) => {
                    let new_count = if m["use_input_count"] == false { 1 } else { stack.count } * result.count;
                    *stack = result.copy_with_count(new_count.min(result.max_stack_size()));
                }
                None => warn!("Couldn't smelt {} {} because there is no smelting recipe", stack.count, stack.item.name()),
            }
        }
        "minecraft:copy_state" => {
            if let Some(state) = ctx.block_state {
                let mut properties = match stack.get(dc::BLOCK_STATE) {
                    Some(crisp_protocol::nbt::Nbt::Compound(fields)) => fields.clone(),
                    _ => Vec::new(),
                };
                for name in m["properties"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    if let Some(value) = state.property_value_by_name(name) {
                        properties.retain(|(k, _)| k != name);
                        properties.push((name.to_string(), crisp_protocol::nbt::Nbt::String(value.to_string())));
                    }
                }
                stack.set(dc::BLOCK_STATE, Some(crisp_protocol::nbt::Nbt::Compound(properties)));
            }
        }
        _ => {}
    }
}

pub fn fill(level: &mut ServerLevel, container: ContainerRef, mut items: Vec<ItemStack>, random: &mut impl crisp_core::random::RandomSource) {
    use crisp_core::mth;
    let mut available: Vec<usize> = (0..level.container_size(container)).filter(|&i| level.get_item(container, i).is_empty()).collect();
    crate::util::shuffle(&mut available, random);
    let mut splittable = Vec::new();
    items.retain(|stack| {
        if stack.is_empty() {
            false
        } else if stack.count > 1 {
            splittable.push(stack.clone());
            false
        } else {
            true
        }
    });
    while available.len() as i64 - items.len() as i64 - splittable.len() as i64 > 0 && !splittable.is_empty() {
        let mut stack: ItemStack = splittable.remove(mth::next_int(random, 0, splittable.len() as i32 - 1) as usize);
        let remove = mth::next_int(random, 1, stack.count / 2);
        let copy = stack.split(remove);
        if stack.count > 1 && random.next_boolean() {
            splittable.push(stack);
        } else {
            items.push(stack);
        }
        if copy.count > 1 && random.next_boolean() {
            splittable.push(copy);
        } else {
            items.push(copy);
        }
    }
    items.extend(splittable);
    crate::util::shuffle(&mut items, random);
    for stack in items {
        let Some(slot) = available.pop() else {
            warn!("Tried to over-fill a container");
            return;
        };
        level.set_item(container, slot, stack);
    }
}

pub fn context_int(v: &Value, ctx: &LootContext) -> Option<i64> {
    static INTS: OnceLock<HashMap<&'static str, Value>> = OnceLock::new();
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(name) => {
            let id = parse_identifier(name)?;
            context_int(parsed(CONTEXT_INT_PROVIDERS, &INTS).get(id.as_str())?, ctx)
        }
        Value::Object(o) => {
            let binary = |f: fn(i32, i32) -> Option<i32>| Some(f(context_int(&v["left"], ctx)? as i32, context_int(&v["right"], ctx)? as i32)? as i64);
            match o.get("type").and_then(Value::as_str).unwrap_or("") {
                "minecraft:constant" => v["value"].as_i64(),
                "minecraft:add" => binary(|a, b| Some(a.wrapping_add(b))),
                "minecraft:sub" => binary(|a, b| Some(a.wrapping_sub(b))),
                "minecraft:mul" => binary(|a, b| Some(a.wrapping_mul(b))),
                "minecraft:div" => binary(|a, b| Some(if b == 0 { 0 } else { a.wrapping_div(b) })),
                "minecraft:mod" => binary(|a, b| Some(if b == 0 { 0 } else { a.wrapping_rem(b) })),
                "minecraft:min" => binary(|a, b| Some(a.min(b))),
                "minecraft:max" => binary(|a, b| Some(a.max(b))),
                "minecraft:negate" => context_int(&v["value"], ctx).map(|a| (a as i32).wrapping_neg() as i64),
                "minecraft:abs" => context_int(&v["value"], ctx).map(|a| (a as i32).wrapping_abs() as i64),
                "minecraft:conditional" => {
                    let mut rng = rand::rng();
                    let branch = if condition(&v["condition"], ctx, &mut rng) { &v["on_true"] } else { &v["on_false"] };
                    context_int(branch, ctx)
                }
                _ => Some(0),
            }
        }
        _ => None,
    }
}

pub fn context_float(v: &Value, ctx: &LootContext) -> Option<f32> {
    static FLOATS: OnceLock<HashMap<&'static str, Value>> = OnceLock::new();
    match v {
        Value::Number(n) => n.as_f64().map(|f| f as f32),
        Value::String(name) => {
            let id = parse_identifier(name)?;
            context_float(parsed(CONTEXT_FLOAT_PROVIDERS, &FLOATS).get(id.as_str())?, ctx)
        }
        Value::Object(o) => match o.get("type").and_then(Value::as_str).unwrap_or("") {
            "minecraft:constant" => v["value"].as_f64().map(|f| f as f32),
            "minecraft:conditional" => {
                let mut rng = rand::rng();
                let branch = if condition(&v["condition"], ctx, &mut rng) { &v["on_true"] } else { &v["on_false"] };
                context_float(branch, ctx)
            }
            _ => Some(0.0),
        },
        _ => None,
    }
}

pub fn resolvable_int(value: Option<&crisp_protocol::nbt::Nbt>, ctx: &LootContext, default: i32) -> i32 {
    match value {
        Some(crisp_protocol::nbt::Nbt::String(id)) => context_int(&Value::String(id.clone()), ctx).map_or(default, |v| v as i32),
        Some(other) => other.as_i64().map_or(default, |v| v as i32),
        None => default,
    }
}

pub fn resolvable_float(value: Option<&crisp_protocol::nbt::Nbt>, ctx: &LootContext, default: f32) -> f32 {
    match value {
        Some(crisp_protocol::nbt::Nbt::String(id)) => context_float(&Value::String(id.clone()), ctx).unwrap_or(default),
        Some(other) => other.as_f64().map_or(default, |v| v as f32),
        None => default,
    }
}

fn enchantment_level(enchantment: &str, tool: Option<&ItemStack>) -> i32 {
    let id = parse_identifier(enchantment).unwrap_or_default();
    tool.map_or(0, |t| t.enchantments().level(&id))
}

fn condition(c: &Value, ctx: &LootContext, rng: &mut impl Rng) -> bool {
    match c {
        Value::Null => true,
        Value::String(name) => predicate(name).is_some_and(|p| condition(p, ctx, rng)),
        Value::Array(all) => all.iter().all(|c| condition(c, ctx, rng)),
        Value::Object(_) => match c["type"].as_str().unwrap_or("") {
            "minecraft:inverted" => !condition(&c["term"], ctx, rng),
            "minecraft:all_of" => c["terms"].as_array().into_iter().flatten().all(|t| condition(t, ctx, rng)),
            "minecraft:any_of" => c["terms"].as_array().into_iter().flatten().any(|t| condition(t, ctx, rng)),
            "minecraft:survives_explosion" => ctx.explosion_radius.is_none_or(|r| rng.random::<f32>() <= 1.0 / r),
            "minecraft:random_chance" => rng.random::<f32>() < float(&c["chance"], rng),
            "minecraft:random_chance_with_enchanted_bonus" => rng.random::<f32>() < float(&c["unenchanted_chance"], rng),
            "minecraft:table_bonus" => {
                let chances = c["chances"].as_array().map(Vec::as_slice).unwrap_or(&[]);
                let level = enchantment_level(c["enchantment"].as_str().unwrap_or(""), ctx.tool).max(0) as usize;
                let chance = chances.get(level.min(chances.len().saturating_sub(1))).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                rng.random::<f32>() < chance
            }
            "minecraft:match_block" => ctx.block_state.is_some_and(|s| match_block(c, s)),
            "minecraft:match_tool" => match_tool(&c["predicate"], ctx.tool),
            "minecraft:killed_by_player" => ctx.last_damage_player.is_some(),
            "minecraft:entity_properties" => {
                let entity = match c["entity"].as_str().unwrap_or("") {
                    "this" => ctx.this_entity.as_ref(),
                    "attacker" => ctx.attacking_entity.as_ref(),
                    "direct_attacker" => ctx.direct_attacking_entity.as_ref(),
                    "attacking_player" => ctx.last_damage_player.as_ref(),
                    _ => None,
                };
                match &c["predicate"] {
                    Value::Null => true,
                    predicate => entity.is_some_and(|e| entity_predicate_matches(predicate, e)),
                }
            }
            _ => false,
        },
        _ => false,
    }
}

fn entity_predicate_matches(predicate: &Value, entity: &LootEntity) -> bool {
    predicate.as_object().into_iter().flatten().all(|(key, value)| match key.as_str() {
        "minecraft:flags" => value.as_object().into_iter().flatten().all(|(flag, expected)| match flag.as_str() {
            "is_on_fire" => expected.as_bool() == Some(entity.on_fire),
            "is_baby" => !entity.living || expected.as_bool() == Some(entity.baby),
            _ => false,
        }),
        "minecraft:equipment" => entity.living && value.as_object().into_iter().flatten().all(|(slot, item)| match slot.as_str() {
            "mainhand" => item_predicate_matches(item, entity.mainhand.as_ref().unwrap_or(&ItemStack::empty())),
            _ => false,
        }),
        "minecraft:components" => value.as_object().into_iter().flatten().all(|(component, expected)| match component.as_str() {
            "minecraft:chicken/variant" => entity.chicken_variant.is_some_and(|v| expected.as_str() == Some(v)),
            _ => false,
        }),
        _ => false,
    })
}

fn match_block(c: &Value, state: BlockState) -> bool {
    let block = state.block();
    if !c["blocks"].is_null() && !holder_set_contains(&c["blocks"], "minecraft:block", block.0 as i32, block.name()) {
        return false;
    }
    c["state"].as_object().into_iter().flatten().all(|(property, expected)| {
        let Some(value) = state.property_value_by_name(property) else { return false };
        match expected {
            Value::String(s) => value == s,
            Value::Object(range) => {
                let Ok(v) = value.parse::<i64>() else { return false };
                range.get("min").and_then(Value::as_i64).is_none_or(|min| v >= min)
                    && range.get("max").and_then(Value::as_i64).is_none_or(|max| v <= max)
            }
            _ => false,
        }
    })
}

fn match_tool(predicate: &Value, tool: Option<&ItemStack>) -> bool {
    let Some(tool) = tool.filter(|t| !t.is_empty()) else { return false };
    item_predicate_matches(predicate, tool)
}

fn nbt_to_json(nbt: &crisp_protocol::nbt::Nbt) -> Value {
    use crisp_protocol::nbt::Nbt;
    match nbt {
        Nbt::Byte(v) => Value::from(*v),
        Nbt::Short(v) => Value::from(*v),
        Nbt::Int(v) => Value::from(*v),
        Nbt::Long(v) => Value::from(*v),
        Nbt::Float(v) => Value::from(*v),
        Nbt::Double(v) => Value::from(*v),
        Nbt::String(s) => Value::String(s.clone()),
        Nbt::ByteArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::IntArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::LongArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::List(items) => Value::Array(items.iter().map(nbt_to_json).collect()),
        Nbt::Compound(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), nbt_to_json(v))).collect()),
    }
}

pub fn item_predicate_nbt_matches(predicate: &crisp_protocol::nbt::Nbt, stack: &ItemStack) -> bool {
    item_predicate_matches(&nbt_to_json(predicate), stack)
}

fn item_predicate_matches(predicate: &Value, tool: &ItemStack) -> bool {
    let items = &predicate["items"];
    let name = tool.get_item().name();
    if !items.is_null() && !holder_set_contains(items, "minecraft:item", tool.get_item().0 as i32, name) {
        return false;
    }
    if !predicate["count"].is_null() && !bounds_match(&predicate["count"], tool.count) {
        return false;
    }
    for (id, expected) in predicate["components"].as_object().into_iter().flatten() {
        let Some(ty) = parse_identifier(id).and_then(|id| DataComponentType::by_name(&id)) else { return false };
        match ty.decode(&json_to_nbt(expected)) {
            Ok(expected) if tool.get(ty) == Some(&expected) => {}
            _ => return false,
        }
    }
    predicate["predicates"].as_object().into_iter().flatten().all(|(id, value)| {
        let id = parse_identifier(id).unwrap_or_default();
        match id.as_str() {
            "minecraft:enchantments" => tool.get(dc::ENCHANTMENTS).is_some_and(|e| enchantments_match(value, &ItemEnchantments::from_nbt(e))),
            "minecraft:stored_enchantments" => tool.get(dc::STORED_ENCHANTMENTS).is_some_and(|e| enchantments_match(value, &ItemEnchantments::from_nbt(e))),
            "minecraft:damage" => tool.get(dc::DAMAGE).is_some_and(|damage| {
                let damage = codec::as_i32(damage);
                (value["durability"].is_null() || bounds_match(&value["durability"], tool.max_damage() - damage))
                    && (value["damage"].is_null() || bounds_match(&value["damage"], damage))
            }),
            other => DataComponentType::by_name(other).is_some_and(|ty| tool.has(ty)),
        }
    })
}

fn enchantments_match(predicates: &Value, enchantments: &ItemEnchantments) -> bool {
    predicates.as_array().into_iter().flatten().all(|p| {
        let levels = &p["levels"];
        match &p["enchantments"] {
            Value::Null if levels.is_null() => !enchantments.is_empty(),
            Value::Null => enchantments.enchantments.iter().any(|(_, level)| bounds_match(levels, *level)),
            set => {
                let set = HolderSet::from_nbt(&json_to_nbt(set));
                let names: Vec<String> = match &set {
                    HolderSet::Direct(names) => names.iter().filter_map(|n| parse_identifier(n)).collect(),
                    HolderSet::Tag(tag) => {
                        let entries = codec::registry("minecraft:enchantment");
                        crate::commands::arguments::registry_tag("minecraft:enchantment", tag)
                            .unwrap_or(&[])
                            .iter()
                            .filter_map(|&i| entries.get(i as usize).map(|e| e.to_string()))
                            .collect()
                    }
                };
                names.iter().any(|name| {
                    let level = enchantments.level(name);
                    level != 0 && (levels.is_null() || bounds_match(levels, level))
                })
            }
        }
    })
}

fn bounds_match(bounds: &Value, value: i32) -> bool {
    match bounds {
        Value::Number(n) => n.as_i64() == Some(value as i64),
        Value::Object(range) => {
            range.get("min").and_then(Value::as_i64).is_none_or(|min| value as i64 >= min) && range.get("max").and_then(Value::as_i64).is_none_or(|max| value as i64 <= max)
        }
        _ => true,
    }
}

fn holder_set_contains(set: &Value, registry: &str, id: i32, name: &str) -> bool {
    match set {
        Value::String(s) => match s.strip_prefix('#') {
            Some(t) => tag(registry, t).contains(&id),
            None => s == name,
        },
        Value::Array(list) => list.iter().any(|v| v.as_str() == Some(name)),
        _ => false,
    }
}

fn int(v: &Value, rng: &mut impl Rng) -> i32 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0).floor() as i32,
        Value::Object(o) => match o.get("type").and_then(Value::as_str).unwrap_or("minecraft:uniform") {
            "minecraft:uniform" => {
                let (min, max) = (int(&v["min"], rng), int(&v["max"], rng));
                if max <= min { min } else { rng.random_range(min..=max) }
            }
            "minecraft:binomial" => {
                let n = int(&v["n"], rng);
                let p = float(&v["p"], rng);
                (0..n).filter(|_| rng.random::<f32>() < p).count() as i32
            }
            "minecraft:constant" => int(&v["value"], rng),
            _ => 0,
        },
        _ => 0,
    }
}

fn float(v: &Value, rng: &mut impl Rng) -> f32 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0) as f32,
        Value::Object(o) => match o.get("type").and_then(Value::as_str).unwrap_or("minecraft:uniform") {
            "minecraft:uniform" => {
                let (min, max) = (float(&v["min"], rng), float(&v["max"], rng));
                min + rng.random::<f32>() * (max - min)
            }
            "minecraft:constant" => float(&v["value"], rng),
            _ => int(v, rng) as f32,
        },
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crisp_data::properties::{SLAB_TYPE, SlabType};
    use crisp_data::{blocks, items};

    fn drops(table: &str, state: BlockState) -> Vec<ItemStack> {
        let ctx = LootContext { block_state: Some(state), ..Default::default() };
        generate(table, &ctx, &mut rand::rng())
    }

    #[test]
    fn grass_block_drops_dirt_by_hand() {
        assert_eq!(drops("minecraft:blocks/grass_block", blocks::GRASS_BLOCK.default_state()), [ItemStack::new(items::DIRT, 1)]);
    }

    #[test]
    fn double_slab_drops_two() {
        let double = blocks::OAK_SLAB.default_state().set(SLAB_TYPE, SlabType::Double);
        assert_eq!(drops("minecraft:blocks/oak_slab", double), [ItemStack::new(items::OAK_SLAB, 2)]);
        assert_eq!(drops("minecraft:blocks/oak_slab", blocks::OAK_SLAB.default_state()), [ItemStack::new(items::OAK_SLAB, 1)]);
    }

    fn enchanted_pickaxe(enchantments: &str) -> ItemStack {
        let mut pickaxe = ItemStack::new(items::DIAMOND_PICKAXE, 1);
        let value = dc::ENCHANTMENTS.decode(&crate::nbt::parse_fully(enchantments).unwrap()).unwrap();
        pickaxe.set(dc::ENCHANTMENTS, Some(value));
        pickaxe
    }

    fn drops_with(table: &str, state: BlockState, tool: &ItemStack) -> Vec<ItemStack> {
        let ctx = LootContext { block_state: Some(state), tool: Some(tool), ..Default::default() };
        generate(table, &ctx, &mut rand::rng())
    }

    #[test]
    fn silk_touch_keeps_glass() {
        let state = blocks::GLASS.default_state();
        assert_eq!(drops_with("minecraft:blocks/glass", state, &enchanted_pickaxe("{silk_touch:1}")), [ItemStack::new(items::GLASS, 1)]);
        assert_eq!(drops_with("minecraft:blocks/glass", state, &enchanted_pickaxe("{efficiency:5}")), []);
    }

    #[test]
    fn fortune_multiplies_ore_drops() {
        let pickaxe = enchanted_pickaxe("{fortune:3}");
        let counts: Vec<i32> = (0..200).map(|_| drops_with("minecraft:blocks/diamond_ore", blocks::DIAMOND_ORE.default_state(), &pickaxe)[0].count).collect();
        assert!(counts.iter().all(|c| (1..=4).contains(c)));
        assert!(counts.iter().any(|&c| c > 1));
    }

    #[test]
    fn silk_touched_beehive_keeps_its_honey_level() {
        let state = blocks::BEEHIVE.default_state();
        let drops = drops_with("minecraft:blocks/beehive", state, &enchanted_pickaxe("{silk_touch:1}"));
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].get(dc::BLOCK_STATE), Some(&crate::nbt::parse_fully("{honey_level:\"0\"}").unwrap()));
    }

    #[test]
    fn chickens_lay_the_egg_of_their_variant() {
        for (variant, egg) in [("minecraft:temperate", items::EGG), ("minecraft:warm", items::BROWN_EGG), ("minecraft:cold", items::BLUE_EGG)] {
            let ctx = LootContext { this_entity: Some(LootEntity { living: true, chicken_variant: Some(variant), ..Default::default() }), ..Default::default() };
            assert_eq!(generate("minecraft:gameplay/chicken_lay", &ctx, &mut rand::rng()), [ItemStack::new(egg, 1)]);
        }
    }

    #[test]
    fn burning_chickens_drop_cooked_chicken() {
        let ctx = LootContext { this_entity: Some(LootEntity { living: true, on_fire: true, ..Default::default() }), ..Default::default() };
        let drops = generate("minecraft:entities/chicken", &ctx, &mut rand::rng());
        assert_eq!(drops.last(), Some(&ItemStack::new(items::COOKED_CHICKEN, 1)));
        let ctx = LootContext { this_entity: Some(LootEntity { living: true, ..Default::default() }), ..Default::default() };
        let drops = generate("minecraft:entities/chicken", &ctx, &mut rand::rng());
        assert_eq!(drops.last(), Some(&ItemStack::new(items::CHICKEN, 1)));
    }

    #[test]
    fn looting_adds_to_chicken_drops() {
        let mut sword = ItemStack::new(items::DIAMOND_SWORD, 1);
        sword.set(dc::ENCHANTMENTS, Some(dc::ENCHANTMENTS.decode(&crate::nbt::parse_fully("{looting:3}").unwrap()).unwrap()));
        let killer = LootEntity { living: true, mainhand: Some(sword), ..Default::default() };
        let ctx = LootContext { this_entity: Some(LootEntity { living: true, ..Default::default() }), attacking_entity: Some(killer), ..Default::default() };
        let counts: Vec<i32> = (0..200).map(|_| generate("minecraft:entities/chicken", &ctx, &mut rand::rng()).last().unwrap().count).collect();
        assert!(counts.iter().all(|c| (1..=4).contains(c)));
        assert!(counts.iter().any(|&c| c > 1));
    }

    #[test]
    fn glowstone_drops_two_to_four_dust() {
        for _ in 0..50 {
            let d = drops("minecraft:blocks/glowstone", blocks::GLOWSTONE.default_state());
            assert_eq!(d.len(), 1);
            assert_eq!(d[0].item, items::GLOWSTONE_DUST);
            assert!((2..=4).contains(&d[0].count));
        }
    }
}
