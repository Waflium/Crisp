use super::Component;
use super::message_signature::MessageSignature;
use super::signed_message_body::SignedMessageBody;
use super::signed_message_link::{NIL_UUID, SignedMessageLink};
use crate::util::signature_validator::SignatureValidator;

const MESSAGE_EXPIRES_AFTER_SERVER: i64 = 5 * 60 * 1000;

#[derive(Clone, Debug)]
pub struct PlayerChatMessage {
    pub link: SignedMessageLink,
    pub signature: Option<MessageSignature>,
    pub signed_body: SignedMessageBody,
    pub unsigned_content: Option<Component>,
}

impl PlayerChatMessage {
    pub fn system(content: &str) -> PlayerChatMessage {
        PlayerChatMessage::unsigned(NIL_UUID, content)
    }

    pub fn unsigned(sender: u128, content: &str) -> PlayerChatMessage {
        PlayerChatMessage { link: SignedMessageLink::unsigned(sender), signature: None, signed_body: SignedMessageBody::unsigned(content), unsigned_content: None }
    }

    pub fn with_unsigned_content(&self, content: Component) -> PlayerChatMessage {
        let unsigned_content = (content != Component::literal(self.signed_content())).then_some(content);
        PlayerChatMessage { unsigned_content, ..self.clone() }
    }

    pub fn update_signature(output: &mut Vec<u8>, link: &SignedMessageLink, body: &SignedMessageBody) {
        output.extend_from_slice(&1i32.to_be_bytes());
        link.update_signature(output);
        body.update_signature(output);
    }

    pub fn verify(&self, validator: &SignatureValidator) -> bool {
        let Some(signature) = &self.signature else { return false };
        let mut payload = Vec::new();
        PlayerChatMessage::update_signature(&mut payload, &self.link, &self.signed_body);
        validator.validate(&payload, &signature.0[..])
    }

    pub fn signed_content(&self) -> &str {
        &self.signed_body.content
    }

    pub fn decorated_content(&self) -> Component {
        self.unsigned_content.clone().unwrap_or_else(|| Component::literal(self.signed_content()))
    }

    pub fn has_expired_server(&self, now: i64) -> bool {
        now > self.signed_body.time_stamp + MESSAGE_EXPIRES_AFTER_SERVER
    }

    pub fn sender(&self) -> u128 {
        self.link.sender
    }

    pub fn is_system(&self) -> bool {
        self.sender() == NIL_UUID
    }

    pub fn has_signature(&self) -> bool {
        self.signature.is_some()
    }
}
