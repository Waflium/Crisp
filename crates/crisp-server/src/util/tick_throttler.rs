pub struct TickThrottler {
    increment_step: i32,
    threshold: i32,
    count: i32,
}

impl TickThrottler {
    pub fn new(increment_step: i32, threshold: i32) -> TickThrottler {
        TickThrottler { increment_step, threshold, count: 0 }
    }

    pub fn increment(&mut self) {
        self.count += self.increment_step;
    }

    pub fn tick(&mut self) {
        if self.count > 0 {
            self.count -= 1;
        }
    }

    pub fn is_under_threshold(&self) -> bool {
        self.threshold <= 0 || self.count < self.threshold
    }
}
