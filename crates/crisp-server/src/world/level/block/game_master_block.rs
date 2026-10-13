use crisp_data::Block;

pub fn is_game_master_block(block: Block) -> bool {
    ["CommandBlock", "JigsawBlock", "StructureBlock", "TestBlock", "TestInstanceBlock"].iter().any(|class| block.is_instance_of(class))
}
