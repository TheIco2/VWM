// ~/src/config/universal.rs
// Universal configuration (applies to all window managers)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UniversalConfig {
    pub exclude_processes: Option<Vec<String>>,
}

impl Default for UniversalConfig {
    fn default() -> Self {
        UniversalConfig {
            exclude_processes: None,
        }
    }
}
