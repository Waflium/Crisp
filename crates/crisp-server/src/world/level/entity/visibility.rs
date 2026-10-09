use crate::server::level::chunk_level::FullChunkStatus;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Visibility {
    #[default]
    Hidden,
    Tracked,
    Ticking,
}

impl Visibility {
    pub fn is_ticking(self) -> bool {
        self == Visibility::Ticking
    }

    pub fn is_accessible(self) -> bool {
        self != Visibility::Hidden
    }

    pub fn from_full_chunk_status(status: FullChunkStatus) -> Visibility {
        if status.is_or_after(FullChunkStatus::EntityTicking) {
            Visibility::Ticking
        } else if status.is_or_after(FullChunkStatus::Full) {
            Visibility::Tracked
        } else {
            Visibility::Hidden
        }
    }
}
