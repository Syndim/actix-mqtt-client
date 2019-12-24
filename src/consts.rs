use std::time::Duration;

use lazy_static::lazy_static;

pub const PROTOCOL_NAME: &str = "MQTT";
pub const MAX_RETRY_COUNT: u16 = 5;
pub const DEFAULT_MAILBOX_CAPACITY: usize = 64;
pub const MAILBOX_CAPACITY_FOR_PUBLISH: usize = 256;

lazy_static! {
    pub static ref COMMAND_TIMEOUT: Duration = Duration::new(5, 0);
    pub static ref DELAY_BEFORE_SHUTDOWN: Duration = Duration::new(5, 0);
    pub static ref RESEND_DELAY: Duration = Duration::new(1, 0);
    pub static ref PING_INTERVAL: Duration = Duration::new(10, 0);
}
