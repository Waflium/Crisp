use std::io;
use std::path::PathBuf;

use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use crate::world::level::storage::{read_dat, write_dat};

pub struct ScoreboardSaveData {
    dir: Option<PathBuf>,
    data: Nbt,
    dirty: bool,
}

impl ScoreboardSaveData {
    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("scoreboard.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> ScoreboardSaveData {
        let data = dir.as_deref().and_then(|dir| match read_dat(&ScoreboardSaveData::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:scoreboard]");
                None
            }
        });
        let dirty = data.is_none();
        ScoreboardSaveData { dir, data: data.unwrap_or(Nbt::Compound(Vec::new())), dirty }
    }

    pub fn data(&self) -> &Nbt {
        &self.data
    }

    pub fn set_data(&mut self, data: Nbt) {
        if data != self.data {
            self.data = data;
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let file = Nbt::Compound(vec![("data".into(), self.data.clone()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&ScoreboardSaveData::file(dir), &file)?;
        self.dirty = false;
        Ok(())
    }
}
