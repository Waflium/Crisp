use std::io::Read;
use std::sync::{LazyLock, OnceLock};

use flate2::read::GzDecoder;

use super::structure_template::StructureTemplate;
use crate::world::level::chunk::storage::decode_nbt;

static TEMPLATES: LazyLock<Vec<OnceLock<StructureTemplate>>> = LazyLock::new(|| (0..crisp_data::STRUCTURES.len()).map(|_| OnceLock::new()).collect());

static EMPTY: StructureTemplate = StructureTemplate::EMPTY;

pub fn get_or_create(id: &str) -> &'static StructureTemplate {
    match crisp_data::STRUCTURES.binary_search_by(|(name, _)| (*name).cmp(id)) {
        Ok(index) => TEMPLATES[index].get_or_init(|| load_from_resource(id, crisp_data::STRUCTURES[index].1)),
        Err(_) => &EMPTY,
    }
}

fn load_from_resource(id: &str, bytes: &[u8]) -> StructureTemplate {
    let mut decompressed = Vec::new();
    let tag = GzDecoder::new(bytes).read_to_end(&mut decompressed).and_then(|_| decode_nbt(&decompressed));
    match tag {
        Ok(tag) => StructureTemplate::load(&tag),
        Err(e) => {
            error!("Couldn't load structure {id}: {e}");
            StructureTemplate::EMPTY
        }
    }
}
