use std::cell::Cell;

pub const MIN_TICKRATE: f32 = 1.0;
const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;

pub struct TickRateManager {
    tickrate: Cell<f32>,
    nanoseconds_per_tick: Cell<i64>,
    frozen_ticks_to_run: Cell<i32>,
    run_game_elements: Cell<bool>,
    is_frozen: Cell<bool>,
}

impl Default for TickRateManager {
    fn default() -> TickRateManager {
        TickRateManager {
            tickrate: Cell::new(20.0),
            nanoseconds_per_tick: Cell::new(NANOSECONDS_PER_SECOND / 20),
            frozen_ticks_to_run: Cell::new(0),
            run_game_elements: Cell::new(true),
            is_frozen: Cell::new(false),
        }
    }
}

impl TickRateManager {
    pub fn set_tick_rate(&self, rate: f32) {
        self.tickrate.set(rate.max(MIN_TICKRATE));
        self.nanoseconds_per_tick.set((NANOSECONDS_PER_SECOND as f64 / self.tickrate.get() as f64) as i64);
    }

    pub fn tickrate(&self) -> f32 {
        self.tickrate.get()
    }

    pub fn milliseconds_per_tick(&self) -> f32 {
        self.nanoseconds_per_tick.get() as f32 / 1_000_000.0
    }

    pub fn nanoseconds_per_tick(&self) -> i64 {
        self.nanoseconds_per_tick.get()
    }

    pub fn runs_normally(&self) -> bool {
        self.run_game_elements.get()
    }

    pub fn is_stepping_forward(&self) -> bool {
        self.frozen_ticks_to_run.get() > 0
    }

    pub fn frozen_ticks_to_run(&self) -> i32 {
        self.frozen_ticks_to_run.get()
    }

    pub fn set_frozen_ticks_to_run(&self, ticks: i32) {
        self.frozen_ticks_to_run.set(ticks);
    }

    pub fn set_frozen(&self, frozen: bool) {
        self.is_frozen.set(frozen);
    }

    pub fn is_frozen(&self) -> bool {
        self.is_frozen.get()
    }

    pub fn tick(&self) {
        self.run_game_elements.set(!self.is_frozen.get() || self.frozen_ticks_to_run.get() > 0);
        if self.frozen_ticks_to_run.get() > 0 {
            self.frozen_ticks_to_run.set(self.frozen_ticks_to_run.get() - 1);
        }
    }

    pub fn is_entity_frozen(&self) -> bool {
        !self.runs_normally()
    }
}
