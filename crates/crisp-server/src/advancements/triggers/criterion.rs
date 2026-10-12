use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::util::codec::{self, DataResult};

pub struct Criterion {
    pub trigger: CriterionTrigger,
    pub trigger_instance: CriterionTriggerInstance,
}

impl Criterion {
    pub fn decode(tag: &Nbt) -> DataResult<Criterion> {
        codec::map(tag)?;
        let Some(type_name) = tag.get("trigger") else {
            return Err(format!("Missing \"trigger\" in: MapLike[{}]", codec::to_json(tag)));
        };
        let name = codec::identifier(type_name)?;
        let trigger = CriterionTrigger::by_name(&name)
            .ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:trigger_type]: {name}"))?;
        let empty = Nbt::Compound(Vec::new());
        let value = tag.get("conditions").unwrap_or(&empty);
        Ok(Criterion { trigger, trigger_instance: trigger.decode_instance(value)? })
    }
}
