use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::HWND;

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
pub struct WindowFilters {
    pub min_width: Option<i32>,
    pub min_height: Option<i32>,
    pub exclude_classes: Option<Vec<String>>,
    pub exclude_titles: Option<Vec<String>>,
    pub include_processes: Option<Vec<String>>,
    pub exclude_processes: Option<Vec<String>>,
}

impl Default for WindowFilters {
    fn default() -> Self {
        WindowFilters {
            min_width: None,  // Don't filter by size - let layout algorithm handle it
            min_height: None, // This allows managing 6+ windows on a single monitor
            exclude_classes: Some(vec![
                "Shell_TrayWnd".to_string(),
                "Progman".to_string(),
                "WorkerW".to_string(),
            ]),
            exclude_titles: Some(vec![
                "Program Manager".to_string(),
                "Task Manager".to_string(),
                "Settings".to_string(),
                "Windows Input Experience".to_string(),
                "PowerToys Quick Access".to_string(),
            ]),
            include_processes: None,
            exclude_processes: Some(vec![
                "explorer.exe".to_string(),
                "taskmgr.exe".to_string(),
                "systemsettings.exe".to_string(),
                "steamwebhelper.exe".to_string(),
                "msiexec.exe".to_string(),
            ]),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowManagerConfig {
    pub enabled: bool,
    pub manager_type: Option<ManagerType>,
    pub gap: Option<u32>,
    pub border_width: Option<u32>,
    pub debounce_ms: Option<u64>,
    pub animation_enabled: Option<bool>,
    pub animation_duration_ms: Option<u64>,
    pub filters: Option<WindowFilters>,
}

impl Default for WindowManagerConfig {
    fn default() -> Self {
        WindowManagerConfig {
            enabled: true,
            manager_type: Some(ManagerType::Tiling),
            gap: Some(10),
            border_width: Some(2),
            debounce_ms: Some(500),
            animation_enabled: Some(true),
            animation_duration_ms: Some(150),
            filters: Some(WindowFilters::default()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagedWindow {
    pub hwnd: HWND,
    pub title: String,
    pub class_name: String,
    pub process_name: String,
}

#[derive(Debug, Clone)]
pub struct WindowManager {
    pub config: WindowManagerConfig,
    pub display: DisplayInfo,
    pub windows: Vec<ManagedWindow>,
}

impl WindowManager {
    pub fn new(display: DisplayInfo, config: WindowManagerConfig) -> Self {
        WindowManager { 
            config, 
            display,
            windows: Vec::new(),
        }
    }
}
