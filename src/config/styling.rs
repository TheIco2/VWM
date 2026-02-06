use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StylingConfig {
    pub gap: Option<GapConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GapConfig {
    pub space: u32,
    pub behavior: GapBehavior,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapBehavior {
    #[serde(alias = "PerWindow", alias = "perwindow", alias = "PERWINDOW", alias = "Per_Window", alias = "per_window")]
    PerWindow,
    #[serde(alias = "Shared", alias = "shared", alias = "SHARED")]
    Shared,
}

impl Default for StylingConfig {
    fn default() -> Self {
        StylingConfig {
            gap: Some(GapConfig {
                space: 10,
                behavior: GapBehavior::PerWindow,
            }),
        }
    }
}
