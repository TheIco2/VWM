use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HWND;
use crate::{
    config::{AnimationConfig, EventsConfig, FiltersConfig, StylingConfig},
};

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayInfo {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[allow(dead_code)]
    pub scale: f64,
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
    pub monitor_index: Option<Vec<String>>,
    pub animation: Option<AnimationConfig>,
    pub styling: Option<StylingConfig>,
    pub events: Option<EventsConfig>,
    pub filters: Option<FiltersConfig>,
}

impl Default for WindowManagerConfig {
    fn default() -> Self {
        WindowManagerConfig {
            enabled: true,
            manager_type: Some(ManagerType::Tiling),
            monitor_index: Some(vec!["*".to_string()]),
            animation: Some(AnimationConfig::default()),
            styling: Some(StylingConfig::default()),
            events: Some(EventsConfig::default()),
            filters: Some(FiltersConfig::default()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagedWindow {
    pub hwnd: HWND,
    #[allow(dead_code)]
    pub title: String,
    #[allow(dead_code)]
    pub class_name: String,
    #[allow(dead_code)]
    pub process_name: String,
}


