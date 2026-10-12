use crisp_protocol::Writer;

use super::advancement::Advancement;
use crate::util::java_collections::{JavaHashCode, identifier_hash};

pub struct AdvancementHolder {
    pub id: String,
    pub value: Advancement,
}

impl AdvancementHolder {
    pub fn write(&self, output: &mut Writer) {
        output.string(&self.id);
        self.value.write(output);
    }
}

impl PartialEq for AdvancementHolder {
    fn eq(&self, other: &AdvancementHolder) -> bool {
        std::ptr::eq(self, other) || self.id == other.id
    }
}

impl JavaHashCode for &AdvancementHolder {
    fn java_hash_code(&self) -> i32 {
        identifier_hash(&self.id)
    }
}

impl std::fmt::Display for AdvancementHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id)
    }
}
