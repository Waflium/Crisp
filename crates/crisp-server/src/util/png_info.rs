pub struct PngInfo {
    pub width: i32,
    pub height: i32,
}

const PNG_HEADER: i64 = -8552249625308161526;
const IHDR_TYPE: i32 = 1229472850;
const IHDR_SIZE: i32 = 13;

impl PngInfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<PngInfo, String> {
        let mut stream = DataInputStream { bytes, pos: 0 };
        let magic = stream.read_long()?;
        if magic != PNG_HEADER {
            return Err(format!("java.io.IOException: Bad PNG Signature: 0x{magic:016X}"));
        }
        let header_size = stream.read_int()?;
        if header_size != IHDR_SIZE {
            return Err(format!("java.io.IOException: Bad length for IHDR chunk: {header_size}"));
        }
        let header_type = stream.read_int()?;
        if header_type != IHDR_TYPE {
            return Err(format!("java.io.IOException: Bad type for IHDR chunk: 0x{header_type:08X}"));
        }
        let width = stream.read_int()?;
        let height = stream.read_int()?;
        let bit_depth = stream.read_byte()?;
        if !matches!(bit_depth, 1 | 2 | 4 | 8 | 16) {
            return Err(format!("java.io.IOException: Invalid bit depth: {bit_depth}"));
        }
        let color_type = stream.read_byte()?;
        if !matches!(color_type, 0 | 2 | 3 | 4 | 6) {
            return Err(format!("java.io.IOException: Invalid color type: {color_type}"));
        }
        if bit_depth == 16 && i32::try_from(4i64.wrapping_mul(width as i64).wrapping_mul(height as i64).wrapping_mul(2)).is_err() {
            return Err(format!("java.io.IOException: Bounds check failed, {width}x{height}@{bit_depth}, type: {color_type}"));
        }
        Ok(PngInfo { width, height })
    }
}

struct DataInputStream<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl DataInputStream<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let bytes = self.bytes.get(self.pos..self.pos + N).ok_or_else(|| "java.io.EOFException".to_string())?;
        self.pos += N;
        Ok(bytes.try_into().unwrap())
    }

    fn read_long(&mut self) -> Result<i64, String> {
        self.take().map(i64::from_be_bytes)
    }

    fn read_int(&mut self) -> Result<i32, String> {
        self.take().map(i32::from_be_bytes)
    }

    fn read_byte(&mut self) -> Result<i8, String> {
        self.take::<1>().map(|[b]| b as i8)
    }
}
