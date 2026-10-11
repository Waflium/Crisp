use std::io;

use crisp_core::ChunkPos;
use crisp_core::BlockPos;
use crisp_data::{Block, BlockState, DimensionType, Fluid, WORLD_VERSION, registry};
use crisp_protocol::nbt::Nbt;

use super::data_layer::{self, DataLayer};
use super::palette::{PackedData, PalettedContainer};
use super::{Biome, ChunkStructures, HeightmapType, LevelChunk, LevelChunkSection, biome_strategy, block_strategy};
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use crate::world::ticks::{LevelChunkTicks, SavedTick, TickPriority};

pub struct ReadChunk {
    pub chunk: LevelChunk,
    pub block_ticks: LevelChunkTicks<Block>,
    pub fluid_ticks: LevelChunkTicks<Fluid>,
    pub light: Vec<SectionLight>,
}

pub struct SectionLight {
    pub y: i32,
    pub block: Option<DataLayer>,
    pub sky: Option<DataLayer>,
}

fn layer_to_nbt(layer: &DataLayer) -> Nbt {
    Nbt::ByteArray(layer.to_bytes().iter().map(|&b| b as i8).collect())
}

fn layer_from_nbt(nbt: Option<&Nbt>) -> io::Result<Option<DataLayer>> {
    match nbt {
        Some(Nbt::ByteArray(bytes)) => {
            let bytes: Box<[u8; data_layer::SIZE]> = bytes
                .iter()
                .map(|&b| b as u8)
                .collect::<Vec<u8>>()
                .into_boxed_slice()
                .try_into()
                .map_err(|_| invalid(&format!("DataLayer should be 2048 bytes not: {}", bytes.len())))?;
            Ok(Some(DataLayer::from_bytes(bytes)))
        }
        _ => Ok(None),
    }
}

fn fluid_by_name(name: &str) -> Option<Fluid> {
    let id = registry::FLUID.iter().position(|&f| f == name)?;
    Some([Fluid::Empty, Fluid::FlowingWater, Fluid::Water, Fluid::FlowingLava, Fluid::Lava][id])
}

fn ticks_to_nbt<T: Copy>(ticks: &[SavedTick<T>], name: impl Fn(T) -> &'static str) -> Nbt {
    Nbt::List(
        ticks
            .iter()
            .map(|t| {
                Nbt::Compound(vec![
                    ("i".into(), Nbt::String(name(t.ty).into())),
                    ("x".into(), Nbt::Int(t.pos.x)),
                    ("y".into(), Nbt::Int(t.pos.y)),
                    ("z".into(), Nbt::Int(t.pos.z)),
                    ("t".into(), Nbt::Int(t.delay)),
                    ("p".into(), Nbt::Int(t.priority.0)),
                ])
            })
            .collect(),
    )
}

fn ticks_from_nbt<T: Copy + Eq + std::hash::Hash>(nbt: Option<&Nbt>, ty: impl Fn(&str) -> Option<T>) -> LevelChunkTicks<T> {
    let ticks = nbt
        .map(Nbt::as_list)
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| {
            let int = |k: &str| t.get(k).and_then(Nbt::as_i64).map(|v| v as i32);
            Some(SavedTick {
                ty: ty(t.get("i")?.as_str()?)?,
                pos: BlockPos::new(int("x")?, int("y")?, int("z")?),
                delay: int("t")?,
                priority: TickPriority::by_value(int("p").unwrap_or(0)),
            })
        })
        .collect();
    LevelChunkTicks::with_pending(ticks)
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

pub fn state_to_nbt(state: BlockState) -> Nbt {
    let block = state.block();
    if block.default_state() == state {
        return Nbt::String(block.name().into());
    }
    let properties = state.properties().map(|(name, value)| (name.to_string(), Nbt::String(value.into()))).collect();
    Nbt::Compound(vec![("id".into(), Nbt::String(block.name().into())), ("properties".into(), Nbt::Compound(properties))])
}

pub fn state_from_nbt(nbt: &Nbt) -> Option<BlockState> {
    match nbt {
        Nbt::String(name) => Some(Block::by_name(name)?.default_state()),
        Nbt::Compound(_) if nbt.get("").is_some() => state_from_nbt(nbt.get("")?),
        Nbt::Compound(_) => {
            let mut state = Block::by_name(nbt.get("id")?.as_str()?)?.default_state();
            if let Some(Nbt::Compound(props)) = nbt.get("properties") {
                for (name, value) in props {
                    state = state.with_property_by_name(name, value.as_str()?)?;
                }
            }
            Some(state)
        }
        _ => None,
    }
}

fn mixed_list(items: Vec<Nbt>) -> Nbt {
    Nbt::List(items)
}

fn container_to_nbt<T: super::palette::PaletteValue>(container: &PalettedContainer<T>, entry: impl Fn(T) -> Nbt) -> Nbt {
    let packed = container.pack();
    let mut fields = vec![("palette".to_string(), mixed_list(packed.palette.into_iter().map(entry).collect()))];
    if let Some(data) = packed.storage {
        fields.push(("data".to_string(), Nbt::LongArray(data)));
    }
    Nbt::Compound(fields)
}

fn container_from_nbt<T: super::palette::PaletteValue>(
    nbt: &Nbt,
    strategy: super::palette::Strategy,
    entry: impl Fn(&Nbt) -> Option<T>,
) -> io::Result<PalettedContainer<T>> {
    let palette = nbt.get("palette").map(Nbt::as_list).unwrap_or(&[]).iter().map(&entry).collect::<Option<Vec<T>>>();
    let palette = palette.ok_or_else(|| invalid("unknown palette entry"))?;
    let storage = match nbt.get("data") {
        Some(Nbt::LongArray(longs)) => Some(longs.clone()),
        _ => None,
    };
    PalettedContainer::unpack(strategy, PackedData { palette, storage }).map_err(|e| invalid(&e))
}

pub fn write(chunk: &LevelChunk, last_update: i64, block_ticks: &[SavedTick<Block>], fluid_ticks: &[SavedTick<Fluid>], light: &[SectionLight]) -> Nbt {
    let min_section = chunk.min_y() >> 4;
    let mut sections = Vec::new();
    for section_y in min_section - 1..=min_section + chunk.sections.len() as i32 {
        let section = chunk.sections.get((section_y - min_section) as usize).filter(|_| section_y >= min_section);
        let light = light.iter().find(|l| l.y == section_y);
        let block_light = light.and_then(|l| l.block.as_ref());
        let sky_light = light.and_then(|l| l.sky.as_ref());
        if section.is_none() && block_light.is_none() && sky_light.is_none() {
            continue;
        }
        let mut tag = Vec::new();
        if let Some(s) = section {
            tag.push(("block_states".into(), container_to_nbt(&s.states, state_to_nbt)));
            tag.push(("biomes".into(), container_to_nbt(&s.biomes, |b| Nbt::String(b.name().into()))));
        }
        if let Some(layer) = block_light {
            tag.push(("BlockLight".into(), layer_to_nbt(layer)));
        }
        if let Some(layer) = sky_light {
            tag.push(("SkyLight".into(), layer_to_nbt(layer)));
        }
        tag.push(("Y".into(), Nbt::Byte(section_y as i8)));
        sections.push(Nbt::Compound(tag));
    }
    let heightmaps = chunk.heightmaps.iter().map(|h| (h.kind.serialization_key().to_string(), Nbt::LongArray(h.raw().to_vec()))).collect();
    let mut tag = vec![
        ("DataVersion".into(), Nbt::Int(WORLD_VERSION)),
        ("xPos".into(), Nbt::Int(chunk.pos.x)),
        ("yPos".into(), Nbt::Int(min_section)),
        ("zPos".into(), Nbt::Int(chunk.pos.z)),
        ("LastUpdate".into(), Nbt::Long(last_update)),
        ("InhabitedTime".into(), Nbt::Long(chunk.inhabited_time)),
        ("Status".into(), Nbt::String("minecraft:full".into())),
        ("sections".into(), Nbt::List(sections)),
    ];
    if chunk.light_correct {
        tag.push(("isLightOn".into(), Nbt::Byte(1)));
    }
    tag.extend([
        ("block_entities".into(), Nbt::List(chunk.block_entities_for_saving())),
        ("block_ticks".into(), ticks_to_nbt(block_ticks, Block::name)),
        ("fluid_ticks".into(), ticks_to_nbt(fluid_ticks, |f| registry::FLUID[f as usize])),
        ("PostProcessing".into(), Nbt::List(chunk.sections.iter().map(|_| Nbt::List(Vec::new())).collect())),
        ("Heightmaps".into(), Nbt::Compound(heightmaps)),
        ("structures".into(), pack_structure_data(chunk.pos, &chunk.structures)),
    ]);
    Nbt::Compound(tag)
}

fn pack_structure_data(pos: ChunkPos, structures: &ChunkStructures) -> Nbt {
    let starts = structures.starts.iter().map(|(structure, start)| (structure.key().to_owned(), start.create_tag(pos))).collect();
    let references = structures
        .references
        .iter()
        .filter(|(_, set)| !set.is_empty())
        .map(|(structure, set)| (structure.key().to_owned(), Nbt::LongArray(set.iter().collect())))
        .collect();
    Nbt::Compound(vec![("References".into(), Nbt::Compound(references)), ("starts".into(), Nbt::Compound(starts))])
}

fn unpack_structure_data(pos: ChunkPos, tag: Option<&Nbt>) -> ChunkStructures {
    let mut structures = ChunkStructures::default();
    let Some(tag) = tag else { return structures };
    if let Some(Nbt::Compound(starts)) = tag.get("starts") {
        for (key, start_tag) in starts {
            match StructureHolder::by_name(key) {
                None => error!("Unknown structure start: {key}"),
                Some(structure) => {
                    if let Some(Some(start)) = StructureStart::load_static_start(start_tag) {
                        structures.starts.insert(structure, std::sync::Arc::new(start));
                    }
                }
            }
        }
    }
    if let Some(Nbt::Compound(references)) = tag.get("References") {
        for (key, entry) in references {
            let Some(structure) = StructureHolder::by_name(key) else {
                warn!("Found reference to unknown structure '{key}' in chunk {pos:?}, discarding");
                continue;
            };
            let Nbt::LongArray(values) = entry else { continue };
            let mut set = crisp_core::fastutil::LongOpenHashSet::with_expected(values.len(), 0.75);
            for &value in values {
                let ref_pos = ChunkPos::unpack(value);
                if ref_pos.chessboard_distance(pos) > 8 {
                    warn!("Found invalid structure reference [ {key} @ {ref_pos:?} ] for chunk {pos:?}.");
                } else {
                    set.add(value);
                }
            }
            structures.references.insert(structure, set);
        }
    }
    structures
}

pub fn read(nbt: &Nbt, dimension: &DimensionType) -> io::Result<ReadChunk> {
    let version = nbt.get("DataVersion").and_then(Nbt::as_i64).unwrap_or(0);
    if version != WORLD_VERSION as i64 {
        return Err(invalid(&format!("chunk data version {version} is not {WORLD_VERSION}")));
    }
    let x = nbt.get("xPos").and_then(Nbt::as_i64).ok_or_else(|| invalid("missing xPos"))? as i32;
    let z = nbt.get("zPos").and_then(Nbt::as_i64).ok_or_else(|| invalid("missing zPos"))? as i32;
    let plains = Biome::by_name("minecraft:plains").unwrap_or(Biome(0));
    let mut sections: Vec<LevelChunkSection> = (0..dimension.height / 16).map(|_| LevelChunkSection::new(plains)).collect();
    let min_section = dimension.min_y >> 4;
    let mut light = Vec::new();
    for section in nbt.get("sections").map(Nbt::as_list).unwrap_or(&[]) {
        let y = section.get("Y").and_then(Nbt::as_i64).ok_or_else(|| invalid("section without Y"))? as i32;
        let (block, sky) = (layer_from_nbt(section.get("BlockLight"))?, layer_from_nbt(section.get("SkyLight"))?);
        if block.is_some() || sky.is_some() {
            light.push(SectionLight { y, block, sky });
        }
        let Some(target) = sections.get_mut((y - min_section) as usize) else { continue };
        let states = match section.get("block_states") {
            Some(s) => container_from_nbt(s, block_strategy(), state_from_nbt)?,
            None => target.states.clone(),
        };
        let biomes = match section.get("biomes") {
            Some(b) => container_from_nbt(b, biome_strategy(), |n| n.as_str().and_then(Biome::by_name))?,
            None => target.biomes.clone(),
        };
        *target = LevelChunkSection::from_containers(states, biomes);
    }
    let mut chunk = LevelChunk::with_sections(ChunkPos::new(x, z), dimension, sections);
    chunk.inhabited_time = nbt.get("InhabitedTime").and_then(Nbt::as_i64).unwrap_or(0);
    chunk.light_correct = nbt.get("isLightOn").and_then(Nbt::as_i64).is_some_and(|b| b != 0);
    chunk.load_block_entities(nbt.get("block_entities").map(Nbt::as_list).unwrap_or(&[]));
    chunk.structures = unpack_structure_data(chunk.pos, nbt.get("structures"));
    let mut missing = false;
    for kind in HeightmapType::LIVE {
        let i = HeightmapType::LIVE.iter().position(|&t| t == kind).unwrap();
        match nbt.get("Heightmaps").and_then(|h| h.get(kind.serialization_key())) {
            Some(Nbt::LongArray(data)) if chunk.heightmaps[i].set_raw(data) => {}
            _ => missing = true,
        }
    }
    if missing {
        chunk.prime_heightmaps();
    }
    let block_ticks = ticks_from_nbt(nbt.get("block_ticks"), Block::by_name);
    let fluid_ticks = ticks_from_nbt(nbt.get("fluid_ticks"), fluid_by_name);
    Ok(ReadChunk { chunk, block_ticks, fluid_ticks, light })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::level::chunk::FlatLevelSource;
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::HORIZONTAL_FACING;
    use crisp_core::Direction;

    fn overworld() -> &'static DimensionType {
        &crisp_data::DIMENSION_TYPES[0]
    }

    #[test]
    fn chunk_round_trip() {
        let mut chunk = FlatLevelSource::default().generate(ChunkPos::new(3, -7), overworld());
        let stairs = blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::East);
        chunk.set_block_state(BlockPos::new(49, -60, -110), stairs);
        chunk.set_block_state(BlockPos::new(53, 100, -103), blocks::GLOWSTONE.default_state());
        let read = read(&write(&chunk, 0, &[], &[], &[]), overworld()).unwrap().chunk;
        assert_eq!(read.pos, ChunkPos::new(3, -7));
        assert_eq!(read.block_state(BlockPos::new(49, -60, -110)), stairs);
        assert_eq!(read.block_state(BlockPos::new(53, 100, -103)), blocks::GLOWSTONE.default_state());
        assert_eq!(read.block_state(BlockPos::new(48, -64, -112)), blocks::BEDROCK.default_state());
    }

    #[test]
    fn mixed_palette_wraps_ids() {
        let stairs = blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::East);
        let list = mixed_list(vec![state_to_nbt(blocks::DIRT.default_state()), state_to_nbt(stairs)]);
        let mut w = crisp_protocol::Writer::new();
        list.write(&mut w);
        assert_eq!(&w.as_slice()[..2], &[9, 10]);
        assert_eq!(Nbt::read(&mut crisp_protocol::Reader::new(w.as_slice())).unwrap(), list);
        assert_eq!(list.as_list().iter().map(state_from_nbt).collect::<Vec<_>>(), [Some(blocks::DIRT.default_state()), Some(stairs)]);
    }
}
