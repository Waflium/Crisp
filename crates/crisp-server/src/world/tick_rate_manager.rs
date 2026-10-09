pub const MIN_TICKRATE: f32 = 1.0;
const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;

pub struct TickRateManager {
    tickrate: f32,
    nanoseconds_per_tick: i64,
    pub frozen_ticks_to_run: i32,
    run_game_elements: bool,
    is_frozen: bool,
}

impl Default for TickRateManager {
    fn default() -> TickRateManager {
        TickRateManager { tickrate: 20.0, nanoseconds_per_tick: NANOSECONDS_PER_SECOND / 20, frozen_ticks_to_run: 0, run_game_elements: true, is_frozen: false }
    }
}

impl TickRateManager {
    pub fn set_tick_rate(&mut self, rate: f32) {
        self.tickrate = rate.max(MIN_TICKRATE);
        self.nanoseconds_per_tick = (NANOSECONDS_PER_SECOND as f64 / self.tickrate as f64) as i64;
    }

    pub fn tickrate(&self) -> f32 {
        self.tickrate
    }

    pub fn milliseconds_per_tick(&self) -> f32 {
        self.nanoseconds_per_tick as f32 / 1_000_000.0
    }

    pub fn nanoseconds_per_tick(&self) -> i64 {
        self.nanoseconds_per_tick
    }

    pub fn runs_normally(&self) -> bool {
        self.run_game_elements
    }

    pub fn is_stepping_forward(&self) -> bool {
        self.frozen_ticks_to_run > 0
    }

    pub fn set_frozen(&mut self, frozen: bool) {
        self.is_frozen = frozen;
    }

    pub fn is_frozen(&self) -> bool {
        self.is_frozen
    }

    pub fn tick(&mut self) {
        self.run_game_elements = !self.is_frozen || self.frozen_ticks_to_run > 0;
        if self.frozen_ticks_to_run > 0 {
            self.frozen_ticks_to_run -= 1;
        }
    }

    pub fn is_entity_frozen(&self) -> bool {
        !self.runs_normally()
    }
}
