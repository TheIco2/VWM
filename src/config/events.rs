// ~/src/config/events.rs
// Events configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventsConfig {
    pub debounce_ms: Option<u64>,
}

impl Default for EventsConfig {
    fn default() -> Self {
        EventsConfig {
            debounce_ms: Some(500),
        }
    }
}
