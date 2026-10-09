use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::bail;
use crisp_protocol::{Reader, Writer};
use rsa::RsaPublicKey;
use rsa::pkcs8::{DecodePublicKey, EncodePublicKey};

use crate::network::chat::Component;
use crate::util::signature_validator::SignatureValidator;

pub const EXPIRED_PROFILE_PUBLIC_KEY: &str = "multiplayer.disconnect.expired_public_key";
const INVALID_SIGNATURE: &str = "multiplayer.disconnect.invalid_public_key_signature";
const MAX_KEY_SIGNATURE_SIZE: usize = 4096;

pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

pub fn read_byte_array<'a>(r: &mut Reader<'a>, max: usize) -> anyhow::Result<&'a [u8]> {
    let len = r.varint()?;
    if len < 0 || len as usize > max {
        bail!("ByteArray with size {len} is bigger than allowed {max}");
    }
    Ok(r.take(len as usize)?)
}

pub fn write_byte_array(w: &mut Writer, bytes: &[u8]) {
    w.varint(bytes.len() as i32);
    w.bytes(bytes);
}

#[derive(Clone, Debug)]
pub struct ProfilePublicKey {
    pub data: Data,
}

#[derive(Clone, Debug)]
pub struct Data {
    pub expires_at: i64,
    pub key: RsaPublicKey,
    pub key_signature: Vec<u8>,
}

impl PartialEq for Data {
    fn eq(&self, other: &Data) -> bool {
        self.expires_at == other.expires_at && self.key == other.key && self.key_signature == other.key_signature
    }
}

impl Data {
    pub fn read(r: &mut Reader) -> anyhow::Result<Data> {
        let expires_at = r.i64()?;
        let key = RsaPublicKey::from_public_key_der(read_byte_array(r, 512)?).map_err(|e| anyhow::anyhow!("Failed to decode public key: {e}"))?;
        let key_signature = read_byte_array(r, MAX_KEY_SIGNATURE_SIZE)?.to_vec();
        Ok(Data { expires_at, key, key_signature })
    }

    pub fn write(&self, w: &mut Writer) {
        w.i64(self.expires_at);
        write_byte_array(w, &self.encoded_key());
        write_byte_array(w, &self.key_signature);
    }

    fn encoded_key(&self) -> Vec<u8> {
        self.key.to_public_key_der().expect("an encodable key").into_vec()
    }

    fn signed_payload(&self, profile_id: u128) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&profile_id.to_be_bytes());
        payload.extend_from_slice(&self.expires_at.to_be_bytes());
        payload.extend_from_slice(&self.encoded_key());
        payload
    }

    fn validate_signature(&self, validator: &SignatureValidator, profile_id: u128) -> bool {
        validator.validate(&self.signed_payload(profile_id), &self.key_signature)
    }

    pub fn has_expired(&self) -> bool {
        self.expires_at < now_millis()
    }
}

impl ProfilePublicKey {
    pub fn create_validated(validator: &SignatureValidator, profile_id: u128, data: Data) -> Result<ProfilePublicKey, Component> {
        if !data.validate_signature(validator, profile_id) {
            return Err(Component::translatable(INVALID_SIGNATURE));
        }
        Ok(ProfilePublicKey { data })
    }

    pub fn create_signature_validator(&self) -> SignatureValidator {
        SignatureValidator::Key(self.data.key.clone())
    }
}
