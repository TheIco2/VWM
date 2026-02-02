use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

#[derive(Debug, Deserialize)]
pub struct IpcMonitorsResponse {
    pub ok: bool,
    pub data: Option<Vec<DisplayInfo>>,
    pub error: Option<String>,
}

// Window Manager Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ManagerType {
    Tiling,
    Floating,
    Stacking,
}

impl Default for ManagerType {
    fn default() -> Self {
        ManagerType::Tiling
    }
}

impl std::fmt::Display for ManagerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerType::Tiling => write!(f, "Tiling"),
            ManagerType::Floating => write!(f, "Floating"),
            ManagerType::Stacking => write!(f, "Stacking"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowManagerConfig {
    pub enabled: bool,
    pub manager_type: Option<ManagerType>,
    pub gap: Option<u32>,
    pub border_width: Option<u32>,
}

impl Default for WindowManagerConfig {
    fn default() -> Self {
        WindowManagerConfig {
            enabled: true,
            manager_type: Some(ManagerType::Tiling),
            gap: Some(10),
            border_width: Some(2),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WindowManager {
    pub config: WindowManagerConfig,
    pub display: DisplayInfo,
}

impl WindowManager {
    pub fn new(display: DisplayInfo, config: WindowManagerConfig) -> Self {
        WindowManager { config, display }
    }
}
