// ~/src/config/filters.rs
// Window filters configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FiltersConfig {
    pub min_width: Option<i32>,
    pub min_height: Option<i32>,
    pub include_processes: Option<Vec<String>>,
    pub exclude_processes: Option<Vec<String>>,
    pub exclude_classes: Option<Vec<String>>,
    pub exclude_titles: Option<Vec<String>>,
}

impl Default for FiltersConfig {
    fn default() -> Self {
        FiltersConfig {
            min_width: None,
            min_height: None,
            include_processes: None,
            exclude_processes: Some(vec![
                "explorer.exe".to_string(),
                "taskmgr.exe".to_string(),
                "systemsettings.exe".to_string(),
                "steamwebhelper.exe".to_string(),
                "msiexec.exe".to_string(),
            ]),
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
        }
    }
}
