use std::io;
use std::path::{Path, PathBuf};

use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use crate::world::level::storage::{read_dat, write_dat};

pub struct Raids {
    dir: Option<PathBuf>,
    raids: Option<Nbt>,
    next_id: i32,
    tick: i32,
    dirty: bool,
}

impl Raids {
    fn file(dir: &Path) -> PathBuf {
        dir.join("minecraft").join("raids.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> Raids {
        let data = dir.as_deref().and_then(|dir| match read_dat(&Raids::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:raids]");
                None
            }
        });
        let field = |name: &str| data.as_ref().and_then(|d| d.get(name)).and_then(Nbt::as_i64).map(|v| v as i32);
        match (field("next_id"), field("tick")) {
            (Some(next_id), Some(tick)) => {
                let raids = data.as_ref().and_then(|d| d.get("raids")).cloned();
                Raids { dir, raids, next_id, tick, dirty: false }
            }
            _ => Raids { dir, raids: None, next_id: 1, tick: 0, dirty: true },
        }
    }

    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.tick % 200 == 0 {
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let mut data = Vec::new();
        if let Some(raids) = &self.raids
            && !matches!(raids, Nbt::List(list) if list.is_empty())
        {
            data.push(("raids".into(), raids.clone()));
        }
        data.push(("next_id".into(), Nbt::Int(self.next_id)));
        data.push(("tick".into(), Nbt::Int(self.tick)));
        write_dat(&Raids::file(dir), &Nbt::Compound(vec![("data".into(), Nbt::Compound(data)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]))
    }
}
