use super::Component;
use super::message_signature::MessageSignature;
use super::player_chat_message::PlayerChatMessage;
use super::signed_message_body::SignedMessageBody;
use super::signed_message_link::SignedMessageLink;
use crate::world::entity::player::profile_public_key::{ProfilePublicKey, now_millis};

pub struct DecodeException(pub Component);

const MISSING_PROFILE_KEY: &str = "chat.disabled.missingProfileKey";
const CHAIN_BROKEN: &str = "chat.disabled.chain_broken";
const EXPIRED_PROFILE_KEY: &str = "chat.disabled.expiredProfileKey";
const INVALID_SIGNATURE: &str = "chat.disabled.invalid_signature";
const OUT_OF_ORDER_CHAT: &str = "chat.disabled.out_of_order_chat";

fn error(key: &str) -> DecodeException {
    DecodeException(Component::translatable(key))
}

pub enum Decoder {
    Unsigned { profile_id: u128 },
    Chain {
        profile_public_key: ProfilePublicKey,
        next_link: Option<SignedMessageLink>,
        last_time_stamp: i64,
    },
}

impl Decoder {
    pub fn chain(profile_id: u128, session_id: u128, profile_public_key: ProfilePublicKey) -> Decoder {
        Decoder::Chain { profile_public_key, next_link: Some(SignedMessageLink::root(profile_id, session_id)), last_time_stamp: 0 }
    }

    pub fn unpack(&mut self, signature: Option<MessageSignature>, body: SignedMessageBody, enforces_secure_chat: bool) -> Result<PlayerChatMessage, DecodeException> {
        match self {
            Decoder::Unsigned { profile_id } => {
                if enforces_secure_chat {
                    return Err(error(MISSING_PROFILE_KEY));
                }
                Ok(PlayerChatMessage::unsigned(*profile_id, &body.content))
            }
            Decoder::Chain { profile_public_key, next_link, last_time_stamp } => {
                let Some(signature) = signature else { return Err(error(MISSING_PROFILE_KEY)) };
                if profile_public_key.data.has_expired() {
                    return Err(error(EXPIRED_PROFILE_KEY));
                }
                let Some(link) = *next_link else { return Err(error(CHAIN_BROKEN)) };
                if body.time_stamp < *last_time_stamp {
                    *next_link = None;
                    return Err(error(OUT_OF_ORDER_CHAT));
                }
                *last_time_stamp = body.time_stamp;
                let unpacked = PlayerChatMessage { link, signature: Some(signature), signed_body: body, unsigned_content: None };
                if !unpacked.verify(&profile_public_key.create_signature_validator()) {
                    *next_link = None;
                    return Err(error(INVALID_SIGNATURE));
                }
                if unpacked.has_expired_server(now_millis()) {
                    warn!("Received expired chat: '{}'. Is the client/server system time unsynchronized?", unpacked.signed_content());
                }
                *next_link = link.advance();
                Ok(unpacked)
            }
        }
    }

    pub fn set_chain_broken(&mut self) {
        if let Decoder::Chain { next_link, .. } = self {
            *next_link = None;
        }
    }
}
