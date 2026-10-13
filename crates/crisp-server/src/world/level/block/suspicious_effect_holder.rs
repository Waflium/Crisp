use crisp_data::Item;
use crisp_protocol::nbt::Nbt;
use serde_json::Value;

pub fn try_get(item: Item) -> Option<Nbt> {
    let block = item.info().block.filter(|block| block.is_instance_of("FlowerBlock"))?;
    let fields: Value = serde_json::from_str(block.info().fields).unwrap_or_default();
    let effects = fields["suspiciousStewEffects"]["effects"].as_array()?;
    Some(Nbt::List(
        effects
            .iter()
            .map(|entry| {
                let mut compound = vec![("id".to_string(), Nbt::String(entry["effect"].as_str().unwrap_or_default().to_string()))];
                let duration = entry["duration"].as_i64().unwrap_or(160) as i32;
                if duration != 160 {
                    compound.push(("duration".to_string(), Nbt::Int(duration)));
                }
                Nbt::Compound(compound)
            })
            .collect(),
    ))
}
