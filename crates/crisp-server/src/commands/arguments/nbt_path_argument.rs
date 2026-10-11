use crisp_protocol::nbt::Nbt;

use crate::commands::brigadier::{CommandSyntaxException, DISPATCHER_PARSE_EXCEPTION, DynamicCommandExceptionType, SimpleCommandExceptionType, StringReader, tr};
use crate::nbt;

pub static ERROR_INVALID_NODE: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.nbtpath.node.invalid");

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    CompoundChild(String),
    MatchObject(String, Nbt),
    MatchRootObject(Nbt),
    AllElements,
    IndexedElement(i32),
    MatchElement(Nbt),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NbtPath {
    pub original: String,
    pub nodes: Vec<(Node, usize)>,
}

pub fn parse(reader: &mut StringReader) -> Result<NbtPath, CommandSyntaxException> {
    let mut nodes = Vec::new();
    let start = reader.cursor;
    let mut first_node = true;
    while reader.can_read() && !reader.at(' ') {
        let node = parse_node(reader, first_node)?;
        nodes.push((node, reader.cursor - start));
        first_node = false;
        if reader.can_read() && !reader.at(' ') && !reader.at('[') && !reader.at('{') {
            reader.expect('.')?;
        }
    }
    Ok(NbtPath { original: reader.substring(start, reader.cursor), nodes })
}

fn parse_node(reader: &mut StringReader, first_node: bool) -> Result<Node, CommandSyntaxException> {
    if reader.at('"') || reader.at('\'') {
        let name = reader.read_string()?;
        return read_object_node(reader, name);
    }
    if reader.at('[') {
        reader.skip();
        if !reader.can_read() {
            return Err(DISPATCHER_PARSE_EXCEPTION.create_with_context(reader, "String index out of range"));
        }
        if reader.at('{') {
            let pattern = nbt::parse_compound_as_argument(reader)?;
            reader.expect(']')?;
            return Ok(Node::MatchElement(pattern));
        }
        if reader.at(']') {
            reader.skip();
            return Ok(Node::AllElements);
        }
        let index = reader.read_int()?;
        reader.expect(']')?;
        return Ok(Node::IndexedElement(index));
    }
    if reader.at('{') {
        if !first_node {
            return Err(ERROR_INVALID_NODE.create_with_context(reader));
        }
        return Ok(Node::MatchRootObject(nbt::parse_compound_as_argument(reader)?));
    }
    let name = read_unquoted_name(reader)?;
    read_object_node(reader, name)
}

fn read_object_node(reader: &mut StringReader, name: String) -> Result<Node, CommandSyntaxException> {
    if name.is_empty() {
        return Err(ERROR_INVALID_NODE.create_with_context(reader));
    }
    if reader.at('{') {
        return Ok(Node::MatchObject(name, nbt::parse_compound_as_argument(reader)?));
    }
    Ok(Node::CompoundChild(name))
}

fn read_unquoted_name(reader: &mut StringReader) -> Result<String, CommandSyntaxException> {
    let start = reader.cursor;
    while reader.can_read() && is_allowed_in_unquoted_name(reader.peek()) {
        reader.skip();
    }
    if reader.cursor == start {
        return Err(ERROR_INVALID_NODE.create_with_context(reader));
    }
    Ok(reader.substring(start, reader.cursor))
}

fn is_allowed_in_unquoted_name(c: u16) -> bool {
    !matches!(c, 0x20 | 0x22 | 0x27 | 0x5B | 0x5D | 0x2E | 0x7B | 0x7D)
}

pub static ERROR_DATA_TOO_DEEP: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.nbtpath.too_deep");
pub static ERROR_NOTHING_FOUND: DynamicCommandExceptionType = DynamicCommandExceptionType(|path| tr("arguments.nbtpath.nothing_found", vec![path]));
static ERROR_EXPECTED_LIST: DynamicCommandExceptionType = DynamicCommandExceptionType(|node| tr("commands.data.modify.expected_list", vec![node]));
static ERROR_INVALID_INDEX: DynamicCommandExceptionType = DynamicCommandExceptionType(|node| tr("commands.data.modify.invalid_index", vec![node]));

#[derive(Clone, Debug, PartialEq)]
enum Step {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug)]
enum Found {
    At(Vec<Step>),
    Value(Nbt),
}

fn at<'a>(root: &'a Nbt, steps: &[Step]) -> &'a Nbt {
    steps.iter().fold(root, |tag, step| match (tag, step) {
        (Nbt::Compound(fields), Step::Key(key)) => &fields.iter().find(|(k, _)| k == key).expect("a path that was just found").1,
        (Nbt::List(items), Step::Index(i)) => &items[*i],
        _ => unreachable!("a path that was just found"),
    })
}

fn at_mut<'a>(root: &'a mut Nbt, steps: &[Step]) -> &'a mut Nbt {
    steps.iter().fold(root, |tag, step| match (tag, step) {
        (Nbt::Compound(fields), Step::Key(key)) => &mut fields.iter_mut().find(|(k, _)| k == key).expect("a path that was just found").1,
        (Nbt::List(items), Step::Index(i)) => &mut items[*i],
        _ => unreachable!("a path that was just found"),
    })
}

fn child(steps: &[Step], step: Step) -> Found {
    let mut steps = steps.to_vec();
    steps.push(step);
    Found::At(steps)
}

fn compound_get<'a>(fields: &'a [(String, Nbt)], key: &str) -> Option<&'a Nbt> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn compound_put(fields: &mut Vec<(String, Nbt)>, key: &str, value: Nbt) -> Option<Nbt> {
    match fields.iter_mut().find(|(k, _)| k == key) {
        Some(entry) => Some(std::mem::replace(&mut entry.1, value)),
        None => {
            fields.push((key.to_string(), value));
            None
        }
    }
}

pub fn compare_nbt(expected: &Nbt, actual: Option<&Nbt>) -> bool {
    let Some(actual) = actual else { return false };
    if std::mem::discriminant(expected) != std::mem::discriminant(actual) {
        return false;
    }
    match (expected, actual) {
        (Nbt::Compound(e), Nbt::Compound(a)) => a.len() >= e.len() && e.iter().all(|(k, v)| compare_nbt(v, compound_get(a, k))),
        (Nbt::List(e), Nbt::List(a)) => {
            if e.is_empty() {
                return a.is_empty();
            }
            a.len() >= e.len() && e.iter().all(|tag| a.iter().any(|value| compare_nbt(tag, Some(value))))
        }
        _ => expected == actual,
    }
}

fn numeric(tag: &Nbt) -> Option<(i8, i32, i64)> {
    Some(match *tag {
        Nbt::Byte(v) => (v, v as i32, v as i64),
        Nbt::Short(v) => (v as i8, v as i32, v as i64),
        Nbt::Int(v) => (v as i8, v, v as i64),
        Nbt::Long(v) => (v as i8, v as i32, v),
        Nbt::Float(v) => ((v.floor() as i32) as i8, v.floor() as i32, v as i64),
        Nbt::Double(v) => ((v.floor() as i32) as i8, v.floor() as i32, v.floor() as i64),
        _ => return None,
    })
}

fn element(tag: &Nbt, i: usize) -> Nbt {
    match tag {
        Nbt::List(items) => items[i].clone(),
        Nbt::ByteArray(v) => Nbt::Byte(v[i]),
        Nbt::IntArray(v) => Nbt::Int(v[i]),
        Nbt::LongArray(v) => Nbt::Long(v[i]),
        _ => unreachable!("not a collection"),
    }
}

fn collection_len(tag: &Nbt) -> Option<usize> {
    match tag {
        Nbt::List(items) => Some(items.len()),
        Nbt::ByteArray(v) => Some(v.len()),
        Nbt::IntArray(v) => Some(v.len()),
        Nbt::LongArray(v) => Some(v.len()),
        _ => None,
    }
}

fn add_tag(tag: &mut Nbt, index: usize, value: Nbt) -> bool {
    match tag {
        Nbt::List(items) => {
            items.insert(index, value);
            true
        }
        Nbt::ByteArray(v) => numeric(&value).map(|n| v.insert(index, n.0)).is_some(),
        Nbt::IntArray(v) => numeric(&value).map(|n| v.insert(index, n.1)).is_some(),
        Nbt::LongArray(v) => numeric(&value).map(|n| v.insert(index, n.2)).is_some(),
        _ => false,
    }
}

fn set_tag(tag: &mut Nbt, index: usize, value: Nbt) -> bool {
    match tag {
        Nbt::List(items) => {
            items[index] = value;
            true
        }
        Nbt::ByteArray(v) => numeric(&value).map(|n| v[index] = n.0).is_some(),
        Nbt::IntArray(v) => numeric(&value).map(|n| v[index] = n.1).is_some(),
        Nbt::LongArray(v) => numeric(&value).map(|n| v[index] = n.2).is_some(),
        _ => false,
    }
}

fn remove_at(tag: &mut Nbt, index: usize) {
    match tag {
        Nbt::List(items) => drop(items.remove(index)),
        Nbt::ByteArray(v) => drop(v.remove(index)),
        Nbt::IntArray(v) => drop(v.remove(index)),
        Nbt::LongArray(v) => drop(v.remove(index)),
        _ => {}
    }
}

fn clear(tag: &mut Nbt) {
    match tag {
        Nbt::List(items) => items.clear(),
        Nbt::ByteArray(v) => v.clear(),
        Nbt::IntArray(v) => v.clear(),
        Nbt::LongArray(v) => v.clear(),
        _ => {}
    }
}

fn elements(tag: &Nbt, steps: &[Step]) -> Vec<Found> {
    match tag {
        Nbt::List(items) => (0..items.len()).map(|i| child(steps, Step::Index(i))).collect(),
        _ => (0..collection_len(tag).unwrap_or(0)).map(|i| Found::Value(element(tag, i))).collect(),
    }
}

fn found_element(parent: &Nbt, steps: &[Step], i: usize) -> Found {
    match parent {
        Nbt::List(_) => child(steps, Step::Index(i)),
        _ => Found::Value(element(parent, i)),
    }
}

fn actual_index(index: i32, size: usize) -> Option<usize> {
    let actual = if index < 0 { size as i64 + index as i64 } else { index as i64 };
    (0..size as i64).contains(&actual).then_some(actual as usize)
}

impl Node {
    fn get_tag(&self, parent: &Nbt, steps: &[Step], out: &mut Vec<Found>) {
        match (self, parent) {
            (Node::AllElements, _) => out.extend(elements(parent, steps)),
            (Node::CompoundChild(name), Nbt::Compound(fields)) => {
                if compound_get(fields, name).is_some() {
                    out.push(child(steps, Step::Key(name.clone())));
                }
            }
            (Node::IndexedElement(index), _) => {
                if let Some(i) = collection_len(parent).and_then(|size| actual_index(*index, size)) {
                    out.push(found_element(parent, steps, i));
                }
            }
            (Node::MatchElement(pattern), Nbt::List(items)) => {
                out.extend((0..items.len()).filter(|&i| compare_nbt(pattern, Some(&items[i]))).map(|i| child(steps, Step::Index(i))));
            }
            (Node::MatchObject(name, pattern), Nbt::Compound(fields)) => {
                if compare_nbt(pattern, compound_get(fields, name)) {
                    out.push(child(steps, Step::Key(name.clone())));
                }
            }
            (Node::MatchRootObject(pattern), Nbt::Compound(_)) => {
                if compare_nbt(pattern, Some(parent)) {
                    out.push(Found::At(steps.to_vec()));
                }
            }
            _ => {}
        }
    }

    fn get_or_create_tag(&self, root: &mut Nbt, steps: &[Step], new_child: &dyn Fn() -> Nbt, out: &mut Vec<Found>) {
        let parent = at_mut(root, steps);
        match self {
            Node::AllElements => match collection_len(parent) {
                Some(0) => {
                    if add_tag(parent, 0, new_child()) {
                        out.push(found_element(parent, steps, 0));
                    }
                }
                Some(_) => out.extend(elements(parent, steps)),
                None => {}
            },
            Node::CompoundChild(name) => {
                if let Nbt::Compound(fields) = parent {
                    if compound_get(fields, name).is_none() {
                        fields.push((name.clone(), new_child()));
                    }
                    out.push(child(steps, Step::Key(name.clone())));
                }
            }
            Node::MatchElement(pattern) => {
                if let Nbt::List(items) = parent {
                    let before = out.len();
                    out.extend((0..items.len()).filter(|&i| compare_nbt(pattern, Some(&items[i]))).map(|i| child(steps, Step::Index(i))));
                    if out.len() == before {
                        items.push(pattern.clone());
                        out.push(child(steps, Step::Index(items.len() - 1)));
                    }
                }
            }
            Node::MatchObject(name, pattern) => {
                if let Nbt::Compound(fields) = parent {
                    match compound_get(fields, name) {
                        None => {
                            fields.push((name.clone(), pattern.clone()));
                            out.push(child(steps, Step::Key(name.clone())));
                        }
                        Some(result) if compare_nbt(pattern, Some(result)) => out.push(child(steps, Step::Key(name.clone()))),
                        Some(_) => {}
                    }
                }
            }
            Node::IndexedElement(_) | Node::MatchRootObject(_) => self.get_tag(parent, steps, out),
        }
    }

    fn preferred_parent(&self) -> Nbt {
        match self {
            Node::AllElements | Node::IndexedElement(_) | Node::MatchElement(_) => Nbt::List(Vec::new()),
            Node::CompoundChild(_) | Node::MatchObject(..) | Node::MatchRootObject(_) => Nbt::Compound(Vec::new()),
        }
    }

    fn set_tag(&self, parent: &mut Nbt, value: &Nbt) -> i32 {
        match self {
            Node::AllElements => {
                let Some(size) = collection_len(parent) else { return 0 };
                if size == 0 {
                    add_tag(parent, 0, value.clone());
                    return 1;
                }
                let changed = size - (0..size).filter(|&i| element(parent, i) == *value).count();
                if changed == 0 {
                    return 0;
                }
                clear(parent);
                if !add_tag(parent, 0, value.clone()) {
                    return 0;
                }
                for i in 1..size {
                    add_tag(parent, i, value.clone());
                }
                changed as i32
            }
            Node::CompoundChild(name) => match parent {
                Nbt::Compound(fields) => (compound_put(fields, name, value.clone()).as_ref() != Some(value)) as i32,
                _ => 0,
            },
            Node::IndexedElement(index) => {
                let Some(i) = collection_len(parent).and_then(|size| actual_index(*index, size)) else { return 0 };
                (element(parent, i) != *value && set_tag(parent, i, value.clone())) as i32
            }
            Node::MatchElement(pattern) => {
                let Nbt::List(items) = parent else { return 0 };
                if items.is_empty() {
                    items.push(value.clone());
                    return 1;
                }
                let mut changed = 0;
                for current in items.iter_mut() {
                    if compare_nbt(pattern, Some(current)) && current != value {
                        *current = value.clone();
                        changed += 1;
                    }
                }
                changed
            }
            Node::MatchObject(name, pattern) => {
                let Nbt::Compound(fields) = parent else { return 0 };
                let current = compound_get(fields, name);
                if compare_nbt(pattern, current) && current != Some(value) {
                    compound_put(fields, name, value.clone());
                    1
                } else {
                    0
                }
            }
            Node::MatchRootObject(_) => 0,
        }
    }

    fn remove_tag(&self, parent: &mut Nbt) -> i32 {
        match self {
            Node::AllElements => match collection_len(parent) {
                Some(size) if size > 0 => {
                    clear(parent);
                    size as i32
                }
                _ => 0,
            },
            Node::CompoundChild(name) => match parent {
                Nbt::Compound(fields) => {
                    let before = fields.len();
                    fields.retain(|(k, _)| k != name);
                    (fields.len() != before) as i32
                }
                _ => 0,
            },
            Node::IndexedElement(index) => {
                let Some(i) = collection_len(parent).and_then(|size| actual_index(*index, size)) else { return 0 };
                remove_at(parent, i);
                1
            }
            Node::MatchElement(pattern) => {
                let Nbt::List(items) = parent else { return 0 };
                let mut changed = 0;
                for i in (0..items.len()).rev() {
                    if compare_nbt(pattern, Some(&items[i])) {
                        items.remove(i);
                        changed += 1;
                    }
                }
                changed
            }
            Node::MatchObject(name, pattern) => {
                let Nbt::Compound(fields) = parent else { return 0 };
                if compare_nbt(pattern, compound_get(fields, name)) {
                    fields.retain(|(k, _)| k != name);
                    1
                } else {
                    0
                }
            }
            Node::MatchRootObject(_) => 0,
        }
    }
}

fn get_all(node: &Node, root: &Nbt, found: &[Found]) -> Vec<Found> {
    let mut out = Vec::new();
    for f in found {
        if let Found::At(steps) = f {
            node.get_tag(at(root, steps), steps, &mut out);
        }
    }
    out
}

fn resolve(root: &Nbt, found: Vec<Found>) -> Vec<Nbt> {
    found
        .into_iter()
        .map(|f| match f {
            Found::At(steps) => at(root, &steps).clone(),
            Found::Value(value) => value,
        })
        .collect()
}

pub fn is_too_deep(tag: &Nbt, depth: usize) -> bool {
    if depth >= 512 {
        return true;
    }
    match tag {
        Nbt::Compound(fields) => fields.iter().any(|(_, v)| is_too_deep(v, depth + 1)),
        Nbt::List(items) => items.iter().any(|v| is_too_deep(v, depth + 1)),
        _ => false,
    }
}

impl NbtPath {
    pub fn decode(tag: &Nbt) -> Result<NbtPath, String> {
        let string = crate::util::codec::string(tag)?;
        let units: Vec<u16> = string.encode_utf16().collect();
        parse(&mut StringReader::new(&units)).map_err(|e| format!("Failed to parse path {string}: {}", e.get_message()))
    }

    fn not_found(&self, node: usize) -> CommandSyntaxException {
        let text: Vec<u16> = self.original.encode_utf16().take(self.nodes[node].1).collect();
        ERROR_NOTHING_FOUND.create(String::from_utf16_lossy(&text))
    }

    pub fn get(&self, tag: &Nbt) -> Result<Vec<Nbt>, CommandSyntaxException> {
        let mut found = vec![Found::At(Vec::new())];
        for (i, (node, _)) in self.nodes.iter().enumerate() {
            found = get_all(node, tag, &found);
            if found.is_empty() {
                return Err(self.not_found(i));
            }
        }
        Ok(resolve(tag, found))
    }

    pub fn count_matching(&self, tag: &Nbt) -> i32 {
        let mut found = vec![Found::At(Vec::new())];
        for (node, _) in &self.nodes {
            found = get_all(node, tag, &found);
            if found.is_empty() {
                return 0;
            }
        }
        found.len() as i32
    }

    fn get_or_create_parents(&self, root: &mut Nbt) -> Result<Vec<Found>, CommandSyntaxException> {
        let mut found = vec![Found::At(Vec::new())];
        for i in 0..self.nodes.len() - 1 {
            let next = &self.nodes[i + 1].0;
            let mut out = Vec::new();
            for f in &found {
                if let Found::At(steps) = f {
                    self.nodes[i].0.get_or_create_tag(root, steps, &|| next.preferred_parent(), &mut out);
                }
            }
            found = out;
            if found.is_empty() {
                return Err(self.not_found(i));
            }
        }
        Ok(found)
    }

    fn get_or_create(&self, root: &mut Nbt, new_value: &dyn Fn() -> Nbt) -> Result<Vec<Found>, CommandSyntaxException> {
        let parents = self.get_or_create_parents(root)?;
        let last = &self.nodes[self.nodes.len() - 1].0;
        let mut out = Vec::new();
        for f in &parents {
            if let Found::At(steps) = f {
                last.get_or_create_tag(root, steps, new_value, &mut out);
            }
        }
        Ok(out)
    }

    pub fn get_or_create_each(
        &self,
        root: &mut Nbt,
        new_value: &dyn Fn() -> Nbt,
        mut f: impl FnMut(&mut Nbt) -> Result<i32, CommandSyntaxException>,
    ) -> Result<i32, CommandSyntaxException> {
        let mut total = 0;
        for target in self.get_or_create(root, new_value)? {
            total += match target {
                Found::At(steps) => f(at_mut(root, &steps))?,
                Found::Value(mut value) => f(&mut value)?,
            };
        }
        Ok(total)
    }

    pub fn set(&self, root: &mut Nbt, to_add: &Nbt) -> Result<i32, CommandSyntaxException> {
        if is_too_deep(to_add, self.nodes.len()) {
            return Err(ERROR_DATA_TOO_DEEP.create());
        }
        let parents = self.get_or_create_parents(root)?;
        let last = &self.nodes[self.nodes.len() - 1].0;
        Ok(parents
            .iter()
            .map(|f| match f {
                Found::At(steps) => last.set_tag(at_mut(root, steps), to_add),
                Found::Value(_) => 0,
            })
            .sum())
    }

    pub fn insert(&self, index: i32, root: &mut Nbt, to_insert: &[Nbt]) -> Result<i32, CommandSyntaxException> {
        if to_insert.iter().any(|t| is_too_deep(t, self.nodes.len())) {
            return Err(ERROR_DATA_TOO_DEEP.create());
        }
        let targets = self.get_or_create(root, &|| Nbt::List(Vec::new()))?;
        let mut modified_count = 0;
        for target in targets {
            let tag = match &target {
                Found::At(steps) => at_mut(root, steps),
                Found::Value(value) => return Err(ERROR_EXPECTED_LIST.create(nbt::to_string(value))),
            };
            let Some(size) = collection_len(tag) else { return Err(ERROR_EXPECTED_LIST.create(nbt::to_string(tag))) };
            let mut actual = if index < 0 { size as i64 + index as i64 + 1 } else { index as i64 };
            let mut modified = false;
            for source in to_insert {
                let accepts = matches!(tag, Nbt::List(_)) || numeric(source).is_some();
                if accepts && (actual < 0 || actual > collection_len(tag).unwrap() as i64) {
                    return Err(ERROR_INVALID_INDEX.create(actual as i32));
                }
                if add_tag(tag, actual as usize, source.clone()) {
                    actual += 1;
                    modified = true;
                }
            }
            modified_count += modified as i32;
        }
        Ok(modified_count)
    }

    pub fn remove(&self, root: &mut Nbt) -> i32 {
        let mut found = vec![Found::At(Vec::new())];
        for (node, _) in &self.nodes[..self.nodes.len() - 1] {
            found = get_all(node, root, &found);
        }
        let last = &self.nodes[self.nodes.len() - 1].0;
        found
            .iter()
            .map(|f| match f {
                Found::At(steps) => last.remove_tag(at_mut(root, steps)),
                Found::Value(_) => 0,
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::tests::error;

    fn run(case: &str) -> String {
        let f: Vec<&str> = case.split('\t').collect();
        let mut root = nbt::parse_fully(f[1]).unwrap();
        let units: Vec<u16> = f[2].encode_utf16().collect();
        let path = parse(&mut StringReader::new(&units)).unwrap();
        let list = |tags: Vec<Nbt>| format!("[{}]", tags.iter().map(nbt::to_string).collect::<Vec<_>>().join(", "));
        let result = match f[0] {
            "get" => path.get(&root).map(|tags| format!("GET {}", list(tags))),
            "count" => Ok(format!("COUNT {}", path.count_matching(&root))),
            "set" => path.set(&mut root, &nbt::parse_fully(f[3]).unwrap()).map(|n| format!("SET {n} {}", nbt::to_string(&root))),
            "insert" => {
                let (index, values) = f[3].split_once(' ').unwrap();
                let Nbt::List(values) = nbt::parse_fully(values).unwrap() else { panic!("not a list") };
                path.insert(index.parse().unwrap(), &mut root, &values).map(|n| format!("INS {n} {}", nbt::to_string(&root)))
            }
            "remove" => Ok(format!("REM {} {}", path.remove(&mut root), nbt::to_string(&root))),
            op => panic!("unknown op {op}"),
        };
        result.unwrap_or_else(|e| error(&e))
    }

    #[test]
    fn operations_match_vanilla() {
        let failures: Vec<String> = CASES
            .iter()
            .filter_map(|(case, expected)| {
                let actual = run(case);
                (actual != *expected).then(|| format!("{case:?}\n  expected {expected}\n  actual   {actual}"))
            })
            .collect();
        assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), CASES.len(), failures.join("\n"));
    }

    #[test]
    fn too_deep() {
        let mut deep = Nbt::Int(0);
        for _ in 0..511 {
            deep = Nbt::List(vec![deep]);
        }
        let mut root = Nbt::Compound(Vec::new());
        let units: Vec<u16> = "a".encode_utf16().collect();
        let path = parse(&mut StringReader::new(&units)).unwrap();
        assert_eq!(path.set(&mut root, &deep).map_err(|e| error(&e)), Err("ERR Resulting NBT too deeply nested".to_string()));
    }

    const CASES: &[(&str, &str)] = &[
        ("get\t{a:1}\ta", "GET [1]"),
        ("get\t{a:1}\tb", "ERR Found no elements matching b"),
        ("get\t{a:{b:[1,2,3]}}\ta.b[]", "GET [1, 2, 3]"),
        ("get\t{a:{b:[1,2,3]}}\ta.b[-1]", "GET [3]"),
        ("get\t{a:{b:[1,2,3]}}\ta.b[3]", "ERR Found no elements matching a.b[3]"),
        ("get\t{a:{b:[1,2,3]}}\ta.c.d", "ERR Found no elements matching a.c"),
        ("get\t{a:[{x:1,y:2},{x:2},{x:1}]}\ta[{x:1}]", "GET [{x:1,y:2}, {x:1}]"),
        ("get\t{a:[{x:1,y:2},{x:2},{x:1}]}\ta[{x:1}].y", "GET [2]"),
        ("get\t{a:{x:1,y:[1,2]}}\ta{y:[2]}", "GET [{x:1,y:[1,2]}]"),
        ("get\t{a:{x:1,y:[1,2]}}\ta{y:[]}", "ERR Found no elements matching a{y:[]}"),
        ("get\t{a:{x:1}}\t{a:{x:1}}.a.x", "GET [1]"),
        ("get\t{a:[I;1,2,3]}\ta[]", "GET [1, 2, 3]"),
        ("get\t{a:[I;1,2,3]}\ta[1]", "GET [2]"),
        ("get\t{a:[B;1b,2b]}\ta[-2]", "GET [1b]"),
        ("get\t{a:[L;5L]}\ta[0].b", "ERR Found no elements matching a[0].b"),
        ("get\t{\"é\":{\"x y\":1}}\t\"é\".\"x y\"", "GET [1]"),
        ("get\t{a:[[1],[2,3]]}\ta[][]", "GET [1, 2, 3]"),
        ("get\t{a:[1,\"s\",{c:1}]}\ta[]", "GET [1, \"s\", {c:1}]"),
        ("get\t{a:[1,\"s\",{c:1}]}\ta[{c:1}]", "GET [{c:1}]"),
        ("count\t{a:[{x:1},{x:1},{x:2}]}\ta[{x:1}]", "COUNT 2"),
        ("count\t{a:[{x:1},{x:1},{x:2}]}\ta[].x", "COUNT 3"),
        ("count\t{a:[{x:1},{x:1},{x:2}]}\ta[].z", "COUNT 0"),
        ("count\t{a:[I;1,2]}\ta[]", "COUNT 2"),
        ("set\t{z:0}\ta.b.c\t1", "SET 1 {a:{b:{c:1}},z:0}"),
        ("set\t{z:0}\ta[0]\t1", "SET 0 {a:[],z:0}"),
        ("set\t{z:0}\ta[]\t1", "SET 1 {a:[1],z:0}"),
        ("set\t{z:0}\ta[].b\t1", "SET 1 {a:[{b:1}],z:0}"),
        ("set\t{z:0}\ta[{x:1}].y\t2", "SET 1 {a:[{x:1,y:2}],z:0}"),
        ("set\t{z:0}\ta{x:1}.y\t2", "SET 1 {a:{x:1,y:2},z:0}"),
        ("set\t{a:{x:2}}\ta{x:1}.y\t2", "ERR Found no elements matching a{x:1}"),
        ("set\t{a:5}\ta\t5", "SET 0 {a:5}"),
        ("set\t{a:5}\ta\t6", "SET 1 {a:6}"),
        ("set\t{a:[1,1,2]}\ta[]\t1", "SET 1 {a:[1,1,1]}"),
        ("set\t{a:[1,1,2]}\ta[]\t3", "SET 3 {a:[3,3,3]}"),
        ("set\t{a:[]}\ta[]\t3", "SET 1 {a:[3]}"),
        ("set\t{a:[1,2]}\ta[5]\t9", "SET 0 {a:[1,2]}"),
        ("set\t{a:[1,2]}\ta[-1]\t9", "SET 1 {a:[1,9]}"),
        ("set\t{a:[1,2]}\ta[-1]\t2", "SET 0 {a:[1,2]}"),
        ("set\t{a:[{x:1},{x:2},{x:1}]}\ta[{x:1}]\t{x:1,z:1}", "SET 2 {a:[{x:1,z:1},{x:2},{x:1,z:1}]}"),
        ("set\t{a:[]}\ta[{x:1}]\t{y:1}", "SET 1 {a:[{y:1}]}"),
        ("set\t{a:[I;1,2,3]}\ta[]\t7", "SET 3 {a:[I;7,7,7]}"),
        ("set\t{a:[I;1,2,3]}\ta[]\t7.9f", "SET 3 {a:[I;7,7,7]}"),
        ("set\t{a:[I;1,2,3]}\ta[]\t\"s\"", "SET 0 {a:[I;]}"),
        ("set\t{a:[I;1,2,3]}\ta[1]\t-2.5d", "SET 1 {a:[I;1,-3,3]}"),
        ("set\t{a:[B;1b,2b]}\ta[0]\t300", "SET 1 {a:[B;44B,2B]}"),
        ("set\t{a:[L;1L]}\ta[0]\t-1.5f", "SET 1 {a:[L;-1L]}"),
        ("set\t{a:[L;1L]}\ta[0]\t-1.5d", "SET 1 {a:[L;-2L]}"),
        ("set\t{a:{x:1}}\t{a:{x:1}}\t{}", "SET 0 {a:{x:1}}"),
        ("set\t{a:{x:1}}\t{a:{x:2}}.b\t1", "ERR Found no elements matching {a:{x:2}}"),
        ("set\t{a:1}\ta.b\t1", "SET 0 {a:1}"),
        ("set\t{a:[1,2]}\ta.b\t1", "SET 0 {a:[1,2]}"),
        ("set\t{a:[I;1]}\ta[0].b\t1", "SET 0 {a:[I;1]}"),
        ("set\t{a:{b:1}}\ta{b:1}\t{b:1}", "SET 0 {a:{b:1}}"),
        ("set\t{a:{b:1}}\ta{b:1}\t{b:2}", "SET 1 {a:{b:2}}"),
        ("set\t{a:[1,\"s\"]}\ta[0]\t{c:1}", "SET 1 {a:[{c:1},\"s\"]}"),
        ("set\t{z:0}\ta\t{x:[[1],[2]]}", "SET 1 {a:{x:[[1],[2]]},z:0}"),
        ("insert\t{z:0}\ta\t0 [1,2]", "INS 1 {a:[1,2],z:0}"),
        ("insert\t{a:[1,2]}\ta\t-1 [9]", "INS 1 {a:[1,2,9]}"),
        ("insert\t{a:[1,2]}\ta\t1 [8,9]", "INS 1 {a:[1,8,9,2]}"),
        ("insert\t{a:[1,2]}\ta\t3 [9]", "ERR Invalid list index: 3"),
        ("insert\t{a:[1,2]}\ta\t-4 [9]", "ERR Invalid list index: -1"),
        ("insert\t{a:[1,2]}\ta\t0 [\"x\",{y:1}]", "INS 1 {a:[\"x\",{y:1},1,2]}"),
        ("insert\t{a:[I;1,2]}\ta\t0 [3,4.5f]", "INS 1 {a:[I;3,4,1,2]}"),
        ("insert\t{a:[I;1,2]}\ta\t0 [\"s\"]", "INS 0 {a:[I;1,2]}"),
        ("insert\t{a:[I;1,2]}\ta\t9 [\"s\"]", "INS 0 {a:[I;1,2]}"),
        ("insert\t{a:[I;1,2]}\ta\t9 [1]", "ERR Invalid list index: 9"),
        ("insert\t{a:5}\ta\t0 [1]", "ERR Expected a list: got 5"),
        ("insert\t{a:[[1],[2]]}\ta[]\t0 [0]", "INS 2 {a:[[0,1],[0,2]]}"),
        ("insert\t{a:[I;1]}\ta[]\t0 [0]", "ERR Expected a list: got 1"),
        ("insert\t{a:[{x:1},{x:1}]}\ta[].l\t-1 [1]", "INS 2 {a:[{l:[1],x:1},{l:[1],x:1}]}"),
        ("remove\t{a:1,b:2}\ta", "REM 1 {b:2}"),
        ("remove\t{a:1,b:2}\tc", "REM 0 {a:1,b:2}"),
        ("remove\t{a:[1,2,3]}\ta[]", "REM 3 {a:[]}"),
        ("remove\t{a:[1,2,3]}\ta[-1]", "REM 1 {a:[1,2]}"),
        ("remove\t{a:[1,2,3]}\ta[3]", "REM 0 {a:[1,2,3]}"),
        ("remove\t{a:[I;1,2,3]}\ta[0]", "REM 1 {a:[I;2,3]}"),
        ("remove\t{a:[I;1,2,3]}\ta[]", "REM 3 {a:[I;]}"),
        ("remove\t{a:[{x:1},{x:2},{x:1,y:1}]}\ta[{x:1}]", "REM 2 {a:[{x:2}]}"),
        ("remove\t{a:[{x:1},{x:2},{x:1,y:1}]}\ta[].x", "REM 3 {a:[{},{},{y:1}]}"),
        ("remove\t{a:{x:1}}\ta{x:1}", "REM 1 {}"),
        ("remove\t{a:{x:1}}\ta{x:2}", "REM 0 {a:{x:1}}"),
        ("remove\t{a:{x:1}}\t{a:{}}", "REM 0 {a:{x:1}}"),
        ("remove\t{a:{x:1}}\tq.r.s", "REM 0 {a:{x:1}}"),
    ];
}
