#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StructureCheckResult {
    StartPresent,
    StartNotPresent,
    ChunkLoadNeeded,
}
