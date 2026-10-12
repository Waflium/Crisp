use std::collections::HashMap;
use std::sync::LazyLock;

use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use super::particle_options::ParticleOptions;
use crate::util::hash_ops::{self, HashCode};
use crate::world::item::component::codec::{self, Codec, Custom, Dispatch, RangeError, Selector, Stream};
use crate::world::item::component::custom;
use crate::world::item::component::types::{Pair, def, f, float, i, positive_int, record, req, rgb_color, argb_color, var_int};

fn block_particle_option() -> Pair {
    record(vec![req("block_state", (Codec::Custom(Box::new(BlockStateCodec)), Stream::Custom))])
}

fn color_particle_option() -> Pair {
    record(vec![req("color", (argb_color(), Stream::Int))])
}

fn dust_color_transition_options() -> Pair {
    record(vec![req("from_color", (rgb_color(), Stream::Int)), req("to_color", (rgb_color(), Stream::Int)), req("scale", float(scale()))])
}

fn dust_particle_options() -> Pair {
    record(vec![req("color", (rgb_color(), Stream::Int)), req("scale", float(scale()))])
}

fn scale() -> Codec {
    Codec::Float(0.01, 4.0, false, RangeError::Within)
}

fn geyser_base_particle_options() -> Pair {
    record(vec![req("water_blocks", (positive_int(), Stream::Int)), req("burst_impulse_base", float(Codec::float()))])
}

fn geyser_particle_options() -> Pair {
    record(vec![req("water_blocks", (positive_int(), Stream::Int))])
}

fn item_particle_option() -> Pair {
    record(vec![req("item", custom::template())])
}

fn power_particle_option() -> Pair {
    record(vec![def("power", float(Codec::float()), f(1.0))])
}

fn sculk_charge_particle_options() -> Pair {
    record(vec![req("roll", float(Codec::float()))])
}

fn shriek_particle_option() -> Pair {
    record(vec![req("delay", var_int(Codec::int()))])
}

fn spell_particle_option() -> Pair {
    record(vec![def("color", (rgb_color(), Stream::Int), i(-1)), def("power", float(Codec::float()), f(1.0))])
}

fn trail_particle_option() -> Pair {
    record(vec![
        req("target", (Codec::Custom(Box::new(Vec3Codec)), Stream::Custom)),
        req("color", (rgb_color(), Stream::Int)),
        req("duration", var_int(positive_int())),
    ])
}

fn vibration_particle_option() -> Pair {
    record(vec![req("destination", safe_position_source()), req("arrival_in_ticks", var_int(Codec::int()))])
}

fn safe_position_source() -> Pair {
    let cases = vec![
        ("minecraft:block", vec![req("pos", custom::block_pos())]),
        ("minecraft:entity", vec![req("source_entity", (Codec::Uuid, Stream::Uuid)), def("y_offset", float(Codec::float()), f(0.0))]),
    ];
    let position_source = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:position_source_type"), cases }));
    (Codec::Normalize(Box::new(position_source), reject_entity_position_source), Stream::Dispatch)
}

fn reject_entity_position_source(source: Nbt) -> Result<Nbt, String> {
    if source.get("type").and_then(Nbt::as_str) == Some("minecraft:entity") {
        return Err("Entity position sources are not allowed".into());
    }
    Ok(source)
}

fn simple_particle_type() -> Pair {
    (Codec::Unit, Stream::Unit)
}

static CODECS: LazyLock<HashMap<&'static str, Pair>> = LazyLock::new(|| {
    crisp_data::registry::PARTICLE_TYPE
        .iter()
        .map(|&name| {
            let pair = match name {
                "minecraft:block" | "minecraft:block_marker" | "minecraft:falling_dust" | "minecraft:dust_pillar" | "minecraft:block_crumble" => {
                    block_particle_option()
                }
                "minecraft:geyser" | "minecraft:geyser_plume" => geyser_particle_options(),
                "minecraft:geyser_base" | "minecraft:geyser_poof" => geyser_base_particle_options(),
                "minecraft:dragon_breath" => power_particle_option(),
                "minecraft:dust" => dust_particle_options(),
                "minecraft:dust_color_transition" => dust_color_transition_options(),
                "minecraft:effect" | "minecraft:instant_effect" => spell_particle_option(),
                "minecraft:entity_effect" | "minecraft:tinted_leaves" | "minecraft:flash" => color_particle_option(),
                "minecraft:sculk_charge" => sculk_charge_particle_options(),
                "minecraft:item" => item_particle_option(),
                "minecraft:vibration" => vibration_particle_option(),
                "minecraft:trail" => trail_particle_option(),
                "minecraft:shriek" => shriek_particle_option(),
                _ => simple_particle_type(),
            };
            (name, pair)
        })
        .collect()
});

pub fn codec(particle_type: &str) -> &'static Pair {
    &CODECS[particle_type]
}

pub fn parse(particle_type: &'static str, options: &Nbt) -> Result<ParticleOptions, String> {
    let (codec, _) = codec(particle_type);
    Ok(ParticleOptions { particle_type, options: codec.decode(options)? })
}

struct BlockStateCodec;

impl BlockStateCodec {
    fn decode_state(tag: &Nbt) -> Result<crisp_data::BlockState, String> {
        let fields = codec::map(tag)?;
        let Some((_, id)) = fields.iter().find(|(k, _)| k == "id") else {
            return Err(format!("No key id in MapLike[{}]", crate::nbt::to_string(tag)));
        };
        let id = Codec::Element("minecraft:block").decode(id)?;
        let block = crisp_data::Block::by_name(id.as_str().unwrap_or("")).expect("registered block");
        let mut state = block.default_state();
        if let Some((_, Nbt::Compound(properties))) = fields.iter().find(|(k, _)| k == "properties") {
            for (name, value) in properties {
                if let Some(value) = value.as_str()
                    && let Some(with) = state.with_property_by_name(name, value)
                {
                    state = with;
                }
            }
        }
        Ok(state)
    }
}

impl Custom for BlockStateCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let by_name = || {
            Codec::Element("minecraft:block").decode(tag).map(|id| crisp_data::Block::by_name(id.as_str().unwrap_or("")).expect("registered block").default_state())
        };
        let block_state = by_name().or_else(|first| {
            BlockStateCodec::decode_state(tag).map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}"))
        });
        let state = block_state.or_else(|first| by_name().map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}")))?;
        Ok(Nbt::Int(state.id() as i32))
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        w.varint(codec::as_i32(value));
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Ok(Nbt::Int(r.varint().map_err(|e| e.to_string())?))
    }
}

struct Vec3Codec;

impl Custom for Vec3Codec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let values = Codec::list(Codec::Double).decode(tag)?;
        if values.as_list().len() != 3 {
            return Err("Input is not a list of 3 elements".into());
        }
        Ok(values)
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        for v in value.as_list() {
            w.f64(codec::as_f64(v));
        }
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let mut values = Vec::with_capacity(3);
        for _ in 0..3 {
            values.push(Nbt::Double(r.f64().map_err(|e| e.to_string())?));
        }
        Ok(Nbt::List(values))
    }
}
