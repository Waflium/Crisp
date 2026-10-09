#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChunkStatus {
    Empty,
    Features,
    InitializeLight,
    Light,
    Full,
}

impl ChunkStatus {
    pub const ALL: [ChunkStatus; 5] = [ChunkStatus::Empty, ChunkStatus::Features, ChunkStatus::InitializeLight, ChunkStatus::Light, ChunkStatus::Full];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parent(self) -> ChunkStatus {
        ChunkStatus::ALL[self.index().saturating_sub(1)]
    }

    pub fn next(self) -> ChunkStatus {
        ChunkStatus::ALL[(self.index() + 1).min(4)]
    }

    fn accumulated_dependencies(self) -> &'static [ChunkStatus] {
        use ChunkStatus::*;
        match self {
            Empty => &[],
            Features => &[Empty],
            InitializeLight => &[Features],
            Light => &[InitializeLight, InitializeLight],
            Full => &[Light, InitializeLight],
        }
    }

    pub fn accumulated_radius_of(self, status: ChunkStatus) -> i32 {
        if status == self {
            return 0;
        }
        let dependencies = self.accumulated_dependencies();
        dependencies.iter().rposition(|&d| status <= d).expect("status within the step's dependencies") as i32
    }

    pub fn dependency_at(self, distance: i32) -> ChunkStatus {
        self.accumulated_dependencies()[distance as usize]
    }

    pub fn dependency_radius(self) -> i32 {
        (self.accumulated_dependencies().len() as i32 - 1).max(0)
    }
}

pub const FULL_CHUNK_LEVEL: i32 = 33;
pub const BLOCK_TICKING_LEVEL: i32 = 32;
pub const ENTITY_TICKING_LEVEL: i32 = 31;
pub const RADIUS_AROUND_FULL_CHUNK: i32 = 11;
pub const MAX_LEVEL: i32 = FULL_CHUNK_LEVEL + RADIUS_AROUND_FULL_CHUNK;

pub fn status_around_full_chunk(distance: i32) -> Option<ChunkStatus> {
    match distance {
        ..=0 => Some(ChunkStatus::Full),
        1 => Some(ChunkStatus::InitializeLight),
        2 => Some(ChunkStatus::Features),
        3..=RADIUS_AROUND_FULL_CHUNK => Some(ChunkStatus::Empty),
        _ => None,
    }
}

pub fn generation_status(level: i32) -> Option<ChunkStatus> {
    status_around_full_chunk(level - FULL_CHUNK_LEVEL)
}

pub fn by_status(status: ChunkStatus) -> i32 {
    match status {
        ChunkStatus::Full | ChunkStatus::Light => FULL_CHUNK_LEVEL,
        ChunkStatus::InitializeLight | ChunkStatus::Features => FULL_CHUNK_LEVEL + 1,
        ChunkStatus::Empty => MAX_LEVEL,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FullChunkStatus {
    Inaccessible,
    Full,
    BlockTicking,
    EntityTicking,
}

impl FullChunkStatus {
    pub fn is_or_after(self, status: FullChunkStatus) -> bool {
        self >= status
    }
}

pub fn full_status(level: i32) -> FullChunkStatus {
    if level <= ENTITY_TICKING_LEVEL {
        FullChunkStatus::EntityTicking
    } else if level <= BLOCK_TICKING_LEVEL {
        FullChunkStatus::BlockTicking
    } else if level <= FULL_CHUNK_LEVEL {
        FullChunkStatus::Full
    } else {
        FullChunkStatus::Inaccessible
    }
}

pub fn by_full_status(status: FullChunkStatus) -> i32 {
    match status {
        FullChunkStatus::Inaccessible => MAX_LEVEL,
        FullChunkStatus::Full => FULL_CHUNK_LEVEL,
        FullChunkStatus::BlockTicking => BLOCK_TICKING_LEVEL,
        FullChunkStatus::EntityTicking => ENTITY_TICKING_LEVEL,
    }
}

pub fn is_entity_ticking(level: i32) -> bool {
    level <= ENTITY_TICKING_LEVEL
}

pub fn is_block_ticking(level: i32) -> bool {
    level <= BLOCK_TICKING_LEVEL
}

pub fn is_loaded(level: i32) -> bool {
    level <= MAX_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_match_vanilla() {
        assert_eq!(MAX_LEVEL, 44);
        assert_eq!(generation_status(31), Some(ChunkStatus::Full));
        assert_eq!(generation_status(33), Some(ChunkStatus::Full));
        assert_eq!(generation_status(34), Some(ChunkStatus::InitializeLight));
        assert_eq!(generation_status(35), Some(ChunkStatus::Features));
        assert_eq!(generation_status(36), Some(ChunkStatus::Empty));
        assert_eq!(generation_status(44), Some(ChunkStatus::Empty));
        assert_eq!(generation_status(45), None);
    }

    #[test]
    fn pyramid_radii() {
        use ChunkStatus::*;
        assert_eq!(Full.accumulated_radius_of(Full), 0);
        assert_eq!(Full.accumulated_radius_of(Light), 0);
        assert_eq!(Full.accumulated_radius_of(InitializeLight), 1);
        assert_eq!(Full.accumulated_radius_of(Empty), 1);
        assert_eq!(Light.accumulated_radius_of(Features), 1);
        assert_eq!(InitializeLight.accumulated_radius_of(Empty), 0);
        assert_eq!(Full.dependency_at(1), InitializeLight);
    }
}
