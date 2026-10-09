use base64::Engine;

pub struct Favicon {
    pub icon_bytes: Vec<u8>,
}

impl Favicon {
    pub fn encoded(&self) -> String {
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&self.icon_bytes))
    }
}
