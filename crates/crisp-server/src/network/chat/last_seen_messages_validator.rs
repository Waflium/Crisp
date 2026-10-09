use std::collections::VecDeque;

use super::last_seen_messages::{LastSeenMessages, Update};
use super::message_signature::MessageSignature;

#[derive(Clone)]
struct TrackedEntry {
    signature: MessageSignature,
    pending: bool,
}

pub struct LastSeenMessagesValidator {
    last_seen_count: usize,
    tracked_messages: VecDeque<Option<TrackedEntry>>,
    last_pending_message: Option<MessageSignature>,
}

#[derive(Debug)]
pub struct ValidationException(pub String);

impl LastSeenMessagesValidator {
    pub fn new(last_seen_count: usize) -> LastSeenMessagesValidator {
        LastSeenMessagesValidator { last_seen_count, tracked_messages: (0..last_seen_count).map(|_| None).collect(), last_pending_message: None }
    }

    pub fn add_pending(&mut self, message: &MessageSignature) {
        if self.last_pending_message.as_ref() != Some(message) {
            self.tracked_messages.push_back(Some(TrackedEntry { signature: message.clone(), pending: true }));
            self.last_pending_message = Some(message.clone());
        }
    }

    pub fn tracked_messages_count(&self) -> usize {
        self.tracked_messages.len()
    }

    pub fn apply_offset(&mut self, offset: i32) -> Result<(), ValidationException> {
        let max_offset = self.tracked_messages.len() as i32 - self.last_seen_count as i32;
        if offset < 0 || offset > max_offset {
            return Err(ValidationException(format!("Advanced last seen window by {offset} messages, but expected at most {max_offset}")));
        }
        self.tracked_messages.drain(..offset as usize);
        Ok(())
    }

    pub fn apply_update(&mut self, update: &Update) -> Result<LastSeenMessages, ValidationException> {
        self.apply_offset(update.offset)?;
        let length = 32 - update.acknowledged.leading_zeros() as usize;
        if length > self.last_seen_count {
            return Err(ValidationException(format!(
                "Last seen update contained {length} messages, but maximum window size is {}",
                self.last_seen_count
            )));
        }
        let mut entries = Vec::with_capacity(update.acknowledged.count_ones() as usize);
        for i in 0..self.last_seen_count {
            let message = &mut self.tracked_messages[i];
            if update.acknowledged & 1 << i != 0 {
                let Some(entry) = message else {
                    return Err(ValidationException(format!("Last seen update acknowledged unknown or previously ignored message at index {i}")));
                };
                entry.pending = false;
                entries.push(entry.signature.clone());
            } else {
                if let Some(entry) = message
                    && !entry.pending
                {
                    return Err(ValidationException(format!(
                        "Last seen update ignored previously acknowledged message at index {i} and signature {:?}",
                        entry.signature
                    )));
                }
                *message = None;
            }
        }
        let last_seen = LastSeenMessages { entries };
        if !update.verify_checksum(&last_seen) {
            return Err(ValidationException("Checksum mismatch on last seen update: the client and server must have desynced".into()));
        }
        Ok(last_seen)
    }
}
