use std::sync::{Arc, LazyLock};

use crisp_protocol::Writer;

use super::stat_type::StatType;
use crate::world::scores::criteria::ObjectiveCriteria;

static NAMES: LazyLock<Vec<Vec<Arc<str>>>> = LazyLock::new(|| {
    StatType::VALUES.iter().map(|&stat_type| (0..stat_type.get_registry().len()).map(|value| Stat::build_name(stat_type, value as u16).into()).collect()).collect()
});

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stat {
    pub stat_type: StatType,
    pub value: u16,
}

impl Stat {
    pub fn build_name(stat_type: StatType, value: u16) -> String {
        format!("{}:{}", location_to_key(stat_type.key()), location_to_key(stat_type.get_registry()[value as usize]))
    }

    pub fn get_name(self) -> &'static str {
        &NAMES[self.stat_type as usize][self.value as usize]
    }

    pub fn criteria(self) -> ObjectiveCriteria {
        ObjectiveCriteria::Stat(NAMES[self.stat_type as usize][self.value as usize].clone())
    }

    pub fn value_key(self) -> &'static str {
        self.stat_type.get_registry()[self.value as usize]
    }

    pub fn write(self, w: &mut Writer) {
        w.varint(self.stat_type.id());
        w.varint(self.value as i32);
    }
}

fn location_to_key(location: &str) -> String {
    location.replace(':', ".")
}
