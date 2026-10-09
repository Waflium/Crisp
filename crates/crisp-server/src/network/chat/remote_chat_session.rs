use crisp_protocol::{Reader, Writer};

use super::Component;
use crate::world::entity::player::profile_public_key::{self, ProfilePublicKey};
use crate::util::signature_validator::SignatureValidator;

#[derive(Clone, Debug)]
pub struct RemoteChatSession {
    pub session_id: u128,
    pub profile_public_key: ProfilePublicKey,
}

pub struct Data {
    pub session_id: u128,
    pub profile_public_key: profile_public_key::Data,
}

impl Data {
    pub fn read(r: &mut Reader) -> anyhow::Result<Data> {
        Ok(Data { session_id: r.uuid()?, profile_public_key: profile_public_key::Data::read(r)? })
    }

    pub fn validate(self, profile_id: u128, validator: &SignatureValidator) -> Result<RemoteChatSession, Component> {
        Ok(RemoteChatSession { session_id: self.session_id, profile_public_key: ProfilePublicKey::create_validated(validator, profile_id, self.profile_public_key)? })
    }
}

impl RemoteChatSession {
    pub fn has_expired(&self) -> bool {
        self.profile_public_key.data.has_expired()
    }

    pub fn write_data(&self, w: &mut Writer) {
        w.uuid(self.session_id);
        self.profile_public_key.data.write(w);
    }
}
