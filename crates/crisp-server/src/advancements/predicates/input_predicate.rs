use crisp_protocol::nbt::Nbt;

use crate::server::network::movement::Input;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct InputPredicate {
    forward: Option<bool>,
    backward: Option<bool>,
    left: Option<bool>,
    right: Option<bool>,
    jump: Option<bool>,
    sneak: Option<bool>,
    sprint: Option<bool>,
}

impl InputPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<InputPredicate> {
        codec::map(tag)?;
        let flag = |key| codec::decode_optional(tag, key, codec::bool_value);
        Ok(InputPredicate {
            forward: flag("forward")?,
            backward: flag("backward")?,
            left: flag("left")?,
            right: flag("right")?,
            jump: flag("jump")?,
            sneak: flag("sneak")?,
            sprint: flag("sprint")?,
        })
    }

    pub fn matches(&self, input: Input) -> bool {
        let matches = |expected: Option<bool>, value: bool| expected.is_none_or(|b| b == value);
        matches(self.forward, input.forward)
            && matches(self.backward, input.backward)
            && matches(self.left, input.left)
            && matches(self.right, input.right)
            && matches(self.jump, input.jump)
            && matches(self.sneak, input.shift)
            && matches(self.sprint, input.sprint)
    }
}
