use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde_json::Value;

const SYNCED_REGISTRIES: &[&str] = &[
    "worldgen/biome",
    "chat_type",
    "trim_pattern",
    "trim_material",
    "wolf_variant",
    "wolf_sound_variant",
    "pig_variant",
    "pig_sound_variant",
    "frog_variant",
    "cat_variant",
    "cat_sound_variant",
    "cow_sound_variant",
    "cow_variant",
    "chicken_sound_variant",
    "chicken_variant",
    "zombie_nautilus_variant",
    "painting_variant",
    "sulfur_cube_archetype",
    "dimension_type",
    "damage_type",
    "banner_pattern",
    "enchantment",
    "jukebox_song",
    "instrument",
    "test_environment",
    "test_instance",
    "dialog",
    "world_clock",
    "timeline",
    "decorated_pot_pattern",
    "block_transformer",
    "worldgen/block_state_provider",
];

fn main() -> anyhow::Result<()> {
    let vanilla = match std::env::var_os("CRISP_VANILLA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.vanilla"),
    };
    println!("cargo::rerun-if-env-changed=CRISP_VANILLA_DIR");
    println!("cargo::rerun-if-changed={}", vanilla.join("version.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/blocks.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/block_entity_types.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/fire.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/waxables.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/attributes.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/mob_effects.json").display());
    println!("cargo::rerun-if-changed={}", vanilla.join("extracted/crash_report_nuggets.json").display());
    if !vanilla.join("version.json").exists() {
        bail!("vanilla data not found in {}; run `cargo xtask vanilla` first", vanilla.display());
    }

    let reports = vanilla.join("generated/reports");
    let data = vanilla.join("data");
    let extracted = vanilla.join("extracted");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);

    let mut out = String::new();
    version(&mut out, &read_json(&vanilla.join("version.json"))?);
    packets(&mut out, &read_json(&reports.join("packets.json"))?)?;
    let builtin = builtin_registries(&mut out, &read_json(&reports.join("registries.json"))?)?;
    let properties = read_json(&extracted.join("properties.json"))?;
    let enums = property_types(&properties)?;
    fs::write(out_dir.join("properties.rs"), properties_source(&properties, &enums)?)?;
    property_table(&mut out, &properties, &enums)?;
    blocks(&mut out, &read_json(&extracted.join("blocks.json"))?, &read_json(&reports.join("blocks.json"))?, &properties, &enums, &builtin)?;
    items(&mut out, &read_json(&extracted.join("items.json"))?, &reports.join("minecraft/components/item"), &builtin)?;
    item_components(&mut out, &read_json(&extracted.join("items.json"))?, &reports.join("minecraft/components/item"))?;
    block_entity_types(&mut out, &read_json(&extracted.join("block_entity_types.json"))?, &builtin)?;
    embed_json(&mut out, &data, "loot_table", "LOOT_TABLES")?;
    embed_json(&mut out, &data, "predicate", "PREDICATES")?;
    embed_json(&mut out, &data, "recipe", "RECIPES")?;
    embed_json(&mut out, &data, "enchantment", "ENCHANTMENTS")?;
    embed_json(&mut out, &data, "context_int_provider", "CONTEXT_INT_PROVIDERS")?;
    embed_json(&mut out, &data, "context_float_provider", "CONTEXT_FLOAT_PROVIDERS")?;
    embed_json(&mut out, &data, "timeline", "TIMELINES")?;
    embed_json(&mut out, &data, "chicken_variant", "CHICKEN_VARIANTS")?;
    embed_json(&mut out, &data, "chicken_sound_variant", "CHICKEN_SOUND_VARIANTS")?;
    embed_json(&mut out, &data, "worldgen/biome", "BIOMES")?;
    embed_json(&mut out, &data, "worldgen/density_function", "DENSITY_FUNCTIONS")?;
    embed_json(&mut out, &data, "worldgen/noise", "NOISES")?;
    embed_json(&mut out, &data, "worldgen/noise_settings", "NOISE_SETTINGS")?;
    embed_json(&mut out, &data, "worldgen/material_rule", "MATERIAL_RULES")?;
    embed_json(&mut out, &data, "worldgen/material_condition", "MATERIAL_CONDITIONS")?;
    embed_json(&mut out, &data, "worldgen/world_preset", "WORLD_PRESETS")?;
    embed_json(&mut out, &data, "worldgen/multi_noise_biome_source_parameter_list", "BIOME_SOURCE_PARAMETER_LISTS")?;
    embed_json(&mut out, &data, "worldgen/carver", "CARVERS")?;
    embed_json(&mut out, &data, "worldgen/structure_set", "STRUCTURE_SETS")?;
    writeln!(out, "pub static BIOME_PARAMETERS: &[(&str, &str)] = &[")?;
    let parameters = reports.join("biome_parameters");
    for ns in namespaces(&parameters)? {
        for id in list_entries(&ns, &parameters.join(&ns))? {
            let path = parameters.join(&ns).join(format!("{}.json", id.split_once(':').unwrap().1));
            writeln!(out, "({id:?}, include_str!({:?})),", path.canonicalize()?)?;
        }
    }
    writeln!(out, "];")?;
    let synced = synced_registries(&mut out, &data)?;
    dimension_types(&mut out, &data, &synced["minecraft:dimension_type"])?;
    damage_types(&mut out, &data, &synced["minecraft:damage_type"])?;
    tags(&mut out, &data, &builtin, &synced)?;
    let lang = vanilla.join("assets/minecraft/lang/en_us.json").canonicalize()?;
    writeln!(out, "pub static LANGUAGE_EN_US: &str = include_str!({lang:?});")?;
    command_tree(&mut out, &read_json(&extracted.join("commands.json"))?)?;
    game_rules(&mut out, &read_json(&extracted.join("game_rules.json"))?)?;
    fire_odds(&mut out, &read_json(&extracted.join("fire.json"))?, &builtin)?;
    waxables(&mut out, &read_json(&extracted.join("waxables.json"))?, &builtin)?;
    attributes_and_mob_effects(&mut out, &read_json(&extracted.join("attributes.json"))?, &read_json(&extracted.join("mob_effects.json"))?, &builtin)?;
    let nuggets = read_json(&extracted.join("crash_report_nuggets.json"))?;
    let nuggets: Vec<String> = nuggets.as_array().unwrap().iter().map(|n| format!("{:?}", n.as_str().unwrap())).collect();
    writeln!(out, "pub static CRASH_REPORT_NUGGETS: &[&str] = &[{}];", nuggets.join(", "))?;
    fs::write(out_dir.join("generated.rs"), out)?;
    Ok(())
}

fn read_json(path: &Path) -> anyhow::Result<Value> {
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    serde_json::from_str(&text).with_context(|| path.display().to_string())
}

fn const_name(id: &str) -> String {
    let path = id.strip_prefix("minecraft:").unwrap_or(id);
    let mut name: String = path
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn variant_name(constant: &str) -> String {
    constant
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}

fn version(out: &mut String, v: &Value) {
    writeln!(out, "pub const VERSION_NAME: &str = {:?};", v["name"].as_str().unwrap()).unwrap();
    writeln!(out, "pub const PROTOCOL_VERSION: i32 = {};", v["protocol_version"]).unwrap();
    writeln!(out, "pub const VERSION_ID: &str = {:?};", v["id"].as_str().unwrap()).unwrap();
    writeln!(out, "pub const WORLD_VERSION: i32 = {};", v["world_version"]).unwrap();
    writeln!(out, "pub const VERSION_SERIES: &str = {:?};", v["series_id"].as_str().unwrap()).unwrap();
    let pack = &v["pack_version"];
    writeln!(out, "pub const RESOURCE_PACK_VERSION: (i32, i32) = ({}, {});", pack["resource_major"], pack["resource_minor"]).unwrap();
    writeln!(out, "pub const DATA_PACK_VERSION: (i32, i32) = ({}, {});", pack["data_major"], pack["data_minor"]).unwrap();
    writeln!(out, "pub const BUILD_TIME: &str = {:?};", v["build_time"].as_str().unwrap()).unwrap();
    writeln!(out, "pub const VERSION_STABLE: bool = {};", v["stable"]).unwrap();
}

fn packets(out: &mut String, v: &Value) -> anyhow::Result<()> {
    let mut names = Vec::new();
    writeln!(out, "pub mod packets {{")?;
    for (state, directions) in v.as_object().unwrap() {
        writeln!(out, "pub mod {state} {{")?;
        for (direction, packets) in directions.as_object().unwrap() {
            writeln!(out, "pub mod {direction} {{")?;
            for (name, packet) in packets.as_object().unwrap() {
                writeln!(out, "pub const {}: i32 = {};", const_name(name), packet["protocol_id"])?;
                names.push(format!("({state:?}, {direction:?}, {}, {name:?})", packet["protocol_id"]));
            }
            writeln!(out, "}}")?;
        }
        writeln!(out, "}}")?;
    }
    writeln!(out, "}}")?;
    writeln!(out, "pub static PACKET_NAMES: &[(&str, &str, i32, &str)] = &[{}];", names.join(", "))?;
    Ok(())
}

fn builtin_registries(out: &mut String, v: &Value) -> anyhow::Result<HashMap<String, HashMap<String, i32>>> {
    let mut all = HashMap::new();
    writeln!(out, "pub mod registry {{")?;
    for (name, registry) in v.as_object().unwrap() {
        let mut entries: Vec<(&str, i32)> = registry["entries"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, e)| (k.as_str(), e["protocol_id"].as_i64().unwrap() as i32))
            .collect();
        entries.sort_by_key(|&(_, id)| id);
        for (i, &(_, id)) in entries.iter().enumerate() {
            if id != i as i32 {
                bail!("registry {name} has non-contiguous ids");
            }
        }
        writeln!(out, "pub static {}: &[&str] = &{:?};", const_name(name), entries.iter().map(|e| e.0).collect::<Vec<_>>())?;
        all.insert(name.clone(), entries.into_iter().map(|(k, id)| (k.to_owned(), id)).collect());
    }
    writeln!(out, "}}")?;
    writeln!(out, "pub(crate) static BUILTIN_REGISTRIES: &[(&str, &[&str])] = &[")?;
    for name in v.as_object().unwrap().keys() {
        writeln!(out, "({name:?}, registry::{}),", const_name(name))?;
    }
    writeln!(out, "];")?;
    let fluids: &HashMap<String, i32> = &all["minecraft:fluid"];
    for (i, name) in ["empty", "flowing_water", "water", "flowing_lava", "lava"].iter().enumerate() {
        if fluids[&format!("minecraft:{name}")] != i as i32 {
            bail!("the Fluid enum does not follow the fluid registry order");
        }
    }
    Ok(all)
}

fn property_types(properties: &Value) -> anyhow::Result<BTreeMap<String, (String, Vec<String>, Vec<String>)>> {
    let mut types = BTreeMap::new();
    for p in properties.as_array().unwrap() {
        let ty = p["type"].as_str().unwrap();
        let (rust, constants, names): (String, Vec<String>, Vec<String>) = match ty {
            "boolean" | "integer" => continue,
            "Direction" => ("Direction".into(), ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"].map(String::from).into(), ["down", "up", "north", "south", "west", "east"].map(String::from).into()),
            "Direction.Axis" => ("Axis".into(), ["X", "Y", "Z"].map(String::from).into(), ["x", "y", "z"].map(String::from).into()),
            _ => {
                let rust = match ty {
                    "CopperGolemStatueBlock.Pose" => "CopperGolemPose".to_string(),
                    other if other.contains('.') => bail!("property type {other} needs a Rust name"),
                    other => other.to_string(),
                };
                let constants = p["constants"].as_array().unwrap();
                (
                    rust,
                    constants.iter().map(|c| c["constant"].as_str().unwrap().to_string()).collect(),
                    constants.iter().map(|c| c["name"].as_str().unwrap().to_string()).collect(),
                )
            }
        };
        types.insert(ty.to_string(), (rust, constants, names));
    }
    Ok(types)
}

fn property_const(p: &Value) -> anyhow::Result<String> {
    let field = p["field"].as_str().with_context(|| format!("property {} has no static field", p["name"]))?;
    let (owner, name) = field.split_once('.').unwrap();
    Ok(if owner == "BlockStateProperties" { name.to_string() } else { format!("{}_{name}", const_name(owner)) })
}

fn property_ordinals(p: &Value, enums: &BTreeMap<String, (String, Vec<String>, Vec<String>)>) -> anyhow::Result<Vec<u8>> {
    let values = p["values"].as_array().unwrap().iter().map(|v| v.as_str().unwrap());
    values
        .map(|v| match p["type"].as_str().unwrap() {
            "boolean" => Ok((v == "true") as u8),
            "integer" => Ok(v.parse::<u8>()?),
            ty => enums[ty].2.iter().position(|n| n == v).map(|i| i as u8).with_context(|| format!("{v} is not a {ty}")),
        })
        .collect()
}

fn properties_source(properties: &Value, enums: &BTreeMap<String, (String, Vec<String>, Vec<String>)>) -> anyhow::Result<String> {
    let mut out = String::new();
    for (rust, constants, names) in enums.values() {
        if rust == "Direction" || rust == "Axis" {
            continue;
        }
        let variants: Vec<String> = constants.iter().map(|c| variant_name(c)).collect();
        writeln!(out, "#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]\n#[repr(u8)]\npub enum {rust} {{ {} }}", variants.join(", "))?;
        writeln!(out, "impl {rust} {{")?;
        writeln!(out, "pub const VALUES: [{rust}; {}] = [{}];", variants.len(), variants.iter().map(|v| format!("{rust}::{v}")).collect::<Vec<_>>().join(", "))?;
        writeln!(out, "pub const fn name(self) -> &'static str {{ match self {{ {} }} }}", variants.iter().zip(names).map(|(v, n)| format!("{rust}::{v} => {n:?}")).collect::<Vec<_>>().join(", "))?;
        writeln!(out, "pub fn by_name(name: &str) -> Option<{rust}> {{ Self::VALUES.into_iter().find(|v| v.name() == name) }}")?;
        writeln!(out, "}}")?;
        writeln!(
            out,
            "impl PropertyValue for {rust} {{ fn from_ordinal(ordinal: u8) -> {rust} {{ {rust}::VALUES[ordinal as usize] }} fn ordinal(self) -> u8 {{ self as u8 }} }}"
        )?;
    }
    for (id, p) in properties.as_array().unwrap().iter().enumerate() {
        let ty = match p["type"].as_str().unwrap() {
            "boolean" => "BooleanProperty".to_string(),
            "integer" => "IntegerProperty".to_string(),
            ty => format!("EnumProperty<{}>", enums[ty].0),
        };
        writeln!(out, "pub const {}: {ty} = Property::new({id});", property_const(p)?)?;
    }
    Ok(out)
}

fn property_table(out: &mut String, properties: &Value, enums: &BTreeMap<String, (String, Vec<String>, Vec<String>)>) -> anyhow::Result<()> {
    writeln!(out, "pub(crate) static PROPERTIES: &[properties::PropertyData] = &[")?;
    for p in properties.as_array().unwrap() {
        let values: Vec<&str> = p["values"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        writeln!(out, "properties::PropertyData {{ name: {:?}, values: &{values:?}, ordinals: &{:?} }},", p["name"].as_str().unwrap(), property_ordinals(p, enums)?)?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn f32_literal(v: &Value) -> String {
    format!("{:?}", v.as_f64().unwrap() as f32)
}

fn blocks(
    out: &mut String,
    extracted: &Value,
    report: &Value,
    properties: &Value,
    enums: &BTreeMap<String, (String, Vec<String>, Vec<String>)>,
    builtin: &HashMap<String, HashMap<String, i32>>,
) -> anyhow::Result<()> {
    let block_ids = &builtin["minecraft:block"];
    let item_ids = &builtin["minecraft:item"];
    let sound_ids = &builtin["minecraft:sound_event"];
    let fluid_ids = &builtin["minecraft:fluid"];
    let sound = |name: &Value| -> anyhow::Result<i32> { sound_ids.get(name.as_str().unwrap()).copied().with_context(|| format!("unknown sound {name}")) };
    let instruments = &enums["NoteBlockInstrument"].2;
    const PUSH_REACTIONS: &[&str] = &["PUSH_PULL", "PUSH", "POPPED", "IMMOVEABLE", "IGNORE_ENTITY"];

    let mut shapes: Vec<String> = Vec::new();
    let mut shape_index = |boxes: &Value| -> usize {
        let parts: Vec<String> = boxes
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                let c: Vec<String> = b.as_array().unwrap().iter().map(|c| format!("{:?}", c.as_f64().unwrap())).collect();
                format!("AABB {{ min_x: {}, min_y: {}, min_z: {}, max_x: {}, max_y: {}, max_z: {} }}", c[0], c[1], c[2], c[3], c[4], c[5])
            })
            .collect();
        let shape = format!("&[{}]", parts.join(", "));
        match shapes.iter().position(|s| *s == shape) {
            Some(i) => i,
            None => {
                shapes.push(shape);
                shapes.len() - 1
            }
        }
    };

    let mut block_consts = String::new();
    let mut states = Vec::new();
    writeln!(out, "pub(crate) static BLOCKS: &[BlockInfo] = &[")?;
    for (index, block) in extracted.as_array().unwrap().iter().enumerate() {
        let name = block["name"].as_str().unwrap();
        if block_ids.get(name) != Some(&(index as i32)) {
            bail!("extracted block {name} is not at its registry id {index}");
        }
        let block_states = block["states"].as_array().unwrap();
        let first = block_states[0]["id"].as_u64().unwrap();

        let prop_ids: Vec<usize> = block["properties"].as_array().unwrap().iter().map(|p| p.as_u64().unwrap() as usize).collect();
        let counts: Vec<usize> = prop_ids.iter().map(|&id| properties[id]["values"].as_array().unwrap().len()).collect();
        let mut strides = vec![1usize; counts.len()];
        for i in (0..counts.len().saturating_sub(1)).rev() {
            strides[i] = strides[i + 1] * counts[i + 1];
        }
        if counts.iter().product::<usize>() != block_states.len() {
            bail!("{name}: property value counts do not match its {} states", block_states.len());
        }
        for state in report[name]["states"].as_array().unwrap() {
            let offset = (state["id"].as_u64().unwrap() - first) as usize;
            for (i, &id) in prop_ids.iter().enumerate() {
                let p = &properties[id];
                let value = p["values"][offset / strides[i] % counts[i]].as_str().unwrap();
                if state["properties"][p["name"].as_str().unwrap()] != value {
                    bail!("state {} of {name} does not follow the property layout", state["id"]);
                }
            }
        }
        let block_props: Vec<String> = prop_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| format!("BlockProperty {{ id: properties::PropertyId({id}), stride: {}, count: {} }}", strides[i], counts[i]))
            .collect();

        let s = &block["sound"];
        let behaviour = &block["behaviour"];
        writeln!(
            out,
            "BlockInfo {{ name: {name:?}, destroy_time: {}, explosion_resistance: {}, friction: {}, speed_factor: {}, jump_factor: {}, \
             bounce_restitution: {}, fall_distance_reduction: {}, has_collision: {}, dynamic_shape: {}, item: Item({}), first_state: {first}, \
             state_count: {}, default_state: BlockState({}), loot_table: {:?}, sound: SoundType {{ volume: {}, pitch: {}, break_sound: {}, \
             step_sound: {}, place_sound: {}, hit_sound: {}, fall_sound: {} }}, classes: &{:?}, fields: {:?}, properties: &[{}] }},",
            f32_literal(&block["hardness"]),
            f32_literal(&block["blast_resistance"]),
            f32_literal(&block["friction"]),
            f32_literal(&block["speed_factor"]),
            f32_literal(&block["jump_factor"]),
            f32_literal(&behaviour["bounceRestitution"]),
            f32_literal(&behaviour["fallDistanceReduction"]),
            behaviour["hasCollision"].as_bool().unwrap(),
            block["dynamic_shape"].as_bool().unwrap(),
            item_ids[block["item"].as_str().unwrap()],
            block_states.len(),
            block["default_state"],
            block["loot_table"].as_str(),
            f32_literal(&s["volume"]),
            f32_literal(&s["pitch"]),
            sound(&s["break"])?,
            sound(&s["step"])?,
            sound(&s["place"])?,
            sound(&s["hit"])?,
            sound(&s["fall"])?,
            block["classes"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect::<Vec<_>>(),
            block["fields"].to_string(),
            block_props.join(", "),
        )?;
        writeln!(block_consts, "pub const {}: Block = Block({index});", const_name(name))?;

        for state in block_states {
            const FLAGS: &[&str] = &[
                "air",
                "solid",
                "solid_render",
                "can_occlude",
                "replaceable",
                "requires_tool",
                "propagates_skylight",
                "use_shape_for_light_occlusion",
                "random_ticking",
                "redstone_conductor",
                "suffocating",
                "signal_source",
                "analog_output",
                "block_entity",
                "ignited_by_lava",
                "liquid",
                "large_collision",
                "collision_full_block",
                "offset",
            ];
            let mut flags = 0u32;
            for (bit, key) in FLAGS.iter().enumerate() {
                if state[*key] == true {
                    flags |= 1 << bit;
                }
            }
            let fluid = &state["fluid"];
            if fluid["falling"] == true {
                flags |= 1 << 19;
            }
            if state["suffocating_by_tag"] == true {
                flags |= 1 << 20;
            }
            let mut sturdy = 0u32;
            for (bit, v) in state["face_sturdy"].as_array().unwrap().iter().enumerate() {
                if v == true {
                    sturdy |= 1 << bit;
                }
            }
            let push = PUSH_REACTIONS.iter().position(|p| *p == state["push_reaction"]).context("unknown push reaction")?;
            let instrument = instruments.iter().position(|i| *i == state["instrument"]).context("unknown instrument")?;
            states.push(format!(
                "StateData {{ block: {index}, flags: {flags}, light_emission: {}, light_dampening: {}, face_sturdy: {sturdy}, push_reaction: {push}, \
                 map_color: {}, instrument: {instrument}, fluid: {}, fluid_amount: {}, shape: {}, collision: {}, occlusion: {}, support: {}, interaction: {} }},",
                state["light_emission"],
                state["light_dampening"],
                state["map_color"],
                fluid_ids[fluid["type"].as_str().unwrap()],
                fluid["amount"],
                shape_index(&state["shape"]),
                shape_index(&state["collision"]),
                shape_index(&state["occlusion"]),
                shape_index(&state["support"]),
                shape_index(&state["interaction"]),
            ));
            if state["id"].as_u64().unwrap() as usize != states.len() - 1 {
                bail!("block state ids of {name} are not contiguous");
            }
        }
    }
    writeln!(out, "];")?;
    writeln!(out, "pub mod blocks {{ use crate::Block;\n{block_consts}}}")?;
    writeln!(out, "use block::StateData;\npub(crate) static STATES: &[StateData] = &[\n{}\n];", states.join("\n"))?;
    writeln!(out, "use crisp_core::AABB;\npub(crate) static SHAPES: &[VoxelShape] = &[{}];", shapes.join(",\n"))?;
    Ok(())
}

fn items(out: &mut String, extracted: &Value, components_dir: &Path, builtin: &HashMap<String, HashMap<String, i32>>) -> anyhow::Result<()> {
    let item_ids = &builtin["minecraft:item"];
    let block_ids = &builtin["minecraft:block"];
    let mut consts = String::new();
    writeln!(out, "pub(crate) static ITEMS: &[ItemInfo] = &[")?;
    for (index, item) in extracted.as_array().unwrap().iter().enumerate() {
        let name = item["name"].as_str().unwrap();
        if item_ids.get(name) != Some(&(index as i32)) {
            bail!("extracted item {name} is not at its registry id {index}");
        }
        let path = components_dir.join(format!("{}.json", name.strip_prefix("minecraft:").unwrap()));
        let components = &read_json(&path)?["components"];
        let tool = &components["minecraft:tool"];
        let tool = if tool.is_object() {
            let rules: Vec<String> = tool["rules"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let blocks = match &r["blocks"] {
                        Value::String(s) if s.starts_with('#') => format!("ToolRuleBlocks::Tag(&block_tags::{})", const_name(&s[1..])),
                        Value::String(s) => format!("ToolRuleBlocks::Blocks(&[Block({})])", block_ids[s]),
                        Value::Array(list) => format!(
                            "ToolRuleBlocks::Blocks(&[{}])",
                            list.iter().map(|b| format!("Block({})", block_ids[b.as_str().unwrap()])).collect::<Vec<_>>().join(", ")
                        ),
                        other => panic!("unexpected tool rule blocks {other}"),
                    };
                    format!("ToolRule {{ blocks: {blocks}, speed: {:?}, correct_for_drops: {:?} }}", r["speed"].as_f64().map(|s| s as f32), r["correct_for_drops"].as_bool())
                })
                .collect();
            format!(
                "Some(ToolInfo {{ default_mining_speed: {:?}, damage_per_block: {}, rules: &[{}] }})",
                tool["default_mining_speed"].as_f64().unwrap_or(1.0) as f32,
                tool["damage_per_block"].as_i64().unwrap_or(1),
                rules.join(", ")
            )
        } else {
            "None".into()
        };
        let slot = match components["minecraft:equippable"]["slot"].as_str() {
            Some(slot) => format!("Some(EquipmentSlot::{})", variant_name(slot)),
            None => "None".into(),
        };
        let survival = survival_components(components, item_ids, &builtin["minecraft:mob_effect"]).with_context(|| format!("components of {name}"))?;
        writeln!(
            out,
            "ItemInfo {{ name: {name:?}, description_id: {:?}, max_stack_size: {}, max_damage: {}, tool: {tool}, equipment_slot: {slot}, {survival} block: {}, classes: &{:?}, fields: {:?}, rarity: Rarity::{}, components: &{:?} }},",
            components["minecraft:item_name"]["translate"].as_str().context("item_name is not a plain translation")?,
            components["minecraft:max_stack_size"].as_u64().unwrap_or(1),
            components["minecraft:max_damage"].as_i64().unwrap_or(0),
            match item["block"].as_str() {
                Some(b) => format!("Some(Block({}))", block_ids[b]),
                None => "None".into(),
            },
            item["classes"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect::<Vec<_>>(),
            item["fields"].to_string(),
            variant_name(components["minecraft:rarity"].as_str().context("an item without rarity")?),
            components.as_object().unwrap().keys().collect::<Vec<_>>(),
        )?;
        writeln!(consts, "pub const {}: Item = Item({index});", const_name(name))?;
    }
    writeln!(out, "];")?;
    writeln!(out, "pub mod items {{ use crate::Item;\n{consts}}}")?;
    Ok(())
}

fn block_entity_types(out: &mut String, extracted: &Value, builtin: &HashMap<String, HashMap<String, i32>>) -> anyhow::Result<()> {
    let type_ids = &builtin["minecraft:block_entity_type"];
    let block_ids = &builtin["minecraft:block"];
    let mut consts = String::new();
    let mut created = vec![u16::MAX; block_ids.len()];
    writeln!(out, "pub(crate) static BLOCK_ENTITY_TYPES: &[block_entity::BlockEntityTypeInfo] = &[")?;
    for (index, ty) in extracted.as_array().unwrap().iter().enumerate() {
        let name = ty["name"].as_str().unwrap();
        if type_ids.get(name) != Some(&(index as i32)) {
            bail!("extracted block entity type {name} is not at its registry id {index}");
        }
        let blocks: Vec<String> = ty["blocks"].as_array().unwrap().iter().map(|b| format!("Block({})", block_ids[b.as_str().unwrap()])).collect();
        for block in ty["created_by"].as_array().unwrap() {
            created[block_ids[block.as_str().unwrap()] as usize] = index as u16;
        }
        writeln!(
            out,
            "block_entity::BlockEntityTypeInfo {{ name: {name:?}, valid_blocks: &[{}], only_op_can_set_nbt: {} }},",
            blocks.join(", "),
            ty["op_only"].as_bool().unwrap()
        )?;
        writeln!(consts, "pub const {}: BlockEntityType = BlockEntityType({index});", const_name(name))?;
    }
    writeln!(out, "];")?;
    writeln!(out, "pub mod block_entity_types {{ use crate::BlockEntityType;
{consts}}}")?;
    writeln!(out, "pub(crate) static BLOCK_ENTITY_TYPE_OF_BLOCK: &[u16] = &{created:?};")?;
    Ok(())
}

fn item_components(out: &mut String, extracted: &Value, components_dir: &Path) -> anyhow::Result<()> {
    let names: Vec<&str> = extracted.as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    let remainders: Vec<u16> = extracted
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["crafting_remainder"].as_str().map_or(u16::MAX, |r| names.iter().position(|n| *n == r).expect("remainder item") as u16))
        .collect();
    writeln!(out, "pub(crate) static CRAFTING_REMAINDERS: &[u16] = &{remainders:?};")?;
    writeln!(out, "pub static ITEM_COMPONENTS: &[&str] = &[")?;
    for item in extracted.as_array().unwrap() {
        let name = item["name"].as_str().unwrap();
        let path = components_dir.join(format!("{}.json", name.strip_prefix("minecraft:").unwrap()));
        writeln!(out, "{:?},", read_json(&path)?["components"].to_string())?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn survival_components(components: &Value, item_ids: &HashMap<String, i32>, effect_ids: &HashMap<String, i32>) -> anyhow::Result<String> {
    let modifiers: Vec<String> = components["minecraft:attribute_modifiers"]
        .as_array()
        .context("attribute_modifiers is not a list")?
        .iter()
        .map(|m| {
            let operation = match m["operation"].as_str().unwrap() {
                "add_value" => 0,
                "add_multiplied_base" => 1,
                "add_multiplied_total" => 2,
                other => bail!("unknown operation {other}"),
            };
            Ok(format!(
                "ItemAttributeModifier {{ attribute: {:?}, id: {:?}, amount: {:?}, operation: {operation}, slot: EquipmentSlotGroup::{} }}",
                m["type"].as_str().unwrap(),
                m["id"].as_str().unwrap(),
                m["amount"].as_f64().unwrap(),
                variant_name(m["slot"].as_str().unwrap_or("any")),
            ))
        })
        .collect::<anyhow::Result<_>>()?;
    let food = &components["minecraft:food"];
    let food = if food.is_object() {
        format!(
            "Some(FoodProperties {{ nutrition: {}, saturation: {:?}, can_always_eat: {} }})",
            food["nutrition"].as_i64().unwrap(),
            food["saturation"].as_f64().unwrap() as f32,
            food["can_always_eat"].as_bool().unwrap_or(false)
        )
    } else {
        "None".into()
    };
    let consumable = &components["minecraft:consumable"];
    let consumable = if consumable.is_object() {
        format!(
            "Some(Consumable {{ consume_seconds: {:?}, animation: ItemUseAnimation::{}, sound: {:?}, has_consume_particles: {}, on_consume_effects: &[{}] }})",
            consumable["consume_seconds"].as_f64().unwrap_or(1.6) as f32,
            variant_name(consumable["animation"].as_str().unwrap_or("eat")),
            match consumable.get("sound") {
                Some(sound) => sound_name(sound)?,
                None => "minecraft:entity.generic.eat".into(),
            },
            consumable["has_consume_particles"].as_bool().unwrap_or(true),
            consume_effects(&consumable["on_consume_effects"], effect_ids)?
        )
    } else {
        "None".into()
    };
    let use_remainder = match components["minecraft:use_remainder"]["id"].as_str() {
        Some(id) => format!("Some(Item({}))", item_ids[id]),
        None => "None".into(),
    };
    let damage_resistant = match components["minecraft:damage_resistant"]["types"].as_str() {
        Some(tag) => format!("Some(&damage_type_tags::{})", const_name(tag.strip_prefix('#').context("damage_resistant is not a tag")?)),
        None => "None".into(),
    };
    let death_protection = &components["minecraft:death_protection"];
    let death_protection =
        if death_protection.is_object() { format!("Some(&[{}])", consume_effects(&death_protection["death_effects"], effect_ids)?) } else { "None".into() };
    Ok(format!(
        "attribute_modifiers: &[{}], food: {food}, consumable: {consumable}, use_remainder: {use_remainder}, damage_on_hurt: {}, damage_resistant: {damage_resistant}, death_protection: {death_protection},",
        modifiers.join(", "),
        components["minecraft:equippable"]["damage_on_hurt"].as_bool().unwrap_or(true),
    ))
}

fn consume_effects(list: &Value, effect_ids: &HashMap<String, i32>) -> anyhow::Result<String> {
    let mut effects = Vec::new();
    for effect in list.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        effects.push(match effect["type"].as_str().unwrap() {
            "minecraft:apply_effects" => {
                let instances = effect["effects"]
                    .as_array()
                    .context("apply_effects without effects")?
                    .iter()
                    .map(|e| effect_instance(e, effect_ids))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                format!("ConsumeEffect::ApplyEffects {{ effects: &[{}], probability: {:?} }}", instances.join(", "), effect["probability"].as_f64().unwrap_or(1.0) as f32)
            }
            "minecraft:remove_effects" => {
                let ids: Vec<&Value> = match &effect["effects"] {
                    Value::Array(list) => list.iter().collect(),
                    one => vec![one],
                };
                let ids = ids
                    .iter()
                    .map(|id| Ok(format!("MobEffect({})", effect_ids[id.as_str().filter(|s| !s.starts_with('#')).context("remove_effects with a tag")?])))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                format!("ConsumeEffect::RemoveEffects {{ effects: &[{}] }}", ids.join(", "))
            }
            "minecraft:clear_all_effects" => "ConsumeEffect::ClearAllEffects".into(),
            "minecraft:teleport_randomly" => format!("ConsumeEffect::TeleportRandomly {{ diameter: {:?} }}", effect["diameter"].as_f64().unwrap_or(16.0) as f32),
            "minecraft:play_sound" => format!("ConsumeEffect::PlaySound {{ sound: {:?} }}", sound_name(&effect["sound"])?),
            other => bail!("unknown consume effect {other}"),
        });
    }
    Ok(effects.join(", "))
}

fn sound_name(v: &Value) -> anyhow::Result<String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Object(o) => Ok(o["sound_id"].as_str().context("sound without an id")?.to_owned()),
        other => bail!("unexpected sound {other}"),
    }
}

fn effect_instance(v: &Value, effect_ids: &HashMap<String, i32>) -> anyhow::Result<String> {
    if v.get("hidden_effect").is_some() {
        bail!("an effect instance with a hidden effect");
    }
    let show_particles = v["show_particles"].as_bool().unwrap_or(true);
    Ok(format!(
        "EffectInstanceTemplate {{ effect: MobEffect({}), duration: {}, amplifier: {}, ambient: {}, show_particles: {show_particles}, show_icon: {} }}",
        effect_ids[v["id"].as_str().context("an effect without an id")?],
        v["duration"].as_i64().unwrap_or(0),
        v["amplifier"].as_u64().unwrap_or(0),
        v["ambient"].as_bool().unwrap_or(false),
        v["show_icon"].as_bool().unwrap_or(show_particles),
    ))
}

fn attributes_and_mob_effects(out: &mut String, attributes: &Value, effects: &Value, builtin: &HashMap<String, HashMap<String, i32>>) -> anyhow::Result<()> {
    let attribute_ids = &builtin["minecraft:attribute"];
    writeln!(out, "pub static ATTRIBUTES: &[AttributeInfo] = &[")?;
    for (index, a) in attributes.as_array().unwrap().iter().enumerate() {
        let name = a["name"].as_str().unwrap();
        if attribute_ids.get(name) != Some(&(index as i32)) {
            bail!("extracted attribute {name} is not at its registry id {index}");
        }
        writeln!(
            out,
            "AttributeInfo {{ name: {name:?}, description_id: {:?}, default_value: {:?}, min_value: {:?}, max_value: {:?}, syncable: {}, sentiment: Sentiment::{} }},",
            a["description_id"].as_str().unwrap(),
            a["default"].as_f64().unwrap(),
            a["min"].as_f64().unwrap(),
            a["max"].as_f64().unwrap(),
            a["syncable"].as_bool().unwrap(),
            variant_name(a["sentiment"].as_str().unwrap()),
        )?;
    }
    writeln!(out, "];")?;
    let effect_ids = &builtin["minecraft:mob_effect"];
    let mut consts = String::new();
    writeln!(out, "pub static MOB_EFFECTS: &[MobEffectInfo] = &[")?;
    for (index, e) in effects.as_array().unwrap().iter().enumerate() {
        let name = e["name"].as_str().unwrap();
        if effect_ids.get(name) != Some(&(index as i32)) {
            bail!("extracted mob effect {name} is not at its registry id {index}");
        }
        writeln!(consts, "pub const {}: MobEffect = MobEffect({index});", const_name(name))?;
        let modifiers = e["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                let operation = ["add_value", "add_multiplied_base", "add_multiplied_total"].iter().position(|o| *o == m["operation"].as_str().unwrap());
                Ok(format!(
                    "EffectAttributeModifier {{ attribute: {}, id: {:?}, amount: {:?}, operation: {} }}",
                    attribute_ids[m["attribute"].as_str().unwrap()],
                    m["id"].as_str().unwrap(),
                    m["amount"].as_f64().unwrap(),
                    operation.context("unknown operation")?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        writeln!(
            out,
            "MobEffectInfo {{ name: {name:?}, description_id: {:?}, category: MobEffectCategory::{}, color: {}, instantaneous: {}, blend_in_duration_ticks: {}, blend_out_duration_ticks: {}, blend_out_advance_ticks: {}, particle: {:?}, sound_on_added: {:?}, modifiers: &[{}] }},",
            e["description_id"].as_str().unwrap(),
            variant_name(e["category"].as_str().unwrap()),
            e["color"].as_i64().unwrap(),
            e["instantaneous"].as_bool().unwrap(),
            e["blend_in"].as_i64().unwrap(),
            e["blend_out"].as_i64().unwrap(),
            e["blend_out_advance"].as_i64().unwrap(),
            e["particle"].as_str(),
            e["sound_on_added"].as_str(),
            modifiers.join(", "),
        )?;
    }
    writeln!(out, "];")?;
    writeln!(out, "pub mod mob_effects {{ use crate::MobEffect;\n{consts}}}")?;
    Ok(())
}

fn damage_types(out: &mut String, data: &Path, entries: &[String]) -> anyhow::Result<()> {
    let consts: String = entries.iter().enumerate().map(|(i, id)| format!("pub const {}: DamageType = DamageType({i});\n", const_name(id))).collect();
    writeln!(out, "pub mod damage_types {{ use crate::DamageType;\n{consts}}}")?;
    writeln!(out, "pub static DAMAGE_TYPES: &[DamageTypeInfo] = &[")?;
    for id in entries {
        let (ns, path) = id.split_once(':').unwrap();
        let v = read_json(&data.join(ns).join("damage_type").join(format!("{path}.json")))?;
        writeln!(
            out,
            "DamageTypeInfo {{ name: {id:?}, msg_id: {:?}, scaling: DamageScaling::{}, exhaustion: {:?}, effects: DamageEffects::{}, death_message_type: DeathMessageType::{} }},",
            v["message_id"].as_str().context("damage type without message_id")?,
            variant_name(v["scaling"].as_str().unwrap()),
            v["exhaustion"].as_f64().unwrap() as f32,
            variant_name(v["effects"].as_str().unwrap_or("hurt")),
            variant_name(v["death_message_type"].as_str().unwrap_or("default")),
        )?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn command_tree(out: &mut String, nodes: &Value) -> anyhow::Result<()> {
    writeln!(out, "pub(crate) static COMMAND_NODES: &[commands::CommandNode] = &[")?;
    for (i, node) in nodes.as_array().unwrap().iter().enumerate() {
        let kind = match node["type"].as_str().unwrap() {
            "root" => "Root",
            "literal" => "Literal",
            _ => "Argument",
        };
        let permission = match node["permissions"]["permission"]["level"].as_str() {
            None => 0,
            Some("moderators") => 1,
            Some("gamemasters") => 2,
            Some("admins") => 3,
            Some("owners") => 4,
            Some(other) => bail!("unknown permission level {other}"),
        };
        let children: Vec<u64> = node["children"].as_array().unwrap().iter().map(|c| c.as_u64().unwrap()).collect();
        writeln!(
            out,
            "commands::CommandNode {{ id: {i}, kind: commands::CommandNodeKind::{kind}, name: {:?}, executable: {}, children: &{children:?}, redirect: {:?}, fork: {}, modifier: {}, command: {:?}, type_class: {}, parser: {:?}, properties: {:?}, permission: {permission}, suggestions: {:?} }},",
            node["name"].as_str().unwrap(),
            node["executable"] == true,
            node["redirect"].as_u64(),
            node["fork"] == true,
            node["modifier"] == true,
            node["command"].as_u64(),
            node["type_class"].as_u64().unwrap_or(0),
            node["parser"].as_str(),
            node.get("properties").map_or("{}".to_string(), |p| p.to_string()),
            node["suggestions"].as_str(),
        )?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn waxables(out: &mut String, waxables: &Value, builtin: &HashMap<String, HashMap<String, i32>>) -> anyhow::Result<()> {
    let block_ids = &builtin["minecraft:block"];
    writeln!(out, "pub static WAXABLES: &[(Block, Block)] = &[")?;
    for pair in waxables.as_array().unwrap() {
        writeln!(out, "(Block({}), Block({})),", block_ids[pair[0].as_str().unwrap()], block_ids[pair[1].as_str().unwrap()])?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn fire_odds(out: &mut String, fire: &Value, builtin: &HashMap<String, HashMap<String, i32>>) -> anyhow::Result<()> {
    let block_ids = &builtin["minecraft:block"];
    let mut odds = vec![(0u64, 0u64); block_ids.len()];
    for entry in fire.as_array().unwrap() {
        let id = block_ids[entry["name"].as_str().unwrap()] as usize;
        odds[id] = (entry["ignite"].as_u64().unwrap(), entry["burn"].as_u64().unwrap());
    }
    writeln!(out, "pub static FIRE_ODDS: &[(u8, u8)] = &[")?;
    for (ignite, burn) in odds {
        write!(out, "({ignite},{burn}),")?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn game_rules(out: &mut String, rules: &Value) -> anyhow::Result<()> {
    let mut consts = String::new();
    writeln!(out, "pub(crate) static GAME_RULES: &[game_rules::GameRuleInfo] = &[")?;
    for (i, rule) in rules.as_array().unwrap().iter().enumerate() {
        let name = rule["name"].as_str().unwrap();
        writeln!(
            out,
            "game_rules::GameRuleInfo {{ name: {name:?}, category: {:?}, kind: game_rules::GameRuleType::{}, default: {}, min: {}, max: {}, experimental: {} }},",
            rule["category"].as_str().unwrap(),
            if rule["type"] == "bool" { "Bool" } else { "Int" },
            rule["default"],
            rule["min"],
            rule["max"],
            rule["features"],
        )?;
        writeln!(consts, "pub const {}: GameRule = GameRule({i});", const_name(name))?;
    }
    writeln!(out, "];")?;
    writeln!(out, "pub mod game_rule {{ use crate::GameRule;
{consts}}}")?;
    Ok(())
}

fn embed_json(out: &mut String, data: &Path, kind: &str, name: &str) -> anyhow::Result<()> {
    writeln!(out, "pub static {name}: &[(&str, &str)] = &[")?;
    for ns in namespaces(data)? {
        let dir = data.join(&ns).join(kind);
        for table in list_entries(&ns, &dir)? {
            let path = dir.join(format!("{}.json", table.split_once(':').unwrap().1));
            writeln!(out, "({table:?}, include_str!({:?})),", path.canonicalize()?)?;
        }
    }
    writeln!(out, "];")?;
    Ok(())
}

fn list_entries(namespace: &str, dir: &Path) -> anyhow::Result<Vec<String>> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(base, &path, out)?;
            } else if path.extension().is_some_and(|e| e == "json") {
                let rel = path.strip_prefix(base)?.with_extension("");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(dir, dir, &mut out)?;
    }
    let mut ids: Vec<String> = out.into_iter().map(|p| format!("{namespace}:{p}")).collect();
    ids.sort();
    Ok(ids)
}

fn namespaces(data: &Path) -> anyhow::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(data)? {
        let entry = entry?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

fn synced_registries(out: &mut String, data: &Path) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let mut all = HashMap::new();
    writeln!(out, "pub static SYNCED_REGISTRIES: &[SyncedRegistry] = &[")?;
    for registry in SYNCED_REGISTRIES {
        let mut entries = Vec::new();
        for ns in namespaces(data)? {
            entries.extend(list_entries(&ns, &data.join(&ns).join(registry))?);
        }
        writeln!(out, "SyncedRegistry {{ name: \"minecraft:{registry}\", entries: &{entries:?} }},")?;
        all.insert(format!("minecraft:{registry}"), entries);
    }
    writeln!(out, "];")?;
    Ok(all)
}

fn dimension_types(out: &mut String, data: &Path, entries: &[String]) -> anyhow::Result<()> {
    writeln!(out, "pub static DIMENSION_TYPES: &[DimensionType] = &[")?;
    for id in entries {
        let (ns, path) = id.split_once(':').unwrap();
        let v = read_json(&data.join(ns).join("dimension_type").join(format!("{path}.json")))?;
        let attributes = v.get("attributes").map_or("{}".to_owned(), Value::to_string);
        writeln!(
            out,
            "DimensionType {{ name: {id:?}, min_y: {}, height: {}, has_skylight: {}, has_ceiling: {}, has_fixed_time: {}, infiniburn: &block_tags::{}, default_clock: {:?}, attributes: {attributes:?}, timelines: {:?} }},",
            v["min_y"],
            v["height"],
            v["has_skylight"],
            v["has_ceiling"],
            v["has_fixed_time"].as_bool().unwrap_or(false),
            const_name(v["infiniburn"].as_str().unwrap().trim_start_matches('#')),
            v["default_clock"].as_str(),
            v["timelines"].to_string()
        )?;
    }
    writeln!(out, "];")?;
    Ok(())
}

fn tags(
    out: &mut String,
    data: &Path,
    builtin: &HashMap<String, HashMap<String, i32>>,
    synced: &HashMap<String, Vec<String>>,
) -> anyhow::Result<()> {
    let mut ids_by_registry: BTreeMap<String, HashMap<String, i32>> = BTreeMap::new();
    for (name, entries) in builtin {
        ids_by_registry.insert(name.clone(), entries.clone());
    }
    for (name, entries) in synced {
        let ids = entries.iter().enumerate().map(|(i, e)| (e.clone(), i as i32)).collect();
        ids_by_registry.insert(name.clone(), ids);
    }

    const TYPED: &[(&str, &str)] = &[
        ("minecraft:block", "block_tags"),
        ("minecraft:item", "item_tags"),
        ("minecraft:fluid", "fluid_tags"),
        ("minecraft:entity_type", "entity_type_tags"),
        ("minecraft:damage_type", "damage_type_tags"),
    ];
    let mut typed = String::new();
    writeln!(out, "pub static TAGS: &[RegistryTags] = &[")?;
    for (registry, ids) in &ids_by_registry {
        let path = registry.strip_prefix("minecraft:").unwrap();
        let mut files = BTreeMap::new();
        for ns in namespaces(data)? {
            let dir = data.join(&ns).join("tags").join(path);
            for tag in list_entries(&ns, &dir)? {
                let rel = tag.split_once(':').unwrap().1;
                files.insert(tag.clone(), read_json(&dir.join(format!("{rel}.json")))?);
            }
        }
        if files.is_empty() {
            continue;
        }
        let mut resolved = HashMap::new();
        let module = TYPED.iter().find(|(r, _)| r == registry).map(|(_, m)| *m);
        if let Some(module) = module {
            writeln!(typed, "pub mod {module} {{ use crate::Tag;")?;
        }
        writeln!(out, "RegistryTags {{ registry: {registry:?}, tags: &[")?;
        for tag in files.keys() {
            let values = resolve_tag(tag, &files, ids, &mut resolved, &mut HashSet::new())
                .with_context(|| format!("resolving tag #{tag} of {registry}"))?;
            writeln!(out, "({tag:?}, &{values:?}),")?;
            if module.is_some() {
                let mut bits = vec![0u64; ids.len().div_ceil(64)];
                for v in &values {
                    bits[*v as usize / 64] |= 1 << (*v as usize % 64);
                }
                writeln!(typed, "pub static {}: Tag = Tag::new({tag:?}, &{bits:?});", const_name(tag))?;
            }
        }
        writeln!(out, "] }},")?;
        if module.is_some() {
            writeln!(typed, "}}")?;
        }
    }
    writeln!(out, "];")?;
    out.push_str(&typed);
    Ok(())
}

fn resolve_tag(
    tag: &str,
    files: &BTreeMap<String, Value>,
    ids: &HashMap<String, i32>,
    resolved: &mut HashMap<String, Vec<i32>>,
    visiting: &mut HashSet<String>,
) -> anyhow::Result<Vec<i32>> {
    if let Some(values) = resolved.get(tag) {
        return Ok(values.clone());
    }
    if !visiting.insert(tag.to_owned()) {
        bail!("tag cycle through #{tag}");
    }
    let file = files.get(tag).with_context(|| format!("unknown tag #{tag}"))?;
    let mut values = Vec::new();
    for value in file["values"].as_array().unwrap() {
        let (id, required) = match value {
            Value::String(s) => (s.as_str(), true),
            Value::Object(o) => (o["id"].as_str().unwrap(), o.get("required").and_then(Value::as_bool).unwrap_or(true)),
            _ => bail!("malformed tag value {value}"),
        };
        let found = match id.strip_prefix('#') {
            Some(inner) if files.contains_key(inner) => resolve_tag(inner, files, ids, resolved, visiting)?,
            Some(_) => Vec::new(),
            None => ids.get(id).map(|&i| vec![i]).unwrap_or_default(),
        };
        if found.is_empty() && required && !id.starts_with('#') {
            bail!("required entry {id} does not exist");
        }
        for v in found {
            if !values.contains(&v) {
                values.push(v);
            }
        }
    }
    visiting.remove(tag);
    resolved.insert(tag.to_owned(), values.clone());
    Ok(values)
}
