use crisp_protocol::nbt::Nbt;

pub type HashCode = u32;

const TAG_MAP_START: u8 = 2;
const TAG_MAP_END: u8 = 3;
const TAG_LIST_START: u8 = 4;
const TAG_LIST_END: u8 = 5;
const TAG_BYTE: u8 = 6;
const TAG_SHORT: u8 = 7;
const TAG_INT: u8 = 8;
const TAG_LONG: u8 = 9;
const TAG_FLOAT: u8 = 10;
const TAG_DOUBLE: u8 = 11;
const TAG_STRING: u8 = 12;
const TAG_BOOLEAN: u8 = 13;
const TAG_BYTE_ARRAY_START: u8 = 14;
const TAG_BYTE_ARRAY_END: u8 = 15;
const TAG_INT_ARRAY_START: u8 = 16;
const TAG_INT_ARRAY_END: u8 = 17;
const TAG_LONG_ARRAY_START: u8 = 18;
const TAG_LONG_ARRAY_END: u8 = 19;

static CRC32C_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut k = 0;
        while k < 8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0x82F6_3B78 } else { crc >> 1 };
            k += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

#[derive(Clone)]
pub struct Hasher(u32);

impl Default for Hasher {
    fn default() -> Hasher {
        Hasher::new()
    }
}

impl Hasher {
    pub fn new() -> Hasher {
        Hasher(!0)
    }

    pub fn put_bytes(&mut self, bytes: &[u8]) -> &mut Hasher {
        let mut crc = self.0;
        for &b in bytes {
            crc = CRC32C_TABLE[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        self.0 = crc;
        self
    }

    pub fn put_byte(&mut self, b: u8) -> &mut Hasher {
        self.put_bytes(&[b])
    }

    pub fn put_short(&mut self, v: i16) -> &mut Hasher {
        self.put_bytes(&v.to_le_bytes())
    }

    pub fn put_int(&mut self, v: i32) -> &mut Hasher {
        self.put_bytes(&v.to_le_bytes())
    }

    pub fn put_long(&mut self, v: i64) -> &mut Hasher {
        self.put_bytes(&v.to_le_bytes())
    }

    pub fn put_hash(&mut self, hash: HashCode) -> &mut Hasher {
        self.put_bytes(&hash.to_le_bytes())
    }

    pub fn hash(&self) -> HashCode {
        !self.0
    }
}

pub fn hash_bytes(bytes: &[u8]) -> HashCode {
    Hasher::new().put_bytes(bytes).hash()
}


pub fn create_byte(v: i8) -> HashCode {
    Hasher::new().put_byte(TAG_BYTE).put_byte(v as u8).hash()
}

pub fn create_short(v: i16) -> HashCode {
    Hasher::new().put_byte(TAG_SHORT).put_short(v).hash()
}

pub fn create_int(v: i32) -> HashCode {
    Hasher::new().put_byte(TAG_INT).put_int(v).hash()
}

pub fn create_long(v: i64) -> HashCode {
    Hasher::new().put_byte(TAG_LONG).put_long(v).hash()
}

pub fn create_float(v: f32) -> HashCode {
    Hasher::new().put_byte(TAG_FLOAT).put_int(v.to_bits() as i32).hash()
}

pub fn create_double(v: f64) -> HashCode {
    Hasher::new().put_byte(TAG_DOUBLE).put_long(v.to_bits() as i64).hash()
}

pub fn create_string(s: &str) -> HashCode {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut h = Hasher::new();
    h.put_byte(TAG_STRING).put_int(units.len() as i32);
    for u in units {
        h.put_bytes(&u.to_le_bytes());
    }
    h.hash()
}

pub fn create_boolean(v: bool) -> HashCode {
    hash_bytes(&[TAG_BOOLEAN, v as u8])
}

pub fn create_map(mut entries: Vec<(HashCode, HashCode)>) -> HashCode {
    entries.sort_unstable();
    let mut h = Hasher::new();
    h.put_byte(TAG_MAP_START);
    for (k, v) in entries {
        h.put_hash(k).put_hash(v);
    }
    h.put_byte(TAG_MAP_END).hash()
}

pub fn create_list(items: impl IntoIterator<Item = HashCode>) -> HashCode {
    let mut h = Hasher::new();
    h.put_byte(TAG_LIST_START);
    for v in items {
        h.put_hash(v);
    }
    h.put_byte(TAG_LIST_END).hash()
}

pub fn create_byte_list(values: &[i8]) -> HashCode {
    let mut h = Hasher::new();
    h.put_byte(TAG_BYTE_ARRAY_START);
    for &v in values {
        h.put_byte(v as u8);
    }
    h.put_byte(TAG_BYTE_ARRAY_END).hash()
}

pub fn create_int_list(values: &[i32]) -> HashCode {
    let mut h = Hasher::new();
    h.put_byte(TAG_INT_ARRAY_START);
    for &v in values {
        h.put_int(v);
    }
    h.put_byte(TAG_INT_ARRAY_END).hash()
}

pub fn create_long_list(values: &[i64]) -> HashCode {
    let mut h = Hasher::new();
    h.put_byte(TAG_LONG_ARRAY_START);
    for &v in values {
        h.put_long(v);
    }
    h.put_byte(TAG_LONG_ARRAY_END).hash()
}

pub fn hash_nbt(tag: &Nbt) -> HashCode {
    match tag {
        Nbt::Byte(v) => create_byte(*v),
        Nbt::Short(v) => create_short(*v),
        Nbt::Int(v) => create_int(*v),
        Nbt::Long(v) => create_long(*v),
        Nbt::Float(v) => create_float(*v),
        Nbt::Double(v) => create_double(*v),
        Nbt::ByteArray(v) => create_byte_list(v),
        Nbt::String(s) => create_string(s),
        Nbt::List(items) => create_list(items.iter().map(hash_nbt)),
        Nbt::Compound(fields) => create_map(fields.iter().map(|(k, v)| (create_string(k), hash_nbt(v))).collect()),
        Nbt::IntArray(v) => create_int_list(v),
        Nbt::LongArray(v) => create_long_list(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32c_check_value() {
        assert_eq!(hash_bytes(b"123456789"), 0xE306_9283);
    }
}
