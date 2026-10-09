use std::sync::Arc;

use crisp_protocol::{Reader, Writer};

use super::message_signature_cache::MessageSignatureCache;

pub const BYTES: usize = 256;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MessageSignature(pub Arc<[u8; BYTES]>);

impl std::fmt::Debug for MessageSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use base64::Engine;
        f.write_str(&base64::engine::general_purpose::STANDARD.encode(&self.0[..]))
    }
}

impl MessageSignature {
    pub fn read(r: &mut Reader) -> anyhow::Result<MessageSignature> {
        Ok(MessageSignature(Arc::new(r.take(BYTES)?.try_into().unwrap())))
    }

    pub fn write(&self, w: &mut Writer) {
        w.bytes(&self.0[..]);
    }

    pub fn read_optional(r: &mut Reader) -> anyhow::Result<Option<MessageSignature>> {
        Ok(if r.bool()? { Some(MessageSignature::read(r)?) } else { None })
    }

    pub fn pack(&self, cache: &MessageSignatureCache) -> Packed {
        match cache.pack(self) {
            -1 => Packed::Full(self.clone()),
            id => Packed::Id(id),
        }
    }

    pub fn checksum(&self) -> i32 {
        self.0.iter().fold(1i32, |h, &b| h.wrapping_mul(31).wrapping_add(b as i8 as i32))
    }
}

pub enum Packed {
    Id(i32),
    Full(MessageSignature),
}

impl Packed {
    pub fn write(&self, w: &mut Writer) {
        match self {
            Packed::Id(id) => w.varint(id + 1),
            Packed::Full(signature) => {
                w.varint(0);
                signature.write(w);
            }
        }
    }
}
