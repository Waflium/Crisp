use crisp_protocol::Writer;

use super::last_seen_messages::LastSeenMessages;
use super::message_signature_cache::MessageSignatureCache;
use crate::world::entity::player::profile_public_key::now_millis;

#[derive(Clone, Debug)]
pub struct SignedMessageBody {
    pub content: String,
    pub time_stamp: i64,
    pub salt: i64,
    pub last_seen: LastSeenMessages,
}

impl SignedMessageBody {
    pub fn unsigned(content: &str) -> SignedMessageBody {
        SignedMessageBody { content: content.to_string(), time_stamp: now_millis(), salt: 0, last_seen: LastSeenMessages::default() }
    }

    pub fn update_signature(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.salt.to_be_bytes());
        output.extend_from_slice(&self.time_stamp.div_euclid(1000).to_be_bytes());
        output.extend_from_slice(&(self.content.len() as i32).to_be_bytes());
        output.extend_from_slice(self.content.as_bytes());
        self.last_seen.update_signature(output);
    }

    pub fn write_packed(&self, w: &mut Writer, cache: &MessageSignatureCache) {
        w.string(&self.content);
        w.i64(self.time_stamp);
        w.i64(self.salt);
        w.varint(self.last_seen.entries.len() as i32);
        for entry in &self.last_seen.entries {
            entry.pack(cache).write(w);
        }
    }
}
