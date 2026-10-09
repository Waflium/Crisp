use rsa::RsaPublicKey;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::signature::Verifier;

#[derive(Clone)]
pub enum SignatureValidator {
    Key(RsaPublicKey),
    Services(Vec<RsaPublicKey>),
}

impl SignatureValidator {
    pub fn validate(&self, payload: &[u8], signature: &[u8]) -> bool {
        let Ok(signature) = Signature::try_from(signature) else { return false };
        match self {
            SignatureValidator::Key(key) => VerifyingKey::<rsa::sha2::Sha256>::new(key.clone()).verify(payload, &signature).is_ok(),
            SignatureValidator::Services(keys) => keys.iter().any(|key| VerifyingKey::<sha1::Sha1>::new(key.clone()).verify(payload, &signature).is_ok()),
        }
    }
}
