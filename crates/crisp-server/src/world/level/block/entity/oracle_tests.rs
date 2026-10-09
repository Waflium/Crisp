use crisp_core::BlockPos;
use crisp_data::Block;
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::BlockEntity;
use super::oracle_data::ORACLE;

fn hex(nbt: &Nbt) -> String {
    let mut w = Writer::new();
    nbt.write(&mut w);
    w.as_slice().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn block_entities_save_like_vanilla() {
    let mut failures = Vec::new();
    let mut lines = ORACLE.lines().filter(|l| !l.trim().is_empty());
    while let Some(case) = lines.next() {
        let (block, snbt) = case.strip_prefix("CASE ").unwrap().split_once('\t').unwrap();
        let save = lines.next().unwrap().strip_prefix("SAVE ").unwrap();
        let components = lines.next().unwrap().strip_prefix("COMPONENTS ").unwrap();
        let state = Block::by_name(block).unwrap().default_state();
        let Nbt::Compound(mut tag) = crate::nbt::parse_fully(snbt).unwrap() else { panic!("{snbt}") };
        tag.push(("id".into(), Nbt::String(state.block().block_entity_type().unwrap().name().into())));
        let entity = BlockEntity::load_static(BlockPos::new(1, 64, -3), state, &Nbt::Compound(tag)).unwrap();
        let saved = hex(&entity.save_with_full_metadata());
        if saved != save {
            failures.push(format!("{block} {snbt}\n  save: {}\n  want: {save}", crate::nbt::to_string(&entity.save_with_full_metadata())));
        }
        let collected = Nbt::Compound(entity.collect_components().into_iter().filter_map(|(ty, v)| Some((ty.name().to_string(), v?))).collect());
        if hex(&collected) != components {
            failures.push(format!("{block} {snbt}\n  components: {} ({})\n  want: {components}", crate::nbt::to_string(&collected), hex(&collected)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
