use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use crisp_core::random::java_string_hash;
use crisp_protocol::Writer;
use serde_json::{Map, Value};

use super::advancement_requirements::AdvancementRequirements;
use super::criterion_progress::{self, CriterionProgress};
use crate::network::chat::Component;
use crate::util::java_collections::JavaHashMap;

static EMPTY_REQUIREMENTS: AdvancementRequirements = AdvancementRequirements::EMPTY;

#[derive(Clone, Debug)]
pub struct AdvancementProgress {
    criteria: JavaHashMap<CriterionProgress>,
    requirements: &'static AdvancementRequirements,
}

impl Default for AdvancementProgress {
    fn default() -> AdvancementProgress {
        AdvancementProgress { criteria: JavaHashMap::new(java_string_hash), requirements: &EMPTY_REQUIREMENTS }
    }
}

impl AdvancementProgress {
    pub fn decode(value: &Value) -> Result<AdvancementProgress, String> {
        let Value::Object(fields) = value else { return Err(format!("Not a JSON object: {value}")) };
        let mut criteria = JavaHashMap::new(java_string_hash);
        if let Some(map) = fields.get("criteria") {
            let Value::Object(map) = map else { return Err(format!("Not a JSON object: {map}")) };
            for (name, obtained) in map {
                let Value::String(obtained) = obtained else { return Err(format!("Not a string: {obtained}")) };
                criteria.insert(name.clone(), CriterionProgress::new(Some(criterion_progress::parse_obtained_time(obtained)?)));
            }
        }
        let criteria = JavaHashMap::copy_of(java_string_hash, criteria.iter().map(|(name, progress)| (name.to_string(), *progress)));
        Ok(AdvancementProgress { criteria, requirements: &EMPTY_REQUIREMENTS })
    }

    pub fn encode(&self) -> Value {
        let mut fields = Map::new();
        let done: Vec<(&str, DateTime<Utc>)> = self.criteria.iter().filter_map(|(name, progress)| progress.get_obtained().map(|obtained| (name, obtained))).collect();
        let mut criteria = JavaHashMap::new(java_string_hash);
        for (name, obtained) in done {
            criteria.insert(name.to_string(), obtained);
        }
        if !criteria.is_empty() {
            fields.insert(
                "criteria".into(),
                Value::Object(criteria.iter().map(|(name, &obtained)| (name.to_string(), Value::String(criterion_progress::format_obtained_time(obtained)))).collect()),
            );
        }
        fields.insert("done".into(), Value::Bool(self.is_done()));
        Value::Object(fields)
    }

    pub fn write(&self, output: &mut Writer) {
        output.varint(self.criteria.len() as i32);
        for (name, progress) in self.criteria.iter() {
            output.string(name);
            progress.write(output);
        }
    }

    pub fn update(&mut self, requirements: &'static AdvancementRequirements) {
        let names = requirements.names();
        let stale: Vec<String> = self.criteria.iter().map(|(name, _)| name).filter(|name| !names.contains(name)).map(str::to_string).collect();
        for name in stale {
            self.criteria.remove(&name);
        }
        for name in names {
            if !self.criteria.contains_key(name) {
                self.criteria.insert(name.to_string(), CriterionProgress::default());
            }
        }
        self.requirements = requirements;
    }

    pub fn is_done(&self) -> bool {
        self.requirements.test(|criterion| self.is_criterion_done(criterion))
    }

    pub fn has_progress(&self) -> bool {
        self.criteria.values().any(CriterionProgress::is_done)
    }

    pub fn grant_progress(&mut self, name: &str) -> bool {
        match self.criteria.get_mut(name) {
            Some(progress) if !progress.is_done() => {
                progress.grant();
                true
            }
            _ => false,
        }
    }

    pub fn revoke_progress(&mut self, name: &str) -> bool {
        match self.criteria.get_mut(name) {
            Some(progress) if progress.is_done() => {
                progress.revoke();
                true
            }
            _ => false,
        }
    }

    pub fn get_criterion(&self, id: &str) -> Option<&CriterionProgress> {
        self.criteria.get(id)
    }

    fn is_criterion_done(&self, criterion: &str) -> bool {
        self.get_criterion(criterion).is_some_and(CriterionProgress::is_done)
    }

    pub fn get_percent(&self) -> f32 {
        if self.criteria.is_empty() {
            return 0.0;
        }
        self.count_completed_requirements() as f32 / self.requirements.size() as f32
    }

    pub fn get_progress_text(&self) -> Option<Component> {
        if self.criteria.is_empty() {
            return None;
        }
        let total = self.requirements.size() as i32;
        if total <= 1 {
            return None;
        }
        Some(Component::translatable_with("advancements.progress", vec![(self.count_completed_requirements() as i32).into(), total.into()]))
    }

    fn count_completed_requirements(&self) -> usize {
        self.requirements.count(|criterion| self.is_criterion_done(criterion))
    }

    pub fn get_remaining_criteria(&self) -> Vec<String> {
        self.criteria.iter().filter(|(_, progress)| !progress.is_done()).map(|(name, _)| name.to_string()).collect()
    }

    pub fn get_completed_criteria(&self) -> Vec<String> {
        self.criteria.iter().filter(|(_, progress)| progress.is_done()).map(|(name, _)| name.to_string()).collect()
    }

    pub fn get_first_progress_date(&self) -> Option<DateTime<Utc>> {
        self.criteria.values().filter_map(CriterionProgress::get_obtained).min()
    }

    pub fn compare_to(&self, o: &AdvancementProgress) -> Ordering {
        match (self.get_first_progress_date(), o.get_first_progress_date()) {
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (None, None) => Ordering::Equal,
            (Some(ours), Some(theirs)) => ours.cmp(&theirs),
        }
    }
}
