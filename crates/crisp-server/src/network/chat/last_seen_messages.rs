use crisp_protocol::Reader;

use super::message_signature::MessageSignature;

pub const LAST_SEEN_MESSAGES_MAX_LENGTH: usize = 20;

#[derive(Clone, Debug, Default)]
pub struct LastSeenMessages {
    pub entries: Vec<MessageSignature>,
}

impl LastSeenMessages {
    pub fn update_signature(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&(self.entries.len() as i32).to_be_bytes());
        for entry in &self.entries {
            output.extend_from_slice(&entry.0[..]);
        }
    }

    pub fn compute_checksum(&self) -> u8 {
        let checksum = self.entries.iter().fold(1i32, |c, e| c.wrapping_mul(31).wrapping_add(e.checksum()));
        match checksum as u8 {
            0 => 1,
            byte => byte,
        }
    }
}

pub struct Update {
    pub offset: i32,
    pub acknowledged: u32,
    pub checksum: u8,
}

impl Update {
    pub fn read(r: &mut Reader) -> anyhow::Result<Update> {
        let offset = r.varint()?;
        let bytes = r.take(LAST_SEEN_MESSAGES_MAX_LENGTH.div_ceil(8))?;
        let acknowledged = bytes.iter().enumerate().fold(0u32, |bits, (i, &b)| bits | (b as u32) << (8 * i));
        Ok(Update { offset, acknowledged, checksum: r.u8()? })
    }

    pub fn verify_checksum(&self, last_seen: &LastSeenMessages) -> bool {
        self.checksum == 0 || self.checksum == last_seen.compute_checksum()
    }
}
