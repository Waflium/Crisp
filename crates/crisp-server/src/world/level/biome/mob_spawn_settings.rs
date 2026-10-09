use serde_json::Value;

use crate::world::level::levelgen::density_function::resource_name;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MobCategory {
    Monster,
    Creature,
    Ambient,
    Axolotls,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}

impl MobCategory {
    pub const VALUES: [MobCategory; 8] = [
        MobCategory::Monster,
        MobCategory::Creature,
        MobCategory::Ambient,
        MobCategory::Axolotls,
        MobCategory::UndergroundWaterCreature,
        MobCategory::WaterCreature,
        MobCategory::WaterAmbient,
        MobCategory::Misc,
    ];

    pub fn name(self) -> &'static str {
        ["monster", "creature", "ambient", "axolotls", "underground_water_creature", "water_creature", "water_ambient", "misc"][self as usize]
    }

    pub fn by_name(name: &str) -> Option<MobCategory> {
        Self::VALUES.into_iter().find(|c| c.name() == name)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SpawnerData {
    pub entity_type: u16,
    pub count: Value,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MobSpawnCost {
    pub energy_budget: f64,
    pub charge: f64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct MobSpawnSettings {
    pub spawns_by_category: [Option<Vec<(SpawnerData, i32)>>; 8],
    pub mob_spawn_costs: Vec<(u16, MobSpawnCost)>,
}

fn entity_type(json: &Value) -> Result<u16, String> {
    let name = resource_name(json.as_str().ok_or_else(|| format!("not an entity type: {json}"))?);
    crisp_data::registry::ENTITY_TYPE.iter().position(|&e| e == name).map(|i| i as u16).ok_or_else(|| format!("unknown entity type {name}"))
}

impl MobSpawnSettings {
    pub fn mobs_in_category(&self, category: MobCategory) -> Option<&[(SpawnerData, i32)]> {
        self.spawns_by_category[category as usize].as_deref()
    }

    pub fn mobs_to_spawn(&self, category: MobCategory) -> &[(SpawnerData, i32)] {
        self.mobs_in_category(category).unwrap_or(&[])
    }

    pub fn mob_spawn_cost(&self, entity_type: u16) -> Option<MobSpawnCost> {
        self.mob_spawn_costs.iter().find(|(e, _)| *e == entity_type).map(|&(_, c)| c)
    }

    pub fn is_empty(&self) -> bool {
        self.spawns_by_category.iter().all(Option::is_none) && self.mob_spawn_costs.is_empty()
    }

    pub fn put_cost(&mut self, entity_type: u16, cost: MobSpawnCost) {
        match self.mob_spawn_costs.iter_mut().find(|(e, _)| *e == entity_type) {
            Some(entry) => entry.1 = cost,
            None => self.mob_spawn_costs.push((entity_type, cost)),
        }
    }

    pub fn parse(json: &Value) -> Result<MobSpawnSettings, String> {
        let mut settings = MobSpawnSettings::default();
        let spawns = json.get("spawns_by_category").and_then(Value::as_object).ok_or("missing spawns_by_category")?;
        for (name, list) in spawns {
            let category = MobCategory::by_name(name).ok_or_else(|| format!("unknown mob category {name}"))?;
            let mut entries = Vec::new();
            for entry in list.as_array().ok_or("spawns must be a list")? {
                let weight = entry.get("weight").and_then(Value::as_i64).filter(|w| *w >= 0).ok_or("spawn weight must be non-negative")? as i32;
                let data = SpawnerData { entity_type: entity_type(&entry["type"])?, count: entry.get("count").cloned().ok_or("missing spawn count")? };
                entries.push((data, weight));
            }
            settings.spawns_by_category[category as usize] = Some(entries);
        }
        let costs = json.get("spawn_costs").and_then(Value::as_object).ok_or("missing spawn_costs")?;
        for (name, cost) in costs {
            let field = |key: &str| cost.get(key).and_then(Value::as_f64).ok_or_else(|| format!("missing {key}"));
            settings.put_cost(entity_type(&Value::String(name.clone()))?, MobSpawnCost { energy_budget: field("energy_budget")?, charge: field("charge")? });
        }
        Ok(settings)
    }
}
