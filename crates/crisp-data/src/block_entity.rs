use crate::{BLOCK_ENTITY_TYPE_OF_BLOCK, BLOCK_ENTITY_TYPES, Block, BlockState};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockEntityType(pub u16);

pub struct BlockEntityTypeInfo {
    pub name: &'static str,
    pub valid_blocks: &'static [Block],
    pub only_op_can_set_nbt: bool,
}

impl BlockEntityType {
    pub fn info(self) -> &'static BlockEntityTypeInfo {
        &BLOCK_ENTITY_TYPES[self.0 as usize]
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn by_name(name: &str) -> Option<BlockEntityType> {
        BLOCK_ENTITY_TYPES.iter().position(|t| t.name == name).map(|i| BlockEntityType(i as u16))
    }

    pub fn count() -> usize {
        BLOCK_ENTITY_TYPES.len()
    }

    pub fn is_valid(self, state: BlockState) -> bool {
        self.info().valid_blocks.binary_search(&state.block()).is_ok()
    }

    pub fn only_op_can_set_nbt(self) -> bool {
        self.info().only_op_can_set_nbt
    }
}

impl std::fmt::Debug for BlockEntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Block {
    pub fn block_entity_type(self) -> Option<BlockEntityType> {
        let ty = BLOCK_ENTITY_TYPE_OF_BLOCK[self.0 as usize];
        (ty != u16::MAX).then_some(BlockEntityType(ty))
    }
}
