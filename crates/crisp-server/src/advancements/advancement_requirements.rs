use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AdvancementRequirements {
    pub requirements: Vec<Vec<String>>,
}

impl AdvancementRequirements {
    pub const EMPTY: AdvancementRequirements = AdvancementRequirements { requirements: Vec::new() };

    pub fn decode(tag: &Nbt) -> DataResult<AdvancementRequirements> {
        Ok(AdvancementRequirements { requirements: codec::list_of(tag, |set| codec::list_of(set, |name| codec::string(name).map(str::to_string)))? })
    }

    pub fn write(&self, output: &mut Writer) {
        output.varint(self.requirements.len() as i32);
        for set in &self.requirements {
            output.varint(set.len() as i32);
            for criterion in set {
                output.string(criterion);
            }
        }
    }

    pub fn all_of<'a>(criteria: impl Iterator<Item = &'a str>) -> AdvancementRequirements {
        AdvancementRequirements { requirements: criteria.map(|criterion| vec![criterion.to_string()]).collect() }
    }

    pub fn any_of<'a>(criteria: impl Iterator<Item = &'a str>) -> AdvancementRequirements {
        AdvancementRequirements { requirements: vec![criteria.map(str::to_string).collect()] }
    }

    pub fn size(&self) -> usize {
        self.requirements.len()
    }

    pub fn test(&self, predicate: impl Fn(&str) -> bool) -> bool {
        if self.requirements.is_empty() {
            return false;
        }
        self.requirements.iter().all(|set| any_match(set, &predicate))
    }

    pub fn count(&self, predicate: impl Fn(&str) -> bool) -> usize {
        self.requirements.iter().filter(|set| any_match(set, &predicate)).count()
    }

    pub fn validate(&self, expected_criteria: &[&str]) -> DataResult<()> {
        let mut referenced_criteria: Vec<&str> = Vec::new();
        for set in &self.requirements {
            if set.is_empty() && expected_criteria.is_empty() {
                return Err("Requirement entry cannot be empty".into());
            }
            for criterion in set {
                if !referenced_criteria.contains(&criterion.as_str()) {
                    referenced_criteria.push(criterion);
                }
            }
        }
        let missing_criteria: Vec<&str> = expected_criteria.iter().copied().filter(|c| !referenced_criteria.contains(c)).collect();
        let unknown_criteria: Vec<&str> = referenced_criteria.iter().copied().filter(|c| !expected_criteria.contains(c)).collect();
        if missing_criteria.is_empty() && unknown_criteria.is_empty() {
            return Ok(());
        }
        Err(format!(
            "Advancement completion requirements did not exactly match specified criteria. Missing: [{}]. Unknown: [{}]",
            missing_criteria.join(", "),
            unknown_criteria.join(", ")
        ))
    }

    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Object2ObjectOpenHashMap<()> = Object2ObjectOpenHashMap::new(16, 0.75);
        for set in &self.requirements {
            names.try_capacity(names.len() + set.len());
            for name in set {
                if !names.contains_key(name) {
                    names.insert(name.clone(), ());
                }
            }
        }
        let mut ordered = Vec::with_capacity(names.len());
        for (name, ()) in names.iter() {
            if let Some(name) = self.requirements.iter().flatten().find(|n| n.as_str() == name) {
                ordered.push(name.as_str());
            }
        }
        ordered
    }
}

fn any_match(criteria: &[String], predicate: &impl Fn(&str) -> bool) -> bool {
    criteria.iter().any(|criterion| predicate(criterion))
}
