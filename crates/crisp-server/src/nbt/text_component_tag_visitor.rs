use crisp_protocol::nbt::{Nbt, java_hash_map_order};

use super::{java_double_to_string, java_float_to_string, quote_and_escape};
use crate::network::chat::{ChatFormatting, Component};

const INLINE_LIST_THRESHOLD: usize = 8;
const MAX_DEPTH: usize = 64;
const MAX_LENGTH: usize = 128;

const KEY: ChatFormatting = ChatFormatting::Aqua;
const STRING: ChatFormatting = ChatFormatting::Green;
const NUMBER: ChatFormatting = ChatFormatting::Gold;
const NUMBER_TYPE: ChatFormatting = ChatFormatting::Red;

pub fn to_pretty_component(tag: &Nbt) -> Component {
    let mut visitor = Visitor { result: Vec::new(), depth: 0 };
    visitor.visit(tag);
    visitor.result.into_iter().fold(Component::empty(), Component::append)
}

struct Visitor {
    result: Vec<Component>,
    depth: usize,
}

impl Visitor {
    fn text(&mut self, text: &str) {
        self.result.push(Component::literal(text));
    }

    fn styled(&mut self, text: &str, color: ChatFormatting) {
        self.result.push(Component::literal(text).with_color(color));
    }

    fn number(&mut self, value: String, suffix: Option<&str>) {
        self.styled(&value, NUMBER);
        if let Some(suffix) = suffix {
            self.styled(suffix, NUMBER_TYPE);
        }
    }

    fn folded(&mut self) {
        self.styled("<...>", ChatFormatting::Gray);
    }

    fn visit(&mut self, tag: &Nbt) {
        match tag {
            Nbt::String(value) => {
                let mut quoted = String::new();
                quote_and_escape(value, &mut quoted);
                let quote = &quoted[..1];
                self.text(quote);
                self.styled(&quoted[1..quoted.len() - 1], STRING);
                self.text(quote);
            }
            Nbt::Byte(v) => self.number(v.to_string(), Some("b")),
            Nbt::Short(v) => self.number(v.to_string(), Some("s")),
            Nbt::Int(v) => self.number(v.to_string(), None),
            Nbt::Long(v) => self.number(v.to_string(), Some("L")),
            Nbt::Float(v) => self.number(java_float_to_string(crisp_protocol::nbt::float_tag_value(*v)), Some("f")),
            Nbt::Double(v) => self.number(java_double_to_string(crisp_protocol::nbt::double_tag_value(*v)), Some("d")),
            Nbt::ByteArray(data) => self.array("B", data.iter().map(|v| v.to_string()).collect(), Some("b")),
            Nbt::IntArray(data) => self.array("I", data.iter().map(|v| v.to_string()).collect(), None),
            Nbt::LongArray(data) => self.array("L", data.iter().map(|v| v.to_string()).collect(), Some("L")),
            Nbt::List(items) => self.list(items),
            Nbt::Compound(fields) => self.compound(fields),
        }
    }

    fn array(&mut self, prefix: &str, data: Vec<String>, suffix: Option<&str>) {
        self.text("[");
        self.styled(prefix, NUMBER_TYPE);
        self.text(";");
        for (i, value) in data.iter().enumerate().take(MAX_LENGTH) {
            self.text(" ");
            self.number(value.clone(), suffix);
            if i != data.len() - 1 {
                self.text(",");
            }
        }
        if data.len() > MAX_LENGTH {
            self.folded();
        }
        self.text("]");
    }

    fn should_wrap(items: &[Nbt]) -> bool {
        items.len() < INLINE_LIST_THRESHOLD && items.iter().any(|tag| !matches!(tag, Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_)))
    }

    fn list(&mut self, items: &[Nbt]) {
        if items.is_empty() {
            self.text("[");
            self.text("]");
        } else if self.depth >= MAX_DEPTH {
            self.text("[");
            self.folded();
            self.text("]");
        } else if !Self::should_wrap(items) {
            self.text("[");
            for (i, item) in items.iter().enumerate() {
                if i != 0 {
                    self.text(",");
                    self.text(" ");
                }
                self.sub_tag(item);
            }
            self.text("]");
        } else {
            self.text("[");
            for (i, item) in items.iter().enumerate().take(MAX_LENGTH) {
                self.text("");
                self.sub_tag(item);
                if i != items.len() - 1 {
                    self.text(",");
                    self.text(" ");
                }
            }
            if items.len() > MAX_LENGTH {
                self.text("");
                self.folded();
            }
            self.text("]");
        }
    }

    fn compound(&mut self, fields: &[(String, Nbt)]) {
        if fields.is_empty() {
            self.text("{");
            self.text("}");
        } else if self.depth >= MAX_DEPTH {
            self.text("{");
            self.folded();
            self.text("}");
        } else {
            self.text("{");
            let order = java_hash_map_order(&fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>());
            for (n, &i) in order.iter().enumerate() {
                let (key, value) = &fields[i];
                self.text("");
                self.key(key);
                self.text(":");
                self.text(" ");
                self.sub_tag(value);
                if n != order.len() - 1 {
                    self.text(",");
                    self.text(" ");
                }
            }
            self.text("}");
        }
    }

    fn sub_tag(&mut self, tag: &Nbt) {
        self.depth += 1;
        self.visit(tag);
        self.depth -= 1;
    }

    fn key(&mut self, key: &str) {
        if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-')) {
            self.result.push(Component::literal(key).with_color(KEY));
            return;
        }
        let mut quoted = String::new();
        quote_and_escape(key, &mut quoted);
        let quote = &quoted[..1];
        let inner = Component::literal(&quoted[1..quoted.len() - 1]).with_color(KEY);
        self.result.push(Component::literal(quote).append(inner).append(Component::literal(quote)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt;

    fn run(case: &str) -> String {
        let f: Vec<&str> = case.split('\t').collect();
        match f[0] {
            "pretty" => format!("PRETTY {}", to_pretty_component(&nbt::parse_fully(f[1]).unwrap()).to_json()),
            "merge" => {
                let (Nbt::Compound(mut root), Nbt::Compound(other)) = (nbt::parse_fully(f[1]).unwrap(), nbt::parse_fully(f[2]).unwrap()) else { panic!("not compounds") };
                nbt::merge(&mut root, &other);
                format!("MERGE {}", to_pretty_component(&Nbt::Compound(root)).to_json())
            }
            op => panic!("unknown op {op}"),
        }
    }

    fn json(line: &str) -> (&str, serde_json::Value) {
        let (tag, json) = line.split_once(' ').unwrap();
        (tag, serde_json::from_str(json).unwrap())
    }

    #[test]
    fn matches_vanilla() {
        let failures: Vec<String> = CASES
            .iter()
            .filter_map(|(case, expected)| {
                let actual = run(case);
                (json(&actual) != json(expected)).then(|| format!("{case:?}\n  expected {expected}\n  actual   {actual}"))
            })
            .collect();
        assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), CASES.len(), failures.join("\n"));
    }

    const CASES: &[(&str, &str)] = &[
        ("pretty\t{a:1,b:2b,c:3s,d:4L,e:5.5f,f:6.25d,g:\"str\"}", "PRETTY {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"a\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"b\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"2\",\"color\":\"gold\"},{\"text\":\"b\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"c\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},{\"text\":\"s\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"d\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"4\",\"color\":\"gold\"},{\"text\":\"L\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"e\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"5.5\",\"color\":\"gold\"},{\"text\":\"f\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"f\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"6.25\",\"color\":\"gold\"},{\"text\":\"d\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"g\",\"color\":\"aqua\"},\":\",\" \",\"\\\"\",{\"text\":\"str\",\"color\":\"green\"},\"\\\"\",\"}\"]}"),
        ("pretty\t{\"key with space\":1,\"q'uote\":\"it's\",\"dq\\\"\":'say \"hi\"',\"é\":1}", "PRETTY {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"\\\"\",\"extra\":[{\"text\":\"q'uote\",\"color\":\"aqua\"},\"\\\"\"]},\":\",\" \",\"\\\"\",{\"text\":\"it's\",\"color\":\"green\"},\"\\\"\",\",\",\" \",\"\",{\"text\":\"'\",\"extra\":[{\"text\":\"dq\\\"\",\"color\":\"aqua\"},\"'\"]},\":\",\" \",\"'\",{\"text\":\"say \\\"hi\\\"\",\"color\":\"green\"},\"'\",\",\",\" \",\"\",{\"text\":\"\\\"\",\"extra\":[{\"text\":\"key with space\",\"color\":\"aqua\"},\"\\\"\"]},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"\\\"\",\"extra\":[{\"text\":\"é\",\"color\":\"aqua\"},\"\\\"\"]},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\"}\"]}"),
        ("pretty\t[1,2,3]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\"]\"]}"),
        ("pretty\t[1,2,3,4,5,6,7,8,9]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"4\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"5\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"6\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"7\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"8\",\"color\":\"gold\"},\",\",\" \",{\"text\":\"9\",\"color\":\"gold\"},\"]\"]}"),
        ("pretty\t[\"a\",\"b\"]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",\"\",\"\\\"\",{\"text\":\"a\",\"color\":\"green\"},\"\\\"\",\",\",\" \",\"\",\"\\\"\",{\"text\":\"b\",\"color\":\"green\"},\"\\\"\",\"]\"]}"),
        ("pretty\t[{a:1},{b:[]}]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",\"\",\"{\",\"\",{\"text\":\"a\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\"}\",\",\",\" \",\"\",\"{\",\"\",{\"text\":\"b\",\"color\":\"aqua\"},\":\",\" \",\"[\",\"]\",\"}\",\"]\"]}"),
        ("pretty\t[[1],[2]]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",\"\",\"[\",{\"text\":\"1\",\"color\":\"gold\"},\"]\",\",\",\" \",\"\",\"[\",{\"text\":\"2\",\"color\":\"gold\"},\"]\",\"]\"]}"),
        ("pretty\t[1,\"s\",{c:1}]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",\"\",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",\"\\\"\",{\"text\":\"s\",\"color\":\"green\"},\"\\\"\",\",\",\" \",\"\",\"{\",\"\",{\"text\":\"c\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\"}\",\"]\"]}"),
        ("pretty\t[B;1b,2b,-3b]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",{\"text\":\"B\",\"color\":\"red\"},\";\",\" \",{\"text\":\"1\",\"color\":\"gold\"},{\"text\":\"b\",\"color\":\"red\"},\",\",\" \",{\"text\":\"2\",\"color\":\"gold\"},{\"text\":\"b\",\"color\":\"red\"},\",\",\" \",{\"text\":\"-3\",\"color\":\"gold\"},{\"text\":\"b\",\"color\":\"red\"},\"]\"]}"),
        ("pretty\t[I;]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",{\"text\":\"I\",\"color\":\"red\"},\";\",\"]\"]}"),
        ("pretty\t[L;1L,2L]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",{\"text\":\"L\",\"color\":\"red\"},\";\",\" \",{\"text\":\"1\",\"color\":\"gold\"},{\"text\":\"L\",\"color\":\"red\"},\",\",\" \",{\"text\":\"2\",\"color\":\"gold\"},{\"text\":\"L\",\"color\":\"red\"},\"]\"]}"),
        ("pretty\t{}", "PRETTY {\"text\":\"\",\"extra\":[\"{\",\"}\"]}"),
        ("pretty\t[]", "PRETTY {\"text\":\"\",\"extra\":[\"[\",\"]\"]}"),
        ("pretty\t\"plain\"", "PRETTY {\"text\":\"\",\"extra\":[\"\\\"\",{\"text\":\"plain\",\"color\":\"green\"},\"\\\"\"]}"),
        ("pretty\t1.0E10d", "PRETTY {\"text\":\"\",\"extra\":[{\"text\":\"1.0E10\",\"color\":\"gold\"},{\"text\":\"d\",\"color\":\"red\"}]}"),
        ("pretty\t1.0E-5f", "PRETTY {\"text\":\"\",\"extra\":[{\"text\":\"1.0E-5\",\"color\":\"gold\"},{\"text\":\"f\",\"color\":\"red\"}]}"),
        ("pretty\t{k0:0,k1:1,k2:2,k3:3,k4:4,k5:5,k6:6,k7:7,k8:8,k9:9,k10:10,k11:11,k12:12,k13:13,k14:14,alpha:1,beta:2,gamma:3}", "PRETTY {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"k0\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"0\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k1\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k2\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k3\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k4\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"4\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k5\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"5\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k11\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"11\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k6\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"6\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k10\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"10\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k7\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"7\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k13\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"13\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k8\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"8\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k12\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"12\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k9\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"9\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"k14\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"14\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"alpha\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"beta\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"gamma\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\"}\"]}"),
        ("pretty\t{zebra:1,apple:2,mango:3,kiwi:4,banana:5,cherry:6,date:7,fig:8,grape:9,lemon:10,lime:11,melon:12,nut:13}", "PRETTY {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"zebra\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"banana\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"5\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"date\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"7\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"cherry\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"6\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"kiwi\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"4\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"fig\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"8\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"lime\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"11\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"grape\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"9\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"nut\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"13\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"mango\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"apple\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"lemon\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"10\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"melon\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"12\",\"color\":\"gold\"},\"}\"]}"),
        ("pretty\t{list:[{x:1,y:2},{x:3}],nested:{deep:{deeper:{deepest:\"bottom\"}}},true:1b,False:0b}", "PRETTY {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"true\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},{\"text\":\"b\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"False\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"0\",\"color\":\"gold\"},{\"text\":\"b\",\"color\":\"red\"},\",\",\" \",\"\",{\"text\":\"list\",\"color\":\"aqua\"},\":\",\" \",\"[\",\"\",\"{\",\"\",{\"text\":\"x\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"y\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\"}\",\",\",\" \",\"\",\"{\",\"\",{\"text\":\"x\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\"}\",\"]\",\",\",\" \",\"\",{\"text\":\"nested\",\"color\":\"aqua\"},\":\",\" \",\"{\",\"\",{\"text\":\"deep\",\"color\":\"aqua\"},\":\",\" \",\"{\",\"\",{\"text\":\"deeper\",\"color\":\"aqua\"},\":\",\" \",\"{\",\"\",{\"text\":\"deepest\",\"color\":\"aqua\"},\":\",\" \",\"\\\"\",{\"text\":\"bottom\",\"color\":\"green\"},\"\\\"\",\"}\",\"}\",\"}\",\"}\"]}"),
        ("pretty\t\"tab\\there\\nnewline\\back\"", "PRETTY {\"text\":\"\",\"extra\":[\"\\\"\",{\"text\":\"tab\\\\there\\\\nnewline\\\\back\",\"color\":\"green\"},\"\\\"\"]}"),
        ("merge\t{a:1,b:{x:1,y:2},c:[1]}\t{b:{y:3,z:4},c:[2],d:5,a:{n:1}}", "MERGE {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"a\",\"color\":\"aqua\"},\":\",\" \",\"{\",\"\",{\"text\":\"n\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\"}\",\",\",\" \",\"\",{\"text\":\"b\",\"color\":\"aqua\"},\":\",\" \",\"{\",\"\",{\"text\":\"x\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"y\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"z\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"4\",\"color\":\"gold\"},\"}\",\",\",\" \",\"\",{\"text\":\"c\",\"color\":\"aqua\"},\":\",\" \",\"[\",{\"text\":\"2\",\"color\":\"gold\"},\"]\",\",\",\" \",\"\",{\"text\":\"d\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"5\",\"color\":\"gold\"},\"}\"]}"),
        ("merge\t{z1:0}\t{q:1,w:2,e:3,r:4,t:5,y:6,u:7,i:8,o:9,p:10,a:11,s:12,d:13,f:14}", "MERGE {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"a\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"11\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"d\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"13\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"e\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"3\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"f\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"14\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"i\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"8\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"o\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"9\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"p\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"10\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"q\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"r\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"4\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"s\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"12\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"t\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"5\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"u\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"7\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"z1\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"0\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"w\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"2\",\"color\":\"gold\"},\",\",\" \",\"\",{\"text\":\"y\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"6\",\"color\":\"gold\"},\"}\"]}"),
        ("merge\t{b:{x:1}}\t{b:{x:1}}", "MERGE {\"text\":\"\",\"extra\":[\"{\",\"\",{\"text\":\"b\",\"color\":\"aqua\"},\":\",\" \",\"{\",\"\",{\"text\":\"x\",\"color\":\"aqua\"},\":\",\" \",{\"text\":\"1\",\"color\":\"gold\"},\"}\",\"}\"]}"),
    ];
}
