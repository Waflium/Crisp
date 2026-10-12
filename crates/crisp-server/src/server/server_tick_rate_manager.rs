use std::ops::Deref;
use std::rc::Rc;
use std::time::Instant;

use crisp_data::packets::play::clientbound;

use super::Server;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::network::connection::Outbound;
use crate::util::java_format_fixed;
use crate::world::tick_rate_manager::TickRateManager;

#[derive(Default)]
pub struct ServerTickRateManager {
    tick_rate_manager: Rc<TickRateManager>,
    remaining_sprint_ticks: i64,
    sprint_tick_start_time: Option<Instant>,
    sprint_time_spend: i64,
    scheduled_current_sprint_ticks: i64,
    previous_is_frozen: bool,
}

impl Deref for ServerTickRateManager {
    type Target = TickRateManager;

    fn deref(&self) -> &TickRateManager {
        &self.tick_rate_manager
    }
}

impl ServerTickRateManager {
    pub fn shared(&self) -> Rc<TickRateManager> {
        self.tick_rate_manager.clone()
    }

    pub fn is_sprinting(&self) -> bool {
        self.scheduled_current_sprint_ticks > 0
    }

    pub fn end_tick_work(&mut self) {
        if let Some(start) = self.sprint_tick_start_time {
            self.sprint_time_spend += start.elapsed().as_nanos() as i64;
        }
    }
}

impl Server {
    pub fn set_frozen(&mut self, frozen: bool) {
        self.tick_rate_manager.set_frozen(frozen);
        self.update_ticking_state_to_clients();
    }

    fn update_ticking_state_to_clients(&self) {
        for p in self.players() {
            self.send_ticking_state(&p.connection);
        }
    }

    fn send_ticking_state(&self, out: &Outbound) {
        let manager = &self.tick_rate_manager;
        out.send(clientbound::TICKING_STATE, |w| {
            w.f32(manager.tickrate());
            w.bool(manager.is_frozen());
        });
    }

    fn update_step_ticks(&self) {
        for p in self.players() {
            p.connection.send(clientbound::TICKING_STEP, |w| w.varint(self.tick_rate_manager.frozen_ticks_to_run()));
        }
    }

    pub fn update_joining_player_tick_rate(&self, out: &Outbound) {
        self.send_ticking_state(out);
        out.send(clientbound::TICKING_STEP, |w| w.varint(self.tick_rate_manager.frozen_ticks_to_run()));
    }

    pub fn step_game_if_paused(&mut self, ticks: i32) -> bool {
        if !self.tick_rate_manager.is_frozen() {
            return false;
        }
        self.tick_rate_manager.set_frozen_ticks_to_run(ticks);
        self.update_step_ticks();
        true
    }

    pub fn stop_stepping(&mut self) -> bool {
        if self.tick_rate_manager.frozen_ticks_to_run() > 0 {
            self.tick_rate_manager.set_frozen_ticks_to_run(0);
            self.update_step_ticks();
            true
        } else {
            false
        }
    }

    pub fn stop_sprinting(&mut self) -> bool {
        if self.tick_rate_manager.remaining_sprint_ticks > 0 {
            self.finish_tick_sprint();
            true
        } else {
            false
        }
    }

    pub fn request_game_to_sprint(&mut self, time: i32) -> bool {
        let manager = &mut self.tick_rate_manager;
        let interrupted = manager.remaining_sprint_ticks > 0;
        manager.sprint_time_spend = 0;
        manager.scheduled_current_sprint_ticks = time as i64;
        manager.remaining_sprint_ticks = time as i64;
        manager.previous_is_frozen = manager.is_frozen();
        self.set_frozen(false);
        interrupted
    }

    fn finish_tick_sprint(&mut self) {
        let manager = &mut self.tick_rate_manager;
        let completed_ticks = manager.scheduled_current_sprint_ticks - manager.remaining_sprint_ticks;
        let milliseconds_to_complete = (manager.sprint_time_spend as f64).max(1.0) / 1_000_000.0;
        let ticks_per_second = (1000.0 * completed_ticks as f64 / milliseconds_to_complete) as i32;
        let milliseconds_per_tick =
            java_format_fixed(if completed_ticks == 0 { manager.milliseconds_per_tick() as f64 } else { milliseconds_to_complete / completed_ticks as f64 }, 2);
        manager.scheduled_current_sprint_ticks = 0;
        manager.sprint_time_spend = 0;
        let source = CommandSourceStack::server(self);
        source.send_success(
            self,
            || Component::translatable_with("commands.tick.sprint.report", vec![ticks_per_second.into(), milliseconds_per_tick.into()]),
            true,
        );
        self.tick_rate_manager.remaining_sprint_ticks = 0;
        self.set_frozen(self.tick_rate_manager.previous_is_frozen);
        self.on_tick_rate_changed();
    }

    pub fn check_should_sprint_this_tick(&mut self) -> bool {
        if !self.tick_rate_manager.runs_normally() {
            return false;
        }
        let manager = &mut self.tick_rate_manager;
        if manager.remaining_sprint_ticks > 0 {
            manager.sprint_tick_start_time = Some(Instant::now());
            manager.remaining_sprint_ticks -= 1;
            true
        } else {
            self.finish_tick_sprint();
            false
        }
    }

    pub fn set_tick_rate(&mut self, rate: f32) {
        self.tick_rate_manager.set_tick_rate(rate);
        self.on_tick_rate_changed();
        self.update_ticking_state_to_clients();
    }

    pub fn compute_next_autosave_interval(&self) -> i32 {
        let ticks_per_second = if self.tick_rate_manager.is_sprinting() {
            let estimated_tick_time_nanos = self.average_tick_time_nanos() + 1;
            1_000_000_000.0f32 / estimated_tick_time_nanos as f32
        } else {
            self.tick_rate_manager.tickrate()
        };
        100.max((ticks_per_second * 300.0) as i32)
    }

    pub fn on_tick_rate_changed(&mut self) {
        let interval = self.compute_next_autosave_interval();
        if interval < self.ticks_until_autosave {
            self.ticks_until_autosave = interval;
        }
    }

    pub fn average_tick_time_nanos(&self) -> i64 {
        self.aggregated_tick_times_nanos / self.tick_count.clamp(1, 100) as i64
    }
}
