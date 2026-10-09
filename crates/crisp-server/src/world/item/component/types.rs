use crisp_protocol::nbt::Nbt;

use super::codec::{Codec, Dispatch, Field, Kind, RangeError, Selector, Stream};
use super::{DataComponentType, TypeInfo, custom, text};

pub type Pair = (Codec, Stream);

const MAX: usize = i32::MAX as usize;

pub fn req(name: &'static str, p: Pair) -> Field {
    Field::new(name, p.0, p.1, Kind::Required)
}

pub fn opt(name: &'static str, p: Pair) -> Field {
    Field::new(name, p.0, p.1, Kind::Optional)
}

pub fn def(name: &'static str, p: Pair, default: Nbt) -> Field {
    Field::new(name, p.0, p.1, Kind::Default(default))
}

pub fn always(name: &'static str, p: Pair, default: Nbt) -> Field {
    Field::new(name, p.0, p.1, Kind::AlwaysPresent(default))
}

pub fn lenient(name: &'static str, p: Pair, default: Nbt) -> Field {
    Field::new(name, p.0, p.1, Kind::Lenient(default))
}

pub fn lenient_opt(name: &'static str, p: Pair) -> Field {
    Field::new(name, p.0, p.1, Kind::LenientOptional)
}

pub fn record(fields: Vec<Field>) -> Pair {
    (Codec::Record(fields), Stream::Record)
}

pub fn list(p: Pair) -> Pair {
    (Codec::list(p.0), Stream::List(Box::new(p.1), MAX))
}

pub fn list_max(p: Pair, max: usize) -> Pair {
    (Codec::list_max(p.0, max), Stream::List(Box::new(p.1), max))
}

pub fn bool() -> Pair {
    (Codec::Bool, Stream::Bool)
}

pub fn unit() -> Pair {
    (Codec::Unit, Stream::Unit)
}

pub fn var_int(codec: Codec) -> Pair {
    (codec, Stream::VarInt)
}

pub fn non_negative_int() -> Codec {
    Codec::Int(0, i32::MAX, RangeError::NonNegative)
}

pub fn positive_int() -> Codec {
    Codec::Int(1, i32::MAX, RangeError::Positive)
}

pub fn float(codec: Codec) -> Pair {
    (codec, Stream::Float)
}

pub fn non_negative_float() -> Codec {
    Codec::Float(0.0, f32::MAX, false, RangeError::NonNegative)
}

pub fn positive_float() -> Codec {
    Codec::Float(0.0, f32::MAX, true, RangeError::Positive)
}

pub fn float_range(min: f32, max: f32) -> Codec {
    Codec::Float(min, max, false, RangeError::Within)
}

pub fn codec_float_range(min: f32, max: f32) -> Codec {
    Codec::Float(min, max, false, RangeError::Outside)
}

pub fn string() -> Pair {
    (Codec::string(), Stream::Utf8(32767))
}

pub fn identifier() -> Pair {
    (Codec::Identifier, Stream::Identifier)
}

pub fn by_name(registry: &'static str) -> Pair {
    (Codec::Element(registry), Stream::Registry(registry))
}

pub fn fixed(registry: &'static str) -> Pair {
    (Codec::Holder(registry, None), Stream::Registry(registry))
}

pub fn holder(registry: &'static str, direct: Pair) -> Pair {
    (Codec::Holder(registry, Some(Box::new(direct.0))), Stream::Holder(registry, Some(Box::new(direct.1))))
}

pub fn holder_set(registry: &'static str) -> Pair {
    (Codec::HolderSet(registry), Stream::HolderSet(registry))
}

pub fn enumeration(names: &'static [&'static str]) -> Pair {
    (Codec::Enum(names), Stream::Enum)
}

pub fn either(a: Pair, b: Pair) -> Pair {
    (Codec::Either(Box::new(a.0), Box::new(b.0)), Stream::Either(Box::new(a.1), Box::new(b.1)))
}

pub fn alt(primary: Pair, alternative: Codec, convert: fn(&Nbt) -> Result<Nbt, String>) -> Pair {
    (Codec::Alt(Box::new(primary.0), Box::new(alternative), convert), primary.1)
}

pub fn nbt(codec: Codec) -> Pair {
    (codec, Stream::Nbt)
}


pub fn f(v: f32) -> Nbt {
    Nbt::Float(v)
}

pub fn i(v: i32) -> Nbt {
    Nbt::Int(v)
}

pub fn b(v: bool) -> Nbt {
    Nbt::Byte(v as i8)
}

pub fn s(v: &str) -> Nbt {
    Nbt::String(v.into())
}

pub fn empty_list() -> Nbt {
    Nbt::List(Vec::new())
}

pub fn empty_map() -> Nbt {
    Nbt::Compound(Vec::new())
}

pub fn compound(fields: &[(&str, Nbt)]) -> Nbt {
    Nbt::Compound(fields.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

pub const DYE_COLORS: &[&str] =
    &["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

pub const EQUIPMENT_SLOTS: &[&str] = &["mainhand", "feet", "legs", "chest", "head", "offhand", "body", "saddle"];

pub const EQUIPMENT_SLOT_GROUPS: &[&str] = &["any", "mainhand", "offhand", "hand", "feet", "legs", "chest", "head", "armor", "body", "saddle"];

pub const ITEM_USE_ANIMATIONS: &[&str] = &["none", "eat", "drink", "block", "bow", "trident", "crossbow", "spyglass", "toot_horn", "brush", "bundle", "spear"];

pub const RARITIES: &[&str] = &["common", "uncommon", "rare", "epic"];

pub fn dye_color() -> Pair {
    enumeration(DYE_COLORS)
}

pub fn sound_event() -> Pair {
    holder("minecraft:sound_event", record(vec![req("sound_id", identifier()), lenient_opt("range", float(Codec::float()))]))
}

fn color_from_floats(v: &Nbt, alpha: bool) -> Result<Nbt, String> {
    let channel = |c: f32| -> u32 { (c * 255.0).floor().clamp(0.0, 255.0) as u32 };
    let items = v.as_list();
    let get = |i: usize| super::codec::as_f32(&items[i]);
    let a = if alpha { channel(get(3)) } else { 255 };
    Ok(Nbt::Int(((a << 24) | (channel(get(0)) << 16) | (channel(get(1)) << 8) | channel(get(2))) as i32))
}

fn vector(size: usize) -> Codec {
    Codec::Normalize(Box::new(Codec::list(Codec::float())), if size == 3 { fixed_size_3 } else { fixed_size_4 })
}

fn fixed_size_3(v: Nbt) -> Result<Nbt, String> {
    fixed_size(v, 3)
}

fn fixed_size_4(v: Nbt) -> Result<Nbt, String> {
    fixed_size(v, 4)
}

fn fixed_size(v: Nbt, size: usize) -> Result<Nbt, String> {
    let n = v.as_list().len();
    if n == size { Ok(v) } else { Err(format!("Input is not a list of {size} elements")) }
}

pub fn rgb_color() -> Codec {
    Codec::Alt(Box::new(Codec::int()), Box::new(vector(3)), |v| color_from_floats(v, false))
}

pub fn argb_color() -> Codec {
    Codec::Alt(Box::new(Codec::int()), Box::new(vector(4)), |v| color_from_floats(v, true))
}

pub fn filterable(value: fn() -> Pair) -> Pair {
    alt(record(vec![req("raw", value()), opt("filtered", value())]), value().0, |v| Ok(compound(&[("raw", v.clone())])))
}

pub fn mob_effect_instance() -> Pair {
    let mut fields = vec![req("id", by_name("minecraft:mob_effect"))];
    fields.extend(effect_details_fields());
    (Codec::Normalize(Box::new(Codec::Record(fields)), custom::show_icon_default), Stream::Record)
}

fn effect_details_fields() -> Vec<Field> {
    vec![
        def("amplifier", var_int(Codec::UnsignedByte), Nbt::Byte(0)),
        def("duration", var_int(Codec::int()), i(0)),
        def("ambient", bool(), b(false)),
        def("show_particles", bool(), b(true)),
        Field::new("show_icon", Codec::Bool, Stream::Bool, Kind::Plain),
        opt("hidden_effect", (Codec::Lazy(effect_details), Stream::Lazy)),
    ]
}

fn effect_details() -> &'static Pair {
    static DETAILS: std::sync::LazyLock<Pair> =
        std::sync::LazyLock::new(|| (Codec::Normalize(Box::new(Codec::Record(effect_details_fields())), custom::show_icon_default), Stream::Record));
    &DETAILS
}

pub fn consume_effect() -> Pair {
    let cases = vec![
        ("minecraft:apply_effects", vec![req("effects", list(mob_effect_instance())), def("probability", float(codec_float_range(0.0, 1.0)), f(1.0))]),
        ("minecraft:remove_effects", vec![req("effects", holder_set("minecraft:mob_effect"))]),
        ("minecraft:clear_all_effects", vec![]),
        ("minecraft:teleport_randomly", vec![def("diameter", float(positive_float()), f(16.0)), def("directional_particles", bool(), b(true))]),
        ("minecraft:play_sound", vec![req("sound", sound_event())]),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:consume_effect_type"), cases })), Stream::Dispatch)
}

fn resolvable_int() -> Pair {
    either((Codec::int(), Stream::Int), identifier())
}

fn resolvable_float() -> Pair {
    either(float(Codec::float()), identifier())
}

fn enchantments() -> Pair {
    let level = Codec::Int(1, 255, RangeError::Outside);
    (
        Codec::Map(Box::new(Codec::Holder("minecraft:enchantment", None)), Box::new(level)),
        Stream::Map(Box::new(Stream::Registry("minecraft:enchantment")), Box::new(Stream::VarInt), MAX),
    )
}

fn swing_animation() -> Pair {
    record(vec![def("type", enumeration(&["none", "whack", "stab"]), s("whack")), def("duration", var_int(non_negative_int()), i(6))])
}

fn attribute_modifier_entry() -> Pair {
    let display = Dispatch {
        key: "type",
        selector: Selector::Enum,
        cases: vec![("default", vec![]), ("hidden", vec![]), ("override", vec![req("value", text::text())])],
    };
    record(vec![
        req("type", by_name("minecraft:attribute")),
        req("id", identifier()),
        req("amount", (Codec::Double, Stream::Double)),
        req("operation", enumeration(&["add_value", "add_multiplied_base", "add_multiplied_total"])),
        def("slot", enumeration(EQUIPMENT_SLOT_GROUPS), s("any")),
        def("display", (Codec::Dispatch(Box::new(display)), Stream::Dispatch), compound(&[("type", s("default"))])),
    ])
}

fn kinetic_condition() -> Pair {
    record(vec![
        req("max_duration_ticks", var_int(non_negative_int())),
        def("min_speed", float(Codec::float()), f(0.0)),
        def("min_relative_speed", float(Codec::float()), f(0.0)),
    ])
}

fn firework_explosion() -> Pair {
    record(vec![
        req("shape", enumeration(&["small_ball", "large_ball", "star", "creeper", "burst"])),
        def("colors", list((Codec::int(), Stream::Int)), empty_list()),
        def("fade_colors", list((Codec::int(), Stream::Int)), empty_list()),
        def("has_trail", bool(), b(false)),
        def("has_twinkle", bool(), b(false)),
    ])
}

fn trim_material() -> Pair {
    holder("minecraft:trim_material", record(vec![req("palette_id", identifier()), req("description", text::text())]))
}

fn written_title() -> Pair {
    (Codec::String(0, 32), Stream::Utf8(32))
}

fn writable_page() -> Pair {
    (Codec::String(0, 1024), Stream::Utf8(1024))
}

fn written_page() -> Pair {
    text::text()
}

fn global_pos() -> Pair {
    record(vec![req("dimension", identifier()), req("pos", custom::block_pos())])
}

fn damage_reduction() -> Pair {
    record(vec![
        def("horizontal_blocking_angle", float(positive_float()), f(90.0)),
        opt("type", holder_set("minecraft:damage_type")),
        req("base", float(Codec::float())),
        req("factor", float(Codec::float())),
    ])
}

fn item_damage_function() -> Pair {
    record(vec![req("threshold", float(non_negative_float())), req("base", float(Codec::float())), req("factor", float(Codec::float()))])
}

fn adventure_mode_predicate() -> Pair {
    let (codec, stream) = custom::block_predicate();
    (
        Codec::Normalize(Box::new(Codec::Compact(Box::new(codec))), custom::non_empty_compact),
        Stream::List(Box::new(stream), MAX),
    )
}

fn info(persistent: bool, p: Pair) -> TypeInfo {
    TypeInfo { persistent, codec: p.0, stream: p.1 }
}

pub fn build() -> Vec<TypeInfo> {
    let p = |pair: Pair| info(true, pair);
    let t = |pair: Pair| info(false, pair);
    let dye = || p(dye_color());
    vec![
        p(nbt(custom::custom_data_codec())),
        p(var_int(Codec::Int(1, 99, RangeError::Within))),
        p(var_int(positive_int())),
        p(var_int(non_negative_int())),
        p(unit()),
        p(record(vec![
            def("can_sprint", bool(), b(false)),
            def("interact_vibrations", bool(), b(true)),
            def("speed_multiplier", float(codec_float_range(0.0, 1.0)), f(0.2)),
        ])),
        p(text::text()),
        p(float(float_range(0.0, 1.0))),
        p(fixed("minecraft:damage_type")),
        p(text::text()),
        p(identifier()),
        p(list_max(text::text(), 256)),
        p(enumeration(RARITIES)),
        p(enchantments()),
        p(adventure_mode_predicate()),
        p(adventure_mode_predicate()),
        p(list(attribute_modifier_entry())),
        p(record(vec![
            def("floats", list(float(Codec::float())), empty_list()),
            def("flags", list(bool()), empty_list()),
            def("strings", list(string()), empty_list()),
            def("colors", list((rgb_color(), Stream::Int)), empty_list()),
        ])),
        p(record(vec![
            def("hide_tooltip", bool(), b(false)),
            def("hidden_components", list(by_name("minecraft:data_component_type")), empty_list()),
        ])),
        p(var_int(non_negative_int())),
        t(unit()),
        p(bool()),
        p(nbt(Codec::Unit)),
        p(record(vec![
            req("nutrition", var_int(non_negative_int())),
            req("saturation", float(Codec::float())),
            def("can_always_eat", bool(), b(false)),
        ])),
        p(record(vec![
            def("consume_seconds", float(non_negative_float()), f(1.6)),
            def("animation", enumeration(ITEM_USE_ANIMATIONS), s("eat")),
            def("sound", sound_event(), s("minecraft:entity.generic.eat")),
            def("has_consume_particles", bool(), b(true)),
            def("on_consume_effects", list(consume_effect()), empty_list()),
        ])),
        p(custom::template()),
        p(record(vec![req("seconds", float(positive_float())), opt("cooldown_group", identifier())])),
        p(record(vec![req("types", holder_set("minecraft:damage_type"))])),
        p(record(vec![
            req(
                "rules",
                list(record(vec![
                    req("blocks", holder_set("minecraft:block")),
                    opt("speed", float(positive_float())),
                    opt("correct_for_drops", bool()),
                ])),
            ),
            def("default_mining_speed", float(Codec::float()), f(1.0)),
            def("damage_per_block", var_int(non_negative_int()), i(1)),
            def("can_destroy_blocks_in_creative", bool(), b(true)),
        ])),
        p(record(vec![
            def("item_damage_per_attack", var_int(non_negative_int()), i(1)),
            def("disable_blocking_for_seconds", float(non_negative_float()), f(0.0)),
        ])),
        p(record(vec![
            def("min_reach", float(float_range(0.0, 64.0)), f(0.0)),
            def("max_reach", float(float_range(0.0, 64.0)), f(3.0)),
            def("min_creative_reach", float(float_range(0.0, 64.0)), f(0.0)),
            def("max_creative_reach", float(float_range(0.0, 64.0)), f(5.0)),
            def("hitbox_margin", float(float_range(0.0, 1.0)), f(0.3)),
            def("mob_factor", float(codec_float_range(0.0, 2.0)), f(1.0)),
        ])),
        p(record(vec![req("value", var_int(positive_int()))])),
        p(record(vec![
            req("slot", enumeration(EQUIPMENT_SLOTS)),
            def("equip_sound", sound_event(), s("minecraft:item.armor.equip_generic")),
            opt("asset_id", identifier()),
            opt("camera_overlay", identifier()),
            opt("allowed_entities", holder_set("minecraft:entity_type")),
            def("dispensable", bool(), b(true)),
            def("swappable", bool(), b(true)),
            def("damage_on_hurt", bool(), b(true)),
            def("equip_on_interact", bool(), b(false)),
            def("can_be_sheared", bool(), b(false)),
            def("shearing_sound", sound_event(), s("minecraft:item.shears.snip")),
        ])),
        p(record(vec![req("items", holder_set("minecraft:item"))])),
        p(unit()),
        p(identifier()),
        p(record(vec![def("death_effects", list(consume_effect()), empty_list())])),
        p(record(vec![
            def("block_delay_seconds", float(non_negative_float()), f(0.0)),
            def("disable_cooldown_scale", float(non_negative_float()), f(1.0)),
            def("damage_reductions", list(damage_reduction()), Nbt::List(vec![compound(&[("base", f(0.0)), ("factor", f(1.0))])])),
            def("item_damage", item_damage_function(), compound(&[("threshold", f(1.0)), ("base", f(0.0)), ("factor", f(1.0))])),
            opt("bypassed_by", holder_set("minecraft:damage_type")),
            opt("block_sound", sound_event()),
            opt("disabled_sound", sound_event()),
        ])),
        p(record(vec![
            def("deals_knockback", bool(), b(true)),
            def("dismounts", bool(), b(false)),
            opt("sound", sound_event()),
            opt("hit_sound", sound_event()),
        ])),
        p(record(vec![
            def("contact_cooldown_ticks", var_int(non_negative_int()), i(10)),
            def("delay_ticks", var_int(non_negative_int()), i(0)),
            opt("dismount_conditions", kinetic_condition()),
            opt("knockback_conditions", kinetic_condition()),
            opt("damage_conditions", kinetic_condition()),
            def("forward_movement", float(Codec::float()), f(0.0)),
            def("damage_multiplier", float(Codec::float()), f(1.0)),
            opt("sound", sound_event()),
            opt("hit_sound", sound_event()),
        ])),
        p(swing_animation()),
        p(swing_animation()),
        t(var_int(Codec::int())),
        p(fixed("minecraft:block_transformer")),
        p(record(vec![req("nutrition", var_int(positive_int()))])),
        p(enchantments()),
        dye(),
        p((rgb_color(), Stream::Int)),
        p(var_int(Codec::int())),
        p(nbt(Codec::Map(
            Box::new(Codec::string()),
            Box::new(Codec::Record(vec![
                req("type", by_name("minecraft:map_decoration_type")),
                req("x", (Codec::Double, Stream::Double)),
                req("z", (Codec::Double, Stream::Double)),
                req("rotation", float(Codec::float())),
            ])),
        ))),
        t(enumeration(&["lock", "scale"])),
        p(list_max(custom::template(), 1024)),
        p(list(custom::template())),
        p(alt(
            record(vec![
                opt("potion", by_name("minecraft:potion")),
                opt("custom_color", (Codec::int(), Stream::Int)),
                def("custom_effects", list(mob_effect_instance()), empty_list()),
                opt("custom_name", string()),
            ]),
            Codec::Element("minecraft:potion"),
            |potion| Ok(compound(&[("potion", potion.clone())])),
        )),
        p(float(non_negative_float())),
        p(list(record(vec![req("id", by_name("minecraft:mob_effect")), lenient("duration", var_int(Codec::int()), i(160))]))),
        p(record(vec![def("pages", list_max(filterable(writable_page), 100), empty_list())])),
        p(record(vec![
            req("title", filterable(written_title)),
            req("author", string()),
            def("generation", var_int(Codec::Int(0, 3, RangeError::Within)), i(0)),
            def("pages", list(filterable(written_page)), empty_list()),
            def("resolved", bool(), b(false)),
        ])),
        p(record(vec![
            req("material", trim_material()),
            req(
                "pattern",
                holder(
                    "minecraft:trim_pattern",
                    record(vec![req("asset_id", identifier()), req("description", text::text()), always("decal", bool(), b(false))]),
                ),
            ),
        ])),
        p(nbt(Codec::Map(Box::new(Codec::Element("minecraft:block")), Box::new(Codec::string())))),
        p(custom::typed_entity_data("minecraft:entity_type")),
        p(nbt(custom::custom_data_codec())),
        p(custom::typed_entity_data("minecraft:block_entity_type")),
        p(holder(
            "minecraft:instrument",
            record(vec![
                req("sound_event", sound_event()),
                req("use_duration", float(non_negative_float())),
                req("range", float(positive_float())),
                def("durability_damage", var_int(non_negative_int()), i(0)),
                req("description", text::text()),
            ]),
        )),
        p(trim_material()),
        p(var_int(Codec::Int(0, 4, RangeError::Within))),
        p((Codec::Holder("minecraft:jukebox_song", None), Stream::Holder("minecraft:jukebox_song", None))),
        p(holder_set("minecraft:banner_pattern")),
        p(nbt(Codec::list(Codec::Identifier))),
        p(record(vec![opt("target", global_pos()), def("tracked", bool(), b(true))])),
        p(firework_explosion()),
        p(record(vec![
            def("flight_duration", var_int(Codec::UnsignedByte), Nbt::Byte(0)),
            def("explosions", list_max(firework_explosion(), 256), empty_list()),
        ])),
        p(custom::profile()),
        p(identifier()),
        p(list(record(vec![
            req(
                "pattern",
                holder("minecraft:banner_pattern", record(vec![req("asset_id", identifier()), req("translation_key", string())])),
            ),
            req("color", dye_color()),
        ]))),
        dye(),
        p(record(vec![
            opt("back", custom::template()),
            opt("left", custom::template()),
            opt("right", custom::template()),
            opt("front", custom::template()),
        ])),
        p(custom::container()),
        p((Codec::Map(Box::new(Codec::string()), Box::new(Codec::string())), Stream::Map(Box::new(Stream::Utf8(32767)), Box::new(Stream::Utf8(32767)), MAX))),
        p(list(record(vec![
            req("entity_data", custom::typed_entity_data("minecraft:entity_type")),
            req("ticks_in_hive", var_int(Codec::int())),
            req("min_ticks_in_hive", var_int(Codec::int())),
        ]))),
        p(custom::template()),
        p(nbt(custom::item_predicate())),
        p(nbt(Codec::Record(vec![
            req("loot_table", identifier()),
            Field::new("seed", Codec::long(), Stream::Long, Kind::Default(Nbt::Long(0))),
        ]))),
        p(sound_event()),
        p(record(vec![req("layers", resolvable_int())])),
        p(record(vec![req("burn_time", resolvable_int()), req("speed_multiplier", resolvable_float())])),
        p(record(vec![req("uses", resolvable_int()), req("speed_multiplier", resolvable_float())])),
        p(record(vec![req("targeting_entity_types", holder_set("minecraft:entity_type")), req("visibility", float(float_range(0.0, 10.0)))])),
        p(fixed("minecraft:villager_type")),
        p(fixed("minecraft:wolf_variant")),
        p(fixed("minecraft:wolf_sound_variant")),
        dye(),
        p(enumeration(&["red", "snow"])),
        p(enumeration(&["small", "medium", "large"])),
        p(enumeration(&["red_blue", "blue", "green", "yellow_blue", "gray"])),
        p((
            Codec::IdEnum(&[
                ("kob", 0),
                ("sunstreak", 256),
                ("snooper", 512),
                ("dasher", 768),
                ("brinely", 1024),
                ("spotty", 1280),
                ("flopper", 1),
                ("stripey", 257),
                ("glitter", 513),
                ("blockfish", 769),
                ("betty", 1025),
                ("clayfish", 1281),
            ]),
            Stream::Enum,
        )),
        dye(),
        dye(),
        p(enumeration(&["red", "brown"])),
        p((
            Codec::IdEnum(&[("brown", 0), ("white", 1), ("black", 2), ("white_splotched", 3), ("gold", 4), ("salt", 5), ("evil", 99)]),
            Stream::Enum,
        )),
        p(fixed("minecraft:pig_variant")),
        p(fixed("minecraft:pig_sound_variant")),
        p(fixed("minecraft:cow_variant")),
        p(fixed("minecraft:cow_sound_variant")),
        p(fixed("minecraft:chicken_variant")),
        p(fixed("minecraft:chicken_sound_variant")),
        p(fixed("minecraft:zombie_nautilus_variant")),
        p(fixed("minecraft:frog_variant")),
        p(enumeration(&["white", "creamy", "chestnut", "brown", "black", "gray", "dark_brown"])),
        p((Codec::Holder("minecraft:painting_variant", None), Stream::Holder("minecraft:painting_variant", None))),
        p(enumeration(&["creamy", "white", "brown", "gray"])),
        p(enumeration(&["lucy", "wild", "gold", "cyan", "blue"])),
        p(fixed("minecraft:cat_variant")),
        p(fixed("minecraft:cat_sound_variant")),
        dye(),
        dye(),
        dye(),
        p(fixed("minecraft:decorated_pot_pattern")),
        p(custom::sign_text()),
        p(custom::sign_text()),
        p(unit()),
        dye(),
    ]
}

macro_rules! component_types {
    ($($name:ident = $id:literal $key:literal;)*) => {
        $(#[allow(dead_code)] pub const $name: DataComponentType = DataComponentType($id);)*
        #[cfg(test)]
        pub(super) const NAMED: &[(DataComponentType, &str)] = &[$(($name, $key)),*];
    };
}

component_types! {
    CUSTOM_DATA = 0 "minecraft:custom_data";
    MAX_STACK_SIZE = 1 "minecraft:max_stack_size";
    MAX_DAMAGE = 2 "minecraft:max_damage";
    DAMAGE = 3 "minecraft:damage";
    UNBREAKABLE = 4 "minecraft:unbreakable";
    USE_EFFECTS = 5 "minecraft:use_effects";
    CUSTOM_NAME = 6 "minecraft:custom_name";
    MINIMUM_ATTACK_CHARGE = 7 "minecraft:minimum_attack_charge";
    DAMAGE_TYPE = 8 "minecraft:damage_type";
    ITEM_NAME = 9 "minecraft:item_name";
    ITEM_MODEL = 10 "minecraft:item_model";
    LORE = 11 "minecraft:lore";
    RARITY = 12 "minecraft:rarity";
    ENCHANTMENTS = 13 "minecraft:enchantments";
    CAN_PLACE_ON = 14 "minecraft:can_place_on";
    CAN_BREAK = 15 "minecraft:can_break";
    ATTRIBUTE_MODIFIERS = 16 "minecraft:attribute_modifiers";
    CUSTOM_MODEL_DATA = 17 "minecraft:custom_model_data";
    TOOLTIP_DISPLAY = 18 "minecraft:tooltip_display";
    REPAIR_COST = 19 "minecraft:repair_cost";
    CREATIVE_SLOT_LOCK = 20 "minecraft:creative_slot_lock";
    ENCHANTMENT_GLINT_OVERRIDE = 21 "minecraft:enchantment_glint_override";
    INTANGIBLE_PROJECTILE = 22 "minecraft:intangible_projectile";
    FOOD = 23 "minecraft:food";
    CONSUMABLE = 24 "minecraft:consumable";
    USE_REMAINDER = 25 "minecraft:use_remainder";
    USE_COOLDOWN = 26 "minecraft:use_cooldown";
    DAMAGE_RESISTANT = 27 "minecraft:damage_resistant";
    TOOL = 28 "minecraft:tool";
    WEAPON = 29 "minecraft:weapon";
    ATTACK_RANGE = 30 "minecraft:attack_range";
    ENCHANTABLE = 31 "minecraft:enchantable";
    EQUIPPABLE = 32 "minecraft:equippable";
    REPAIRABLE = 33 "minecraft:repairable";
    GLIDER = 34 "minecraft:glider";
    TOOLTIP_STYLE = 35 "minecraft:tooltip_style";
    DEATH_PROTECTION = 36 "minecraft:death_protection";
    BLOCKS_ATTACKS = 37 "minecraft:blocks_attacks";
    PIERCING_WEAPON = 38 "minecraft:piercing_weapon";
    KINETIC_WEAPON = 39 "minecraft:kinetic_weapon";
    ATTACK_ANIMATION = 40 "minecraft:attack_animation";
    INTERACT_ANIMATION = 41 "minecraft:interact_animation";
    ADDITIONAL_TRADE_COST = 42 "minecraft:additional_trade_cost";
    BLOCK_TRANSFORMER = 43 "minecraft:block_transformer";
    VILLAGER_FOOD = 44 "minecraft:villager_food";
    STORED_ENCHANTMENTS = 45 "minecraft:stored_enchantments";
    DYE = 46 "minecraft:dye";
    DYED_COLOR = 47 "minecraft:dyed_color";
    MAP_ID = 48 "minecraft:map_id";
    MAP_DECORATIONS = 49 "minecraft:map_decorations";
    MAP_POST_PROCESSING = 50 "minecraft:map_post_processing";
    CHARGED_PROJECTILES = 51 "minecraft:charged_projectiles";
    BUNDLE_CONTENTS = 52 "minecraft:bundle_contents";
    POTION_CONTENTS = 53 "minecraft:potion_contents";
    POTION_DURATION_SCALE = 54 "minecraft:potion_duration_scale";
    SUSPICIOUS_STEW_EFFECTS = 55 "minecraft:suspicious_stew_effects";
    WRITABLE_BOOK_CONTENT = 56 "minecraft:writable_book_content";
    WRITTEN_BOOK_CONTENT = 57 "minecraft:written_book_content";
    TRIM = 58 "minecraft:trim";
    DEBUG_STICK_STATE = 59 "minecraft:debug_stick_state";
    ENTITY_DATA = 60 "minecraft:entity_data";
    BUCKET_ENTITY_DATA = 61 "minecraft:bucket_entity_data";
    BLOCK_ENTITY_DATA = 62 "minecraft:block_entity_data";
    INSTRUMENT = 63 "minecraft:instrument";
    PROVIDES_TRIM_MATERIAL = 64 "minecraft:provides_trim_material";
    OMINOUS_BOTTLE_AMPLIFIER = 65 "minecraft:ominous_bottle_amplifier";
    JUKEBOX_PLAYABLE = 66 "minecraft:jukebox_playable";
    PROVIDES_BANNER_PATTERNS = 67 "minecraft:provides_banner_patterns";
    RECIPES = 68 "minecraft:recipes";
    LODESTONE_TRACKER = 69 "minecraft:lodestone_tracker";
    FIREWORK_EXPLOSION = 70 "minecraft:firework_explosion";
    FIREWORKS = 71 "minecraft:fireworks";
    PROFILE = 72 "minecraft:profile";
    NOTE_BLOCK_SOUND = 73 "minecraft:note_block_sound";
    BANNER_PATTERNS = 74 "minecraft:banner_patterns";
    BASE_COLOR = 75 "minecraft:base_color";
    POT_DECORATIONS = 76 "minecraft:pot_decorations";
    CONTAINER = 77 "minecraft:container";
    BLOCK_STATE = 78 "minecraft:block_state";
    BEES = 79 "minecraft:bees";
    SULFUR_CUBE_CONTENT = 80 "minecraft:sulfur_cube_content";
    LOCK = 81 "minecraft:lock";
    CONTAINER_LOOT = 82 "minecraft:container_loot";
    BREAK_SOUND = 83 "minecraft:break_sound";
    COMPOSTABLE = 84 "minecraft:compostable";
    COOKING_FUEL = 85 "minecraft:cooking_fuel";
    BREWING_FUEL = 86 "minecraft:brewing_fuel";
    MOB_VISIBILITY = 87 "minecraft:mob_visibility";
    VILLAGER_VARIANT = 88 "minecraft:villager/variant";
    WOLF_VARIANT = 89 "minecraft:wolf/variant";
    WOLF_SOUND_VARIANT = 90 "minecraft:wolf/sound_variant";
    WOLF_COLLAR = 91 "minecraft:wolf/collar";
    FOX_VARIANT = 92 "minecraft:fox/variant";
    SALMON_SIZE = 93 "minecraft:salmon/size";
    PARROT_VARIANT = 94 "minecraft:parrot/variant";
    TROPICAL_FISH_PATTERN = 95 "minecraft:tropical_fish/pattern";
    TROPICAL_FISH_BASE_COLOR = 96 "minecraft:tropical_fish/base_color";
    TROPICAL_FISH_PATTERN_COLOR = 97 "minecraft:tropical_fish/pattern_color";
    MOOSHROOM_VARIANT = 98 "minecraft:mooshroom/variant";
    RABBIT_VARIANT = 99 "minecraft:rabbit/variant";
    PIG_VARIANT = 100 "minecraft:pig/variant";
    PIG_SOUND_VARIANT = 101 "minecraft:pig/sound_variant";
    COW_VARIANT = 102 "minecraft:cow/variant";
    COW_SOUND_VARIANT = 103 "minecraft:cow/sound_variant";
    CHICKEN_VARIANT = 104 "minecraft:chicken/variant";
    CHICKEN_SOUND_VARIANT = 105 "minecraft:chicken/sound_variant";
    ZOMBIE_NAUTILUS_VARIANT = 106 "minecraft:zombie_nautilus/variant";
    FROG_VARIANT = 107 "minecraft:frog/variant";
    HORSE_VARIANT = 108 "minecraft:horse/variant";
    PAINTING_VARIANT = 109 "minecraft:painting/variant";
    LLAMA_VARIANT = 110 "minecraft:llama/variant";
    AXOLOTL_VARIANT = 111 "minecraft:axolotl/variant";
    CAT_VARIANT = 112 "minecraft:cat/variant";
    CAT_SOUND_VARIANT = 113 "minecraft:cat/sound_variant";
    CAT_COLLAR = 114 "minecraft:cat/collar";
    SHEEP_COLOR = 115 "minecraft:sheep/color";
    SHULKER_COLOR = 116 "minecraft:shulker/color";
    PROVIDES_POTTERY_PATTERN = 117 "minecraft:provides_pottery_pattern";
    SIGN_TEXT_FRONT = 118 "minecraft:sign_text_front";
    SIGN_TEXT_BACK = 119 "minecraft:sign_text_back";
    WAXED = 120 "minecraft:waxed";
    CUSHION_COLOR = 121 "minecraft:cushion/color";
}
