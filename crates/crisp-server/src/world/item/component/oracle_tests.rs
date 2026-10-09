use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use super::DataComponentType;
use super::oracle_data::ORACLE;
use crate::commands::arguments::item::item_parser;
use crate::commands::brigadier::StringReader;
use crate::network::hashed_stack::HashedStack;
use crate::world::item::{ItemStack, read_slot, write_slot};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn stack(case: &str) -> ItemStack {
    let (count, input) = case.split_once(' ').unwrap();
    let units: Vec<u16> = input.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    let input = item_parser::parse(&mut reader).unwrap_or_else(|e| panic!("{case}: {}", e.get_message()));
    ItemStack::with_patch(input.item, count.parse().unwrap(), &input.components)
}

struct Case {
    input: String,
    lines: Vec<(String, String)>,
}

fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for line in ORACLE.lines().filter(|l| !l.trim().is_empty()) {
        let (kind, rest) = line.split_once(' ').unwrap();
        if kind == "CASE" {
            cases.push(Case { input: rest.to_string(), lines: Vec::new() });
        } else {
            cases.last_mut().unwrap().lines.push((kind.to_string(), rest.to_string()));
        }
    }
    cases
}

fn same_hashed(a: &HashedStack, b: &HashedStack) -> bool {
    match (a, b) {
        (HashedStack::ActualItem { item: i1, count: c1, components: p1 }, HashedStack::ActualItem { item: i2, count: c2, components: p2 }) => {
            let sorted = |mut v: Vec<(u16, i32)>| {
                v.sort_unstable();
                v
            };
            let added = |p: &crate::network::hashed_stack::HashedPatchMap| sorted(p.added_components.iter().map(|(t, h)| (t.0, *h)).collect());
            let removed = |p: &crate::network::hashed_stack::HashedPatchMap| sorted(p.removed_components.iter().map(|t| (t.0, 0)).collect());
            i1 == i2 && c1 == c2 && added(p1) == added(p2) && removed(p1) == removed(p2)
        }
        (a, b) => a == b,
    }
}

fn expected<'a>(case: &'a Case, kind: &str) -> &'a str {
    &case.lines.iter().find(|(k, _)| k == kind).unwrap().1
}

#[test]
fn stacks_encode_like_vanilla() {
    let mut failures = Vec::new();
    for case in cases() {
        let stack = stack(&case.input);
        let mut net = Writer::new();
        write_slot(&mut net, Some(&stack));
        let mut untrusted = Writer::new();
        untrusted.varint(stack.count);
        untrusted.varint(stack.item.0 as i32);
        stack.components_patch().write(&mut untrusted, true).unwrap();
        let mut nbt = Writer::new();
        stack.to_nbt().write(&mut nbt);
        let vanilla = HashedStack::read(&mut Reader::new(&unhex(expected(&case, "HASHED")))).unwrap();
        if !same_hashed(&vanilla, &HashedStack::create(Some(&stack))) {
            failures.push(format!("HASHED {}\n  vanilla: {vanilla:?}\n  crisp:   {:?}", case.input, HashedStack::create(Some(&stack))));
        }
        for (kind, actual) in [("NET", net), ("UNTRUSTED", untrusted), ("NBT", nbt)] {
            if hex(actual.as_slice()) != expected(&case, kind) {
                failures.push(format!("{kind} {}\n  vanilla: {}\n  crisp:   {}", case.input, expected(&case, kind), hex(actual.as_slice())));
            }
        }
        for (kind, line) in &case.lines {
            if kind != "HASH" {
                continue;
            }
            let (id, hash) = line.split_once(' ').unwrap();
            let ty = DataComponentType::by_name(id).unwrap();
            let actual = ty.hash(stack.get(ty).unwrap()) as i32;
            if actual.to_string() != hash {
                failures.push(format!("HASH {id} of {}: vanilla {hash}, crisp {actual}", case.input));
            }
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn stacks_decode_like_vanilla() {
    let mut failures = Vec::new();
    for case in cases() {
        let stack = stack(&case.input);
        let net = unhex(expected(&case, "NET"));
        match read_slot(&mut Reader::new(&net), false) {
            Ok(Some(read)) if read == stack => {}
            other => failures.push(format!("NET {}: {other:?}", case.input)),
        }
        let untrusted = unhex(expected(&case, "UNTRUSTED"));
        match read_slot(&mut Reader::new(&untrusted), true) {
            Ok(Some(read)) if read == stack => {}
            other => failures.push(format!("UNTRUSTED {}: {other:?}", case.input)),
        }
        let nbt = unhex(expected(&case, "NBT"));
        let tag = Nbt::read(&mut Reader::new(&nbt)).unwrap();
        match ItemStack::from_nbt(&tag) {
            Ok(read) if read == stack => {}
            other => failures.push(format!("NBT {}: {other:?}", case.input)),
        }
        let hashed = unhex(expected(&case, "HASHED"));
        match HashedStack::read(&mut Reader::new(&hashed)) {
            Ok(h) if h.matches(Some(&stack)) => {}
            other => failures.push(format!("HASHED {}: {other:?}", case.input)),
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
