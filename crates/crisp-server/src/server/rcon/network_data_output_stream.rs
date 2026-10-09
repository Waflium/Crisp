pub struct NetworkDataOutputStream {
    output_stream: Vec<u8>,
}

impl NetworkDataOutputStream {
    pub fn new(size: usize) -> NetworkDataOutputStream {
        NetworkDataOutputStream { output_stream: Vec::with_capacity(size) }
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        self.output_stream.extend_from_slice(data);
    }

    pub fn write_string(&mut self, data: &str) {
        self.output_stream.extend_from_slice(data.as_bytes());
        self.output_stream.push(0);
    }

    pub fn write(&mut self, data: u8) {
        self.output_stream.push(data);
    }

    pub fn write_short(&mut self, data: i16) {
        self.output_stream.extend_from_slice(&data.to_le_bytes());
    }

    pub fn to_byte_array(&self) -> &[u8] {
        &self.output_stream
    }

    pub fn reset(&mut self) {
        self.output_stream.clear();
    }
}
