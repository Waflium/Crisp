use aes::Aes128;
use cfb8::cipher::generic_array::GenericArray;
use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rsa::pkcs8::EncodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey};
use sha1::{Digest, Sha1};

pub struct ServerKeyPair {
    private: RsaPrivateKey,
    pub public_der: Vec<u8>,
}

impl ServerKeyPair {
    pub fn generate() -> anyhow::Result<ServerKeyPair> {
        let private = RsaPrivateKey::new(&mut rand_core::OsRng, 1024)?;
        let public_der = private.to_public_key().to_public_key_der()?.as_bytes().to_vec();
        Ok(ServerKeyPair { private, public_der })
    }

    pub fn decrypt(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        Ok(self.private.decrypt(Pkcs1v15Encrypt, data)?)
    }
}

pub fn server_hash(server_id: &str, secret: &[u8], public_der: &[u8]) -> String {
    let mut sha = Sha1::new();
    sha.update(server_id.chars().map(|c| c as u8).collect::<Vec<_>>());
    sha.update(secret);
    sha.update(public_der);
    java_signed_hex(&sha.finalize())
}

fn java_signed_hex(bytes: &[u8]) -> String {
    let negative = bytes[0] & 0x80 != 0;
    let mut magnitude = bytes.to_vec();
    if negative {
        let mut carry = true;
        for b in magnitude.iter_mut().rev() {
            *b = !*b;
            if carry {
                let (sum, overflow) = b.overflowing_add(1);
                *b = sum;
                carry = overflow;
            }
        }
    }
    let hex: String = magnitude.iter().map(|b| format!("{b:02x}")).collect();
    let trimmed = hex.trim_start_matches('0');
    let digits = if trimmed.is_empty() { "0" } else { trimmed };
    if negative { format!("-{digits}") } else { digits.to_string() }
}

pub struct Encryptor(cfb8::Encryptor<Aes128>);

impl Encryptor {
    pub fn new(secret: &[u8; 16]) -> Encryptor {
        Encryptor(cfb8::Encryptor::new(secret.into(), secret.into()))
    }

    pub fn apply(&mut self, buf: &mut [u8]) {
        for byte in buf.chunks_exact_mut(1) {
            self.0.encrypt_block_mut(GenericArray::from_mut_slice(byte));
        }
    }
}

pub struct Decryptor(cfb8::Decryptor<Aes128>);

impl Decryptor {
    pub fn new(secret: &[u8; 16]) -> Decryptor {
        Decryptor(cfb8::Decryptor::new(secret.into(), secret.into()))
    }

    pub fn apply(&mut self, buf: &mut [u8]) {
        for byte in buf.chunks_exact_mut(1) {
            self.0.decrypt_block_mut(GenericArray::from_mut_slice(byte));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha1_hex(name: &str) -> String {
        java_signed_hex(&Sha1::digest(name.as_bytes()))
    }

    #[test]
    fn signed_hex_digests() {
        assert_eq!(sha1_hex("Notch"), "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
        assert_eq!(sha1_hex("jeb_"), "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
        assert_eq!(sha1_hex("simon"), "88e16a1019277b15d58faf0541e11910eb756f6");
    }

    #[test]
    fn cipher_round_trip() {
        let key = [7u8; 16];
        let mut data: Vec<u8> = (0..100).collect();
        let mut enc = Encryptor::new(&key);
        let mut dec = Decryptor::new(&key);
        enc.apply(&mut data[..33]);
        enc.apply(&mut data[33..]);
        assert_ne!(data, (0..100).collect::<Vec<u8>>());
        dec.apply(&mut data[..50]);
        dec.apply(&mut data[50..]);
        assert_eq!(data, (0..100).collect::<Vec<u8>>());
    }
}
