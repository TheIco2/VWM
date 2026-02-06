use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StylingConfig {
    pub gap: Option<GapConfig>,
    pub border_width: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GapConfig {
    pub space: u32,
    pub behavior: GapBehavior,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapBehavior {
    PerWindow,
    Shared,
}

impl Default for StylingConfig {
    fn default() -> Self {
        StylingConfig {
            gap: Some(GapConfig {
                space: 10,
                behavior: GapBehavior::PerWindow,
            }),
            border_width: Some(2),
        }
    }
}
