use crate::{DecodeError, Reader, Writer};

#[derive(Clone, Debug)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<Nbt>),
    Compound(Vec<(String, Nbt)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

pub fn float_tag_value(v: f32) -> f32 {
    if v == 0.0 { 0.0 } else { v }
}

pub fn double_tag_value(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

impl PartialEq for Nbt {
    fn eq(&self, other: &Nbt) -> bool {
        match (self, other) {
            (Nbt::Byte(a), Nbt::Byte(b)) => a == b,
            (Nbt::Short(a), Nbt::Short(b)) => a == b,
            (Nbt::Int(a), Nbt::Int(b)) => a == b,
            (Nbt::Long(a), Nbt::Long(b)) => a == b,
            (Nbt::Float(a), Nbt::Float(b)) => float_tag_value(*a).to_bits() == float_tag_value(*b).to_bits(),
            (Nbt::Double(a), Nbt::Double(b)) => double_tag_value(*a).to_bits() == double_tag_value(*b).to_bits(),
            (Nbt::ByteArray(a), Nbt::ByteArray(b)) => a == b,
            (Nbt::String(a), Nbt::String(b)) => a == b,
            (Nbt::List(a), Nbt::List(b)) => a == b,
            (Nbt::Compound(a), Nbt::Compound(b)) => a.len() == b.len() && a.iter().all(|(k, v)| b.iter().any(|(k2, v2)| k == k2 && v == v2)),
            (Nbt::IntArray(a), Nbt::IntArray(b)) => a == b,
            (Nbt::LongArray(a), Nbt::LongArray(b)) => a == b,
            _ => false,
        }
    }
}

impl Nbt {
    fn tag_id(&self) -> u8 {
        match self {
            Nbt::Byte(_) => 1,
            Nbt::Short(_) => 2,
            Nbt::Int(_) => 3,
            Nbt::Long(_) => 4,
            Nbt::Float(_) => 5,
            Nbt::Double(_) => 6,
            Nbt::ByteArray(_) => 7,
            Nbt::String(_) => 8,
            Nbt::List(_) => 9,
            Nbt::Compound(_) => 10,
            Nbt::IntArray(_) => 11,
            Nbt::LongArray(_) => 12,
        }
    }

    pub fn write_named(&self, name: &str, w: &mut Writer) {
        w.u8(self.tag_id());
        write_string(w, name);
        self.write_payload(w);
    }

    pub fn read_named(r: &mut Reader) -> Result<(String, Nbt), DecodeError> {
        let id = r.u8()?;
        let name = read_string(r)?;
        Ok((name, read_payload(r, id, 0)?))
    }

    pub fn read(r: &mut Reader) -> Result<Nbt, DecodeError> {
        let id = r.u8()?;
        read_payload(r, id, 0)
    }

    pub fn get(&self, key: &str) -> Option<&Nbt> {
        match self {
            Nbt::Compound(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Nbt::Byte(v) => Some(v as i64),
            Nbt::Short(v) => Some(v as i64),
            Nbt::Int(v) => Some(v as i64),
            Nbt::Long(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Nbt::Float(v) => Some(v as f64),
            Nbt::Double(v) => Some(v),
            _ => self.as_i64().map(|v| v as f64),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Nbt::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> &[Nbt] {
        match self {
            Nbt::List(items) => items,
            _ => &[],
        }
    }

    pub fn write(&self, w: &mut Writer) {
        w.u8(self.tag_id());
        self.write_payload(w);
    }

    fn write_payload(&self, w: &mut Writer) {
        match self {
            Nbt::Byte(v) => w.u8(*v as u8),
            Nbt::Short(v) => w.bytes(&v.to_be_bytes()),
            Nbt::Int(v) => w.bytes(&v.to_be_bytes()),
            Nbt::Long(v) => w.i64(*v),
            Nbt::Float(v) => w.bytes(&float_tag_value(*v).to_be_bytes()),
            Nbt::Double(v) => w.bytes(&double_tag_value(*v).to_be_bytes()),
            Nbt::ByteArray(v) => {
                w.bytes(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|b| w.u8(*b as u8));
            }
            Nbt::String(s) => write_string(w, s),
            Nbt::List(items) => {
                let element = list_element_type(items);
                w.u8(element);
                w.bytes(&(items.len() as i32).to_be_bytes());
                for item in items {
                    if element == 10 && !matches!(item, Nbt::Compound(f) if !is_wrapper(f)) {
                        w.u8(item.tag_id());
                        write_string(w, "");
                        item.write_payload(w);
                        w.u8(0);
                    } else {
                        item.write_payload(w);
                    }
                }
            }
            Nbt::Compound(fields) => {
                for &i in &java_hash_map_order(&fields.iter().map(|f| f.0.as_str()).collect::<Vec<_>>()) {
                    let (name, value) = &fields[i];
                    w.u8(value.tag_id());
                    write_string(w, name);
                    value.write_payload(w);
                }
                w.u8(0);
            }
            Nbt::IntArray(v) => {
                w.bytes(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|i| w.bytes(&i.to_be_bytes()));
            }
            Nbt::LongArray(v) => {
                w.bytes(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|l| w.i64(*l));
            }
        }
    }
}

fn write_string(w: &mut Writer, s: &str) {
    let mut bytes = Vec::with_capacity(s.len());
    for unit in s.encode_utf16() {
        match unit {
            0x0001..=0x007F => bytes.push(unit as u8),
            0x0000 | 0x0080..=0x07FF => {
                bytes.push(0xC0 | (unit >> 6) as u8);
                bytes.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                bytes.push(0xE0 | (unit >> 12) as u8);
                bytes.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                bytes.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    w.u16(bytes.len() as u16);
    w.bytes(&bytes);
}

fn read_string(r: &mut Reader) -> Result<String, DecodeError> {
    let len = r.u16()? as usize;
    let mut units = Vec::with_capacity(len);
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(r.u8()?);
    }
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i] as u16;
        let (unit, size) = if b & 0x80 == 0 {
            (b, 1)
        } else if b & 0xE0 == 0xC0 && i + 1 < bytes.len() {
            ((b & 0x1F) << 6 | (bytes[i + 1] as u16 & 0x3F), 2)
        } else if b & 0xF0 == 0xE0 && i + 2 < bytes.len() {
            ((b & 0x0F) << 12 | (bytes[i + 1] as u16 & 0x3F) << 6 | (bytes[i + 2] as u16 & 0x3F), 3)
        } else {
            return Err(DecodeError::InvalidUtf8);
        };
        units.push(unit);
        i += size;
    }
    String::from_utf16(&units).map_err(|_| DecodeError::InvalidUtf8)
}

fn read_len(r: &mut Reader) -> Result<usize, DecodeError> {
    usize::try_from(r.i32()?).map_err(|_| DecodeError::UnexpectedEof)
}

fn read_payload(r: &mut Reader, id: u8, depth: u32) -> Result<Nbt, DecodeError> {
    if depth > 512 {
        return Err(DecodeError::UnexpectedEof);
    }
    Ok(match id {
        1 => Nbt::Byte(r.u8()? as i8),
        2 => Nbt::Short(r.i16()?),
        3 => Nbt::Int(r.i32()?),
        4 => Nbt::Long(r.i64()?),
        5 => Nbt::Float(r.f32()?),
        6 => Nbt::Double(r.f64()?),
        7 => Nbt::ByteArray((0..read_len(r)?).map(|_| r.u8().map(|b| b as i8)).collect::<Result<_, _>>()?),
        8 => Nbt::String(read_string(r)?),
        9 => {
            let element = r.u8()?;
            let len = read_len(r)?;
            Nbt::List((0..len).map(|_| read_payload(r, element, depth + 1).map(try_unwrap)).collect::<Result<_, _>>()?)
        }
        10 => {
            let mut fields = Vec::new();
            loop {
                let field = r.u8()?;
                if field == 0 {
                    break;
                }
                let name = read_string(r)?;
                fields.push((name, read_payload(r, field, depth + 1)?));
            }
            Nbt::Compound(fields)
        }
        11 => Nbt::IntArray((0..read_len(r)?).map(|_| r.i32()).collect::<Result<_, _>>()?),
        12 => Nbt::LongArray((0..read_len(r)?).map(|_| r.i64()).collect::<Result<_, _>>()?),
        _ => return Err(DecodeError::UnexpectedEof),
    })
}

fn list_element_type(items: &[Nbt]) -> u8 {
    let mut homogeneous = 0;
    for item in items {
        let id = item.tag_id();
        if homogeneous == 0 {
            homogeneous = id;
        } else if homogeneous != id {
            return 10;
        }
    }
    homogeneous
}

fn is_wrapper(fields: &[(String, Nbt)]) -> bool {
    fields.len() == 1 && fields[0].0.is_empty()
}

fn try_unwrap(tag: Nbt) -> Nbt {
    match tag {
        Nbt::Compound(mut fields) if is_wrapper(&fields) => fields.pop().unwrap().1,
        other => other,
    }
}

pub fn text(s: impl Into<String>) -> Nbt {
    Nbt::String(s.into())
}

fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

pub fn java_hash_map_order<S: AsRef<str>>(keys: &[S]) -> Vec<usize> {
    let mut capacity = 16usize;
    while keys.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let bucket = |key: &str| {
        let h = java_string_hash(key);
        ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (capacity - 1)
    };
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&i| bucket(keys[i].as_ref()));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_layout() {
        let mut w = Writer::new();
        Nbt::Compound(vec![("a".into(), Nbt::Byte(1))]).write(&mut w);
        assert_eq!(w.as_slice(), &[10, 1, 0, 1, b'a', 1, 0]);
    }

    #[test]
    fn named_round_trip() {
        let value = Nbt::Compound(vec![
            ("name".into(), Nbt::String(" é😀".into())),
            ("list".into(), Nbt::List(vec![Nbt::Int(1), Nbt::Int(2)])),
            ("longs".into(), Nbt::LongArray(vec![-1, 7])),
            ("nested".into(), Nbt::Compound(vec![("f".into(), Nbt::Float(0.5))])),
        ]);
        let mut w = Writer::new();
        value.write_named("root", &mut w);
        let (name, read) = Nbt::read_named(&mut Reader::new(w.as_slice())).unwrap();
        assert_eq!((name.as_str(), read), ("root", value));
    }

    #[test]
    fn mixed_lists_wrap_on_the_wire() {
        let wrapper = Nbt::Compound(vec![(String::new(), Nbt::Int(1))]);
        for list in [Nbt::List(vec![wrapper.clone(), Nbt::Int(2)]), Nbt::List(vec![wrapper]), Nbt::List(vec![Nbt::String("a".into()), Nbt::Byte(1)])] {
            let mut w = Writer::new();
            list.write(&mut w);
            assert_eq!(w.as_slice()[1], 10);
            assert_eq!(Nbt::read(&mut Reader::new(w.as_slice())).unwrap(), list);
        }
    }

    #[test]
    fn modified_utf8() {
        let mut w = Writer::new();
        Nbt::String("\0😀".into()).write(&mut w);
        assert_eq!(w.as_slice(), &[8, 0, 8, 0xC0, 0x80, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }
}
