use chrono::{DateTime, Local, Utc};
use crisp_protocol::Writer;

#[derive(Clone, Copy, Debug, Default)]
pub struct CriterionProgress {
    obtained: Option<DateTime<Utc>>,
}

impl CriterionProgress {
    pub fn new(obtained: Option<DateTime<Utc>>) -> CriterionProgress {
        CriterionProgress { obtained }
    }

    pub fn write(&self, output: &mut Writer) {
        output.bool(self.obtained.is_some());
        if let Some(obtained) = self.obtained {
            output.i64(obtained.timestamp_millis());
        }
    }

    pub fn is_done(&self) -> bool {
        self.obtained.is_some()
    }

    pub fn grant(&mut self) {
        self.obtained = Some(Utc::now());
    }

    pub fn revoke(&mut self) {
        self.obtained = None;
    }

    pub fn get_obtained(&self) -> Option<DateTime<Utc>> {
        self.obtained
    }
}

pub fn format_obtained_time(obtained: DateTime<Utc>) -> String {
    obtained.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S %z").to_string()
}

pub fn parse_obtained_time(text: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S %z")
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| format!("Text '{text}' could not be parsed at index 0"))
}
