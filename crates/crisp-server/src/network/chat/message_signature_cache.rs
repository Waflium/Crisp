use std::collections::{HashSet, VecDeque};

use super::message_signature::MessageSignature;
use super::signed_message_body::SignedMessageBody;

const DEFAULT_CAPACITY: usize = 128;

pub struct MessageSignatureCache {
    entries: Vec<Option<MessageSignature>>,
}

impl Default for MessageSignatureCache {
    fn default() -> MessageSignatureCache {
        MessageSignatureCache { entries: vec![None; DEFAULT_CAPACITY] }
    }
}

impl MessageSignatureCache {
    pub fn pack(&self, signature: &MessageSignature) -> i32 {
        self.entries.iter().position(|e| e.as_ref() == Some(signature)).map_or(-1, |i| i as i32)
    }

    pub fn push(&mut self, body: &SignedMessageBody, signature: Option<&MessageSignature>) {
        let mut queue: VecDeque<MessageSignature> = body.last_seen.entries.iter().cloned().collect();
        queue.extend(signature.cloned());
        self.push_queue(queue);
    }

    fn push_queue(&mut self, mut queue: VecDeque<MessageSignature>) {
        let new_entries: HashSet<MessageSignature> = queue.iter().cloned().collect();
        for slot in &mut self.entries {
            let Some(next) = queue.pop_back() else { break };
            if let Some(entry) = slot.replace(next)
                && !new_entries.contains(&entry)
            {
                queue.push_front(entry);
            }
        }
    }
}
