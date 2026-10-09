use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedEof,
    VarIntTooLong,
    StringTooLong,
    InvalidUtf8,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnexpectedEof => "unexpected end of packet",
            Self::VarIntTooLong => "VarInt/VarLong is too long",
            Self::StringTooLong => "string exceeds its maximum length",
            Self::InvalidUtf8 => "string is not valid UTF-8",
        })
    }
}

impl std::error::Error for DecodeError {}

pub struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len()
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.buf.len() < n {
            return Err(DecodeError::UnexpectedEof);
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        Ok(self.u8()? != 0)
    }

    pub fn i16(&mut self) -> Result<i16, DecodeError> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn position(&mut self) -> Result<(i32, i32, i32), DecodeError> {
        let v = self.i64()?;
        Ok(((v >> 38) as i32, (v << 52 >> 52) as i32, (v << 26 >> 38) as i32))
    }

    pub fn i64(&mut self) -> Result<i64, DecodeError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn f64(&mut self) -> Result<f64, DecodeError> {
        Ok(f64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub fn varint(&mut self) -> Result<i32, DecodeError> {
        let mut value: u32 = 0;
        for i in 0..5 {
            let byte = self.u8()?;
            value |= ((byte & 0x7F) as u32) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        Err(DecodeError::VarIntTooLong)
    }

    pub fn varlong(&mut self) -> Result<i64, DecodeError> {
        let mut value: u64 = 0;
        for i in 0..10 {
            let byte = self.u8()?;
            value |= ((byte & 0x7F) as u64) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(value as i64);
            }
        }
        Err(DecodeError::VarIntTooLong)
    }

    pub fn string(&mut self, max_len: usize) -> Result<&'a str, DecodeError> {
        let byte_len = self.varint()?;
        let byte_len = usize::try_from(byte_len).map_err(|_| DecodeError::StringTooLong)?;
        if byte_len > max_len * 3 {
            return Err(DecodeError::StringTooLong);
        }
        let s = std::str::from_utf8(self.take(byte_len)?).map_err(|_| DecodeError::InvalidUtf8)?;
        if s.encode_utf16().count() > max_len {
            return Err(DecodeError::StringTooLong);
        }
        Ok(s)
    }

    pub fn uuid(&mut self) -> Result<u128, DecodeError> {
        Ok(u128::from_be_bytes(self.take(16)?.try_into().unwrap()))
    }

    pub fn lp_vec3(&mut self) -> Result<(f64, f64, f64), DecodeError> {
        let first = self.u8()? as u64;
        if first == 0 {
            return Ok((0.0, 0.0, 0.0));
        }
        let second = self.u8()? as u64;
        let rest = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as u64;
        let packed = rest << 16 | second << 8 | first;
        let mut scale = first & 3;
        if first & 4 != 0 {
            scale |= (self.varint()? as u32 as u64) << 2;
        }
        let unpack = |v: u64| ((v & 0x7FFF).min(32766) as f64 * 2.0 / 32766.0 - 1.0) * scale as f64;
        Ok((unpack(packed >> 3), unpack(packed >> 18), unpack(packed >> 33)))
    }
}

#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_be_bytes());
    }

    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }

    pub fn i16(&mut self, v: i16) {
        self.bytes(&v.to_be_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.bytes(&v.to_be_bytes());
    }

    pub fn i64(&mut self, v: i64) {
        self.bytes(&v.to_be_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.bytes(&v.to_be_bytes());
    }

    pub fn f64(&mut self, v: f64) {
        self.bytes(&v.to_be_bytes());
    }

    pub fn bitset(&mut self, longs: &[u64]) {
        let bytes: Vec<u8> = longs.iter().flat_map(|l| l.to_le_bytes()).collect();
        let len = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        self.varint(len as i32);
        self.bytes(&bytes[..len]);
    }

    pub fn lp_vec3(&mut self, x: f64, y: f64, z: f64) {
        let max = x.abs().max(y.abs()).max(z.abs());
        if max < 1.0 / 32766.0 {
            self.u8(0);
            return;
        }
        let scale = max.ceil() as u64;
        let continued = scale > 3;
        let pack = |v: f64| ((v / scale as f64 * 0.5 + 0.5) * 32766.0 + 0.5).floor() as u64;
        let packed = (if continued { scale & 3 | 4 } else { scale }) | pack(x) << 3 | pack(y) << 18 | pack(z) << 33;
        self.u8(packed as u8);
        self.u8((packed >> 8) as u8);
        self.bytes(&((packed >> 16) as u32).to_be_bytes());
        if continued {
            self.varint((scale >> 2) as i32);
        }
    }

    pub fn position(&mut self, x: i32, y: i32, z: i32) {
        let packed = ((x as i64 & 0x3FF_FFFF) << 38) | ((z as i64 & 0x3FF_FFFF) << 12) | (y as i64 & 0xFFF);
        self.i64(packed);
    }

    pub fn varint(&mut self, v: i32) {
        let mut v = v as u32;
        while v & !0x7F != 0 {
            self.buf.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
        self.buf.push(v as u8);
    }

    pub fn varlong(&mut self, v: i64) {
        let mut v = v as u64;
        while v & !0x7F != 0 {
            self.buf.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
        self.buf.push(v as u8);
    }

    pub fn string(&mut self, s: &str) {
        self.varint(s.len() as i32);
        self.bytes(s.as_bytes());
    }

    pub fn uuid(&mut self, v: u128) {
        self.bytes(&v.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARINTS: &[(i32, &[u8])] = &[
        (0, &[0x00]),
        (1, &[0x01]),
        (2, &[0x02]),
        (127, &[0x7f]),
        (128, &[0x80, 0x01]),
        (255, &[0xff, 0x01]),
        (25565, &[0xdd, 0xc7, 0x01]),
        (2097151, &[0xff, 0xff, 0x7f]),
        (2147483647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
        (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
        (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0x08]),
    ];

    const VARLONGS: &[(i64, &[u8])] = &[
        (0, &[0x00]),
        (127, &[0x7f]),
        (128, &[0x80, 0x01]),
        (2147483647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
        (9223372036854775807, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]),
        (-1, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]),
        (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0xf8, 0xff, 0xff, 0xff, 0xff, 0x01]),
        (-9223372036854775808, &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]),
    ];

    #[test]
    fn varint_matches_spec() {
        for &(value, bytes) in VARINTS {
            let mut w = Writer::new();
            w.varint(value);
            assert_eq!(w.as_slice(), bytes, "encode {value}");
            assert_eq!(Reader::new(bytes).varint(), Ok(value), "decode {value}");
        }
    }

    #[test]
    fn varlong_matches_spec() {
        for &(value, bytes) in VARLONGS {
            let mut w = Writer::new();
            w.varlong(value);
            assert_eq!(w.as_slice(), bytes, "encode {value}");
            assert_eq!(Reader::new(bytes).varlong(), Ok(value), "decode {value}");
        }
    }

    #[test]
    fn varint_rejects_sixth_byte() {
        let bytes = [0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
        assert_eq!(Reader::new(&bytes).varint(), Err(DecodeError::VarIntTooLong));
    }

    #[test]
    fn bitset_matches_vanilla() {
        let mut w = Writer::new();
        w.bitset(&[0b110]);
        w.bitset(&[0]);
        w.bitset(&[0b111, 0]);
        assert_eq!(w.as_slice(), &[0x01, 0x06, 0x00, 0x01, 0x07]);
        let mut w = Writer::new();
        w.bitset(&[0x3FF_FFFF]);
        w.bitset(&[0x100, 0b1]);
        assert_eq!(w.as_slice(), &[0x04, 0xFF, 0xFF, 0xFF, 0x03, 0x09, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0x01]);
    }

    #[test]
    fn lp_vec3_round_trips_vanilla_bytes() {
        for bytes in [[0x01, 0x13, 0x85, 0x51, 0x33, 0x32], [0xf9, 0xda, 0x79, 0xe7, 0x33, 0x31]] {
            let (x, y, z) = Reader::new(&bytes).lp_vec3().unwrap();
            assert!((y - 0.2).abs() < 1e-4 && x.abs() <= 0.1 && z.abs() <= 0.1);
            let mut w = Writer::new();
            w.lp_vec3(x, y, z);
            assert_eq!(w.as_slice(), bytes);
        }
        let mut w = Writer::new();
        w.lp_vec3(0.0, 0.0, 0.0);
        assert_eq!(w.as_slice(), [0]);
        let mut w = Writer::new();
        w.lp_vec3(10.0, -5.0, 0.0);
        let (x, y, _) = Reader::new(w.as_slice()).lp_vec3().unwrap();
        assert!((x - 10.0).abs() < 0.01 && (y + 5.0).abs() < 0.01);
    }

    #[test]
    fn position_matches_spec() {
        let mut w = Writer::new();
        w.position(18357644, 831, -20882616);
        let expected: u64 = 0b01000110000001110110001100_10110000010101101101001000_001100111111;
        assert_eq!(w.as_slice(), &expected.to_be_bytes());
    }

    #[test]
    fn string_enforces_max_len() {
        let mut w = Writer::new();
        w.string("abcd");
        assert_eq!(Reader::new(w.as_slice()).string(4), Ok("abcd"));
        assert_eq!(Reader::new(w.as_slice()).string(3), Err(DecodeError::StringTooLong));
    }
}
