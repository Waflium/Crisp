use std::sync::LazyLock;

use crisp_core::mth;
use crisp_data::{SyncedRegistry, TIMELINES};
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::level::levelgen::density_function::resource_name;
use crate::world::timeline::{AttributeTrack, AttributeTrackSampler};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockInstance {
    pub total_ticks: i64,
    pub partial_tick: f32,
    pub rate: f32,
    pub paused: bool,
}

impl Default for ClockInstance {
    fn default() -> Self {
        ClockInstance { total_ticks: 0, partial_tick: 0.0, rate: 1.0, paused: false }
    }
}

impl ClockInstance {
    fn tick(&mut self) {
        if !self.paused {
            self.partial_tick += self.rate;
            let full = mth::floor_f(self.partial_tick);
            self.partial_tick -= full as f32;
            self.total_ticks += full as i64;
        }
    }

    fn to_nbt(self) -> Nbt {
        let mut fields = vec![("total_ticks".to_string(), Nbt::Long(self.total_ticks))];
        if self.partial_tick != 0.0 {
            fields.push(("partial_tick".into(), Nbt::Float(self.partial_tick)));
        }
        if self.rate != 1.0 {
            fields.push(("rate".into(), Nbt::Float(self.rate)));
        }
        if self.paused {
            fields.push(("paused".into(), Nbt::Byte(1)));
        }
        Nbt::Compound(fields)
    }

    fn from_nbt(nbt: &Nbt) -> ClockInstance {
        ClockInstance {
            total_ticks: nbt.get("total_ticks").and_then(Nbt::as_i64).unwrap_or(0),
            partial_tick: nbt.get("partial_tick").and_then(Nbt::as_f64).unwrap_or(0.0) as f32,
            rate: nbt.get("rate").and_then(Nbt::as_f64).unwrap_or(1.0) as f32,
            paused: nbt.get("paused").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
        }
    }
}

pub struct Timeline {
    pub id: &'static str,
    pub clock: usize,
    pub period_ticks: Option<i32>,
    pub time_markers: Vec<(String, i32, bool)>,
    pub tracks: Vec<(usize, AttributeTrack)>,
}

impl Timeline {
    pub fn current_ticks(&self, clocks: &ServerClockManager) -> i64 {
        let total = clocks.clocks[self.clock].total_ticks;
        match self.period_ticks {
            Some(period) => total % period as i64,
            None => total,
        }
    }

    pub fn period_count(&self, clocks: &ServerClockManager) -> i32 {
        match self.period_ticks {
            Some(period) => (clocks.clocks[self.clock].total_ticks / period as i64) as i32,
            None => 0,
        }
    }

    pub fn get(id: &str) -> Option<&'static Timeline> {
        TIMELINE_DATA.iter().find(|t| t.id == id)
    }

    pub fn attributes(&self) -> impl Iterator<Item = usize> + '_ {
        self.tracks.iter().map(|(id, _)| *id)
    }

    pub fn create_track_sampler(&self, attribute: &EnvironmentAttributeInfo) -> AttributeTrackSampler {
        let (_, track) = self.tracks.iter().find(|(id, _)| *id == attribute.id).unwrap_or_else(|| panic!("Timeline has no track for {}", attribute.name));
        track.bake_sampler(attribute, self.clock, self.period_ticks)
    }

    fn parse(id: &'static str, v: &serde_json::Value) -> Result<Timeline, String> {
        let clock = ServerClockManager::id_of(v["clock"].as_str().ok_or("missing clock")?).ok_or("unknown clock")?;
        let period_ticks = match v.get("period_ticks") {
            None => None,
            Some(p) => Some(p.as_i64().filter(|p| (1..=i32::MAX as i64).contains(p)).ok_or("Value must be positive")? as i32),
        };
        let time_markers: Vec<(String, i32, bool)> = v["time_markers"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(marker, info)| match info.as_i64() {
                Some(ticks) => (marker.clone(), ticks as i32, false),
                None => (marker.clone(), info["ticks"].as_i64().unwrap_or(0) as i32, info["show_in_commands"].as_bool().unwrap_or(false)),
            })
            .collect();
        let mut tracks = Vec::new();
        for (name, track) in v["tracks"].as_object().into_iter().flatten() {
            let attribute = EnvironmentAttributeInfo::by_name(&resource_name(name)).ok_or_else(|| format!("unknown environment attribute {name}"))?;
            tracks.push((attribute.id, AttributeTrack::parse(attribute, track)?));
        }
        if let Some(period) = period_ticks {
            if let Some((marker, ..)) = time_markers.iter().find(|m| m.1 < 0 || m.1 >= period) {
                return Err(format!("Time Marker {marker} must be in range [0; {period})"));
            }
            for (_, track) in &tracks {
                track.argument_track.validate_period(period)?;
            }
        }
        Ok(Timeline { id, clock, period_ticks, time_markers, tracks })
    }
}

pub static TIMELINE_DATA: LazyLock<Vec<Timeline>> = LazyLock::new(|| {
    TIMELINES
        .iter()
        .map(|&(id, json)| {
            let v: serde_json::Value = serde_json::from_str(json).expect("timeline JSON");
            Timeline::parse(id, &v).unwrap_or_else(|e| panic!("timeline {id}: {e}"))
        })
        .collect()
});

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockTimeMarker {
    pub ticks: i32,
    pub period_ticks: Option<i32>,
    pub show_in_commands: bool,
}

impl ClockTimeMarker {
    pub fn resolve_time_to_move_to(self, total_ticks: i64) -> i64 {
        match self.period_ticks {
            None => self.ticks as i64,
            Some(period) => {
                let duration = self.ticks as i64 - total_ticks % period as i64;
                total_ticks + if duration > 0 { duration } else { period as i64 + duration }
            }
        }
    }

    pub fn occurs_at(self, total_ticks: i64) -> bool {
        match self.period_ticks {
            None => self.ticks as i64 == total_ticks,
            Some(period) => self.ticks as i64 == total_ticks % period as i64,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveResult {
    NoTimeMarkerFound,
    NotMoved,
    Moved,
}

pub struct ServerClockManager {
    pub clocks: Vec<ClockInstance>,
}

impl ServerClockManager {
    fn registry() -> &'static SyncedRegistry {
        SyncedRegistry::get("minecraft:world_clock")
    }

    pub fn new() -> ServerClockManager {
        ServerClockManager { clocks: vec![ClockInstance::default(); Self::registry().entries.len()] }
    }

    pub fn id_of(name: &str) -> Option<usize> {
        Self::registry().id_of(name).map(|i| i as usize)
    }

    pub fn tick(&mut self, advance_time: bool) {
        if advance_time {
            self.clocks.iter_mut().for_each(ClockInstance::tick);
        }
    }

    pub fn write_set_time(&self, w: &mut Writer, game_time: i64, clocks: &[usize], advance_time: bool) {
        w.i64(game_time);
        w.varint(clocks.len() as i32);
        for &id in clocks {
            let c = self.clocks[id];
            w.varint(id as i32);
            w.varlong(c.total_ticks);
            w.f32(c.partial_tick);
            w.f32(if c.paused || !advance_time { 0.0 } else { c.rate });
        }
    }

    pub fn command_time_markers(clock: usize) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        for timeline in TIMELINE_DATA.iter().filter(|t| t.clock == clock) {
            for marker in &timeline.time_markers {
                if !names.contains(&marker.0.as_str()) {
                    names.push(&marker.0);
                }
            }
        }
        names.retain(|name| Self::time_marker(clock, name).is_some_and(|m| m.show_in_commands));
        names
    }

    pub fn time_marker(clock: usize, id: &str) -> Option<ClockTimeMarker> {
        TIMELINE_DATA
            .iter()
            .filter(|t| t.clock == clock)
            .flat_map(|t| t.time_markers.iter().map(move |m| (t, m)))
            .filter(|(_, m)| m.0 == id)
            .last()
            .map(|(t, m)| ClockTimeMarker { ticks: m.1, period_ticks: t.period_ticks, show_in_commands: m.2 })
    }

    pub fn set_total_ticks(&mut self, clock: usize, total_ticks: i64) {
        self.clocks[clock].total_ticks = total_ticks;
        self.clocks[clock].partial_tick = 0.0;
    }

    pub fn move_to_time_marker(&mut self, clock: usize, marker: &str) -> MoveResult {
        let instance = &mut self.clocks[clock];
        match Self::time_marker(clock, marker) {
            None => MoveResult::NoTimeMarkerFound,
            Some(m) if m.occurs_at(instance.total_ticks) => MoveResult::NotMoved,
            Some(m) => {
                instance.total_ticks = m.resolve_time_to_move_to(instance.total_ticks);
                instance.partial_tick = 0.0;
                MoveResult::Moved
            }
        }
    }

    pub fn add_ticks(&mut self, clock: usize, ticks: i32) {
        let instance = &mut self.clocks[clock];
        instance.total_ticks = (instance.total_ticks + ticks as i64).max(0);
    }

    pub fn all(&self) -> Vec<usize> {
        (0..self.clocks.len()).collect()
    }

    pub fn to_nbt(&self) -> Nbt {
        let names = Self::registry().entries;
        Nbt::Compound(self.clocks.iter().enumerate().map(|(i, c)| (names[i].to_string(), c.to_nbt())).collect())
    }

    pub fn load(&mut self, nbt: &Nbt) {
        if let Nbt::Compound(fields) = nbt {
            for (name, state) in fields {
                if let Some(id) = Self::id_of(name) {
                    self.clocks[id] = ClockInstance::from_nbt(state);
                }
            }
        }
    }
}

impl Default for ServerClockManager {
    fn default() -> Self {
        Self::new()
    }
}
