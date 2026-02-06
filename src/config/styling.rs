// ~/src/config/styling.rs
// Styling configuration (gap, borders, etc.)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StylingConfig {
    pub gap: Option<u32>,
    pub border_width: Option<u32>,
}

impl Default for StylingConfig {
    fn default() -> Self {
        StylingConfig {
            gap: Some(10),
            border_width: Some(2),
        }
    }
}
