pub const NIL_UUID: u128 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SignedMessageLink {
    pub index: i32,
    pub sender: u128,
    pub session_id: u128,
}

impl SignedMessageLink {
    pub fn unsigned(sender: u128) -> SignedMessageLink {
        SignedMessageLink::root(sender, NIL_UUID)
    }

    pub fn root(sender: u128, session_id: u128) -> SignedMessageLink {
        SignedMessageLink { index: 0, sender, session_id }
    }

    pub fn update_signature(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.sender.to_be_bytes());
        output.extend_from_slice(&self.session_id.to_be_bytes());
        output.extend_from_slice(&self.index.to_be_bytes());
    }

    pub fn advance(&self) -> Option<SignedMessageLink> {
        (self.index != i32::MAX).then_some(SignedMessageLink { index: self.index + 1, ..*self })
    }
}
