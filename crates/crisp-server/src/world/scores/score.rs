use crisp_protocol::nbt::Nbt;

use super::objective::ObjectiveId;
use crate::network::chat::Component;
use crate::network::chat::numbers::NumberFormat;

#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub value: i32,
    pub locked: bool,
    pub display: Option<Component>,
    pub number_format: Option<NumberFormat>,
}

impl Default for Score {
    fn default() -> Score {
        Score { value: 0, locked: true, display: None, number_format: None }
    }
}

impl Score {
    pub fn format_value(&self, default: &NumberFormat) -> Component {
        self.number_format.as_ref().unwrap_or(default).format(self.value)
    }

    pub fn pack(&self, fields: &mut Vec<(String, Nbt)>) {
        if self.value != 0 {
            fields.push(("Score".into(), Nbt::Int(self.value)));
        }
        if self.locked {
            fields.push(("Locked".into(), Nbt::Byte(1)));
        }
        if let Some(display) = &self.display {
            fields.push(("display".into(), display.to_nbt()));
        }
        if let Some(format) = &self.number_format {
            fields.push(("format".into(), format.to_nbt()));
        }
    }

    pub fn unpack(tag: &Nbt) -> Result<Score, String> {
        use crate::network::chat::component_serialization::decode;
        Ok(Score {
            value: tag.get("Score").and_then(Nbt::as_i64).map_or(0, |v| v as i32),
            locked: tag.get("Locked").and_then(Nbt::as_f64).is_some_and(|v| v != 0.0),
            display: tag.get("display").map(decode).transpose()?,
            number_format: tag.get("format").map(NumberFormat::from_nbt).transpose()?,
        })
    }
}

#[derive(Default, Debug)]
pub struct PlayerScores {
    scores: Vec<(ObjectiveId, Score)>,
}

impl PlayerScores {
    pub fn get(&self, objective: ObjectiveId) -> Option<&Score> {
        self.scores.iter().find(|s| s.0 == objective).map(|s| &s.1)
    }

    pub fn get_mut(&mut self, objective: ObjectiveId) -> Option<&mut Score> {
        self.scores.iter_mut().find(|s| s.0 == objective).map(|s| &mut s.1)
    }

    pub fn get_or_create(&mut self, objective: ObjectiveId) -> (&mut Score, bool) {
        match self.scores.iter().position(|s| s.0 == objective) {
            Some(i) => (&mut self.scores[i].1, false),
            None => {
                self.scores.push((objective, Score::default()));
                (&mut self.scores.last_mut().unwrap().1, true)
            }
        }
    }

    pub fn remove(&mut self, objective: ObjectiveId) -> bool {
        match self.scores.iter().position(|s| s.0 == objective) {
            Some(i) => {
                self.scores.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn has_scores(&self) -> bool {
        !self.scores.is_empty()
    }

    pub fn set_score(&mut self, objective: ObjectiveId, score: Score) {
        match self.get_mut(objective) {
            Some(existing) => *existing = score,
            None => self.scores.push((objective, score)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(ObjectiveId, Score)> {
        self.scores.iter()
    }
}
