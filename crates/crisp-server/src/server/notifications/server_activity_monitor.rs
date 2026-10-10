use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use super::NotificationManager;
use crate::util::get_millis;

pub struct ServerActivityMonitor {
    minimum_millis_between_notifications: i64,
    last_notification_time: AtomicI64,
    server_activity: AtomicBool,
    notification_manager: NotificationManager,
}

impl ServerActivityMonitor {
    pub fn new(notification_manager: NotificationManager, seconds_between_notifications: i64) -> ServerActivityMonitor {
        ServerActivityMonitor {
            minimum_millis_between_notifications: seconds_between_notifications * 1000,
            last_notification_time: AtomicI64::new(i64::MIN / 2),
            server_activity: AtomicBool::new(false),
            notification_manager,
        }
    }

    pub fn tick(&self) {
        self.process_with_rate_limit();
    }

    pub fn report_login_activity(&self) {
        self.server_activity.store(true, Ordering::Relaxed);
        self.process_with_rate_limit();
    }

    fn process_with_rate_limit(&self) {
        let now = get_millis();
        if self.server_activity.load(Ordering::Relaxed) && now - self.last_notification_time.load(Ordering::Relaxed) >= self.minimum_millis_between_notifications {
            self.notification_manager.server_activity_occured();
            self.last_notification_time.store(get_millis(), Ordering::Relaxed);
        }
        self.server_activity.store(false, Ordering::Relaxed);
    }
}
