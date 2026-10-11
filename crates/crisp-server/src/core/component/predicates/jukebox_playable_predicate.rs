use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::registry;

#[derive(Debug)]
pub struct JukeboxPlayablePredicate {
    song: Option<HolderSet>,
}

impl JukeboxPlayablePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<JukeboxPlayablePredicate> {
        codec::map(tag)?;
        Ok(JukeboxPlayablePredicate { song: codec::decode_optional(tag, "song", |t| HolderSet::decode("minecraft:jukebox_song", t))? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let Some(song) = &self.song else { return true };
        let Some(song_id) = value.as_str() else { return false };
        let songs = registry("minecraft:jukebox_song");
        song.contents().iter().any(|&id| songs.get(id as usize) == Some(&song_id))
    }
}
