use crisp_protocol::{Reader, Writer};
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;

use super::last_seen_messages::LastSeenMessages;
use super::message_signature::MessageSignature;
use super::message_signature_cache::MessageSignatureCache;
use super::signed_message_body::SignedMessageBody;
use super::signed_message_chain::Decoder;
use crate::util::signature_validator::SignatureValidator;
use crate::world::entity::player::profile_public_key::{self, ProfilePublicKey};

const ORACLE: &str = "services_key 30820122300d06092a864886f70d01010105000382010f003082010a02820101008a44b19828bfe47f50c197b0dfab9e1e0b9d7d25a47715d9f7be2bdce488a5f2b700b48f46e2b17e7c9a0610b1aded35805a8150887417cd1c81ae052b8bf51f8974443275dcb7390db286c339a276c7899e734901fb67dced31b3231c2ea05d0aefc25c9e713b5a52e48cfca48c9276b4bc9310d1f8a911d72e11505b5046e1ec0bc3ccc5a8e9894b92027e54b54901d96ceb9ecf49f62a77f18739adfded3dd98b9813a489f7059964098eebc493a8faf67b2fddfc948456570c21d400efb7ab9657a044f74c2baa4e7dab1905ff274dd552d415760e0917eb1301f50db339e2f30188fa1879bd12edead1960b56275c8613905fcfa0ae2b4cdbe6a6f437970203010001
key_data 000003bb2cc3d800a60230820122300d06092a864886f70d01010105000382010f003082010a0282010100b7cd33ba1df7b253f42cc248abf60b6d4d02bc2e9a71c7dcf7c5ce37f935de768a5dff68ca990e7a9c1af7806ef7a794955c26e1e52744cfd951a1a49878ca7206e4b4dbad6837fe3cf4d22f09011a2cfe50830eb956e5d941b7c3ec2d144a6940b43207a0aa23a7ebb38dc2fbef26ff9ced8832f90fe0b89b659e871a332fb199d649f4cc99c9e9cfabe09a67190418f0c1677d9f048c1a2c86829f72a129609611f9a5092f132267996f1a002c3c9bbfd7e607bf1ae811278298ee8688b78a795e29cceaf989ed80298ac785d557d0c5c86d4577977a06e1cd949628702d5a8e1525b63352649aab6aa0c847aabee73310018c97041435e90f95836f04103b020301000180024b1b5e02ab0ec69c5679e3134bdd2403dc2c01705399425fe93ee5727312e36df4a35d96d7358f08c5a77487c6ec193a23aa9995ce709be6d4e078ee684079732bcc144d7c717b78c674fc45a2ca6f2c9b58d720ed906b7389c0f1f6f03af5e16f33df46aa23c35fb24e5602036c76b865c288f0382533734571823f8aa679ddc915ca7a469dd478e048a71d8c06843adc374a999ff0934d5757a8e9a0741d6fea609c0f90daa58c928f2950dcf86c1350f4c46c62e2ef563a7b96adda1d4363dbd220207cd2aa5c1b6bdcc85a5da024bc82be16e8ee8e30c4b02264df7299332e35372ad0e3c093c22bd9c449f04fc7741d056556314c70a2cab7450bc7f661
message 5 1760000000123 1234567 0 83e564648ed93d9bc5d16f7a597638de6c6d2cf0ca97aab7817eac047b6ee3c138421c8c33e5421dc018bcb430c97aa3d2e49a6543577239db9d4f3b9fafa24415c220e815cda5825c7384bd42d8d615239def252a8ee2a8f012b33cd145ff89c11bbf7e435959cf575f50e6111cac60c305b1c281ef2c5dd9e3522b5c062dd5b74da8ae8c82d76c82c57b67ac55378ffd0fd2db5efd22aa0d14fc4f3ae4fa1ba852651bd3a2ec0f2735dc864f9fb51bdb17d3e95ed3343d4e17ffbe82c380ea21775cab6324d8f289aed0143b1d9088d222b1537037aa4b53df0796bf6e14712cc0f7ffd371ac7be66eb18fbda9932d17982c171a1ff4930e77f9fc77f1dcb6
checksum 1
packed 0568656c6c6f00000199c82cc07b000000000012d68700
message 13 1760000001123 2469134 1 7c64465ac29702e67b54f719ec0a49e127944673435d3ee5bf8d245b2eff336f1632650be0cab1a97b139051db3e42770ced6b752264e6f77b79e22e8780ed148830e44c9b227a6d304ca34d551eddd212f414f0a3b3638b4626eeb56e7a2d1e6f148638c8dc3aedd3af38dd9a59f674fdca257eb2dd3688ed519e65e613c5be40f4dd36058f2eaa0b6206bbfb2e4f37010a2d42907771c4f5ed567245074456605e058e7d1c6e5662fd8b271a3d9dff9a4dac20bd0529232234115c64baefb761e3366caa454ac117d1d30ae25ebc6a780eaa48e0325e65371fc873297859573c5a3c4db710c1b2bcf0595a706f2880428e288ebd924116544fd61e8abee2b8
checksum 32
packed 1168c3a96c6c6f2077c3b6726c6420e29c9300000199c82cc463000000000025ad0e0101
message 5 1760000002123 3703701 2 00c7010b5095f48921b83b54c0630f87c39c0d1fd0b4cec7d2d8d580cac6e486477dc484ae14842cdfc2dd65b75ae61b81dbdef9f0859199514e95e3118fe8f5eb0f511e405c70f88fa9dd01f6df8e821afe49a71f6a8bc08fbe4d6d8e654e6c2a00672727ef78e4eee0e87253897b1aaa2c45328401e2e0fa89d8bcfb8de210ac2bd7fefca4181d45eb9bbffbdf4e805706e15af0d72415369b654513329d2a3b3408b18eb164e4713bb9b579dc1ef0c2b3e9ca8b461ab333ce8805d493d56e8d3f3448b84a52d50f3253eb248b35d73654006d177f82fbd00868b7f389df3ca8b24454022f77e50298deee15ed4d0aeaad404bc31b9b263b47cfff456782ee
checksum 177
packed 05746869726400000199c82cc84b0000000000388395020201";

const PROFILE_ID: u128 = 0x0f8d5a39_2b6c_4b1e_9d6e_6f0c2c1b7a11;
const SESSION_ID: u128 = 0x5b3f1a2e_8c4d_4e9f_a1b2_c3d4e5f60718;
const CONTENTS: [&str; 3] = ["hello", "h\u{e9}llo w\u{f6}rld \u{2713}", "third"];

fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn values(name: &str) -> Vec<Vec<&'static str>> {
    ORACLE.lines().filter_map(|l| l.strip_prefix(name)?.strip_prefix(' ')).map(|l| l.split(' ').collect()).collect()
}

fn key() -> ProfilePublicKey {
    let services = RsaPublicKey::from_public_key_der(&hex(values("services_key")[0][0])).unwrap();
    let data = profile_public_key::Data::read(&mut Reader::new(&hex(values("key_data")[0][0]))).unwrap();
    ProfilePublicKey::create_validated(&SignatureValidator::Services(vec![services]), PROFILE_ID, data).unwrap()
}

fn messages() -> Vec<(SignedMessageBody, MessageSignature)> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for (message, content) in values("message").iter().zip(CONTENTS) {
        let body = SignedMessageBody {
            content: content.to_string(),
            time_stamp: message[1].parse().unwrap(),
            salt: message[2].parse().unwrap(),
            last_seen: LastSeenMessages { entries: seen.clone() },
        };
        let signature = MessageSignature(std::sync::Arc::new(hex(message[4]).try_into().unwrap()));
        seen.push(signature.clone());
        out.push((body, signature));
    }
    out
}

#[test]
fn chat_keys_are_checked_against_the_services_key() {
    let services = RsaPublicKey::from_public_key_der(&hex(values("services_key")[0][0])).unwrap();
    let bytes = hex(values("key_data")[0][0]);
    let data = profile_public_key::Data::read(&mut Reader::new(&bytes)).unwrap();
    let mut written = Writer::new();
    data.write(&mut written);
    assert_eq!(written.as_slice(), &bytes[..]);
    let validator = SignatureValidator::Services(vec![services]);
    assert!(ProfilePublicKey::create_validated(&validator, PROFILE_ID, data.clone()).is_ok());
    assert!(ProfilePublicKey::create_validated(&validator, PROFILE_ID + 1, data).is_err());
}

#[test]
fn signed_messages_chain_as_in_vanilla() {
    let mut decoder = Decoder::chain(PROFILE_ID, SESSION_ID, key());
    let mut cache = MessageSignatureCache::default();
    let checksums = values("checksum");
    let packed = values("packed");
    for (i, (body, signature)) in messages().into_iter().enumerate() {
        assert_eq!(body.last_seen.compute_checksum().to_string(), checksums[i][0]);
        let mut w = Writer::new();
        body.write_packed(&mut w, &cache);
        assert_eq!(w.as_slice(), &hex(packed[i][0])[..], "packed body {i}");
        cache.push(&body, Some(&signature));
        let message = decoder.unpack(Some(signature), body, true).unwrap_or_else(|e| panic!("message {i}: {}", e.0.get_string()));
        assert_eq!(message.link.index, i as i32);
    }
}

#[test]
fn broken_chains_stay_broken() {
    let messages = messages();
    let error = |decoder: &mut Decoder, (body, signature): (SignedMessageBody, MessageSignature)| decoder.unpack(Some(signature), body, true).err().map(|e| e.0.get_string());

    let mut decoder = Decoder::chain(PROFILE_ID, SESSION_ID, key());
    let (mut body, signature) = messages[0].clone();
    body.content.push('!');
    assert_eq!(error(&mut decoder, (body, signature)), Some("Chat had an invalid signature. Please try reconnecting.".to_string()));
    assert_eq!(error(&mut decoder, messages[0].clone()), Some("Chat disabled due to broken chain. Please try reconnecting.".to_string()));

    let mut decoder = Decoder::chain(PROFILE_ID, SESSION_ID, key());
    assert!(error(&mut decoder, messages[1].clone()).is_some());

    let mut decoder = Decoder::chain(PROFILE_ID, SESSION_ID, key());
    assert_eq!(error(&mut decoder, messages[0].clone()), None);
    let (mut body, signature) = messages[1].clone();
    body.time_stamp = 0;
    assert_eq!(error(&mut decoder, (body, signature)), Some("Chat received out-of-order. Did your system time change?".to_string()));
}
