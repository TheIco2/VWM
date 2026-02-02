// ~/Sentinel/sentinel-addons/windowmanager/src/main.rs

#![windows_subsystem = "windows"] 
mod ipc_connector;

mod logging;
mod layout;
mod types;
mod window_ops;
mod window_events;

mod utility;
mod watchers;
mod data_loaders;

use ipc_connector::request;
use data_loaders::yaml::load_yaml;
use types::{DisplayInfo, WindowManagerConfig};
use window_events::{EventManager, setup_event_hooks, cleanup_event_hooks, set_event_manager};
use watchers::yaml_watcher;
use utility::{_sentinel_addons_dir, _sentinel_assets_dir};

use windows::{
    Win32::{
        Foundation::GetLastError,
        UI::{
            WindowsAndMessaging::*,
        },
    },
};

pub const ADDON_NAME: &str = "windowmanager";
pub const DEBUG_NAME: &str = "WINDOWMANAGER";

/* ========================= APP STATE ========================= */
#[derive(Clone)]
struct AppState {
    callback_msg: u32,
    monitor: DisplayInfo,
}

// MonitorInfo and IpcMonitorsResponse moved to `src/types.rs`

/* =========================
   IPC MONITORS
   ========================= */
fn ipc_get_displays() -> Option<Vec<DisplayInfo>> {
    info!("[{}][IPC] Requesting monitors via pipe", DEBUG_NAME);

    if let Some(resp) = request("sysdata", "get_displays", None) {
        info!("[{}][IPC] Received IPC response, parsing JSON", DEBUG_NAME);
        // Try to parse the JSON response into Vec<DisplayInfo>
        match serde_json::from_str::<Vec<DisplayInfo>>(&resp) {
            Ok(displays) => Some(displays),
            Err(e) => {
                error!("[{}][IPC] Failed to parse IPC response: {}", DEBUG_NAME, e);
                None
            }
        }
    } else {
        warn!("[{}][IPC] No IPC response received", DEBUG_NAME);
        None
    }
}

/* =========================
   Initial Startup
   ========================= */
fn check_config() {
    // Checks if config.yaml at '~/.Sentinel/addons/<addon>/' exists, if not create default
    if let Some(addons_dir) = _sentinel_addons_dir() {
        let yaml_path = addons_dir.join(ADDON_NAME).join("config.yaml");
        if !yaml_path.exists() {
            info!("[{}] No config.yaml found for Window Manager, creating default", DEBUG_NAME);
            // Create default config.yaml
            let default_yaml = r#"update_check: true
debug: false

# Window Manager Configuration
window_manager:
  enabled: true
  manager_type: tiling  # Options: tiling, floating, stacking
  gap: 10               # Gap between windows in pixels
  border_width: 2       # Border width in pixels
  
  filters:
    min_width: 200
    min_height: 200
    exclude_classes:
      - "Shell_TrayWnd"
      - "Progman"
      - "WorkerW"
    exclude_titles:
      - "Program Manager"
    include_processes: []
    exclude_processes:
      - "explorer.exe""#;

            if let Err(e) = std::fs::write(&yaml_path, default_yaml) {
                error!("[{}] Failed to create default config.yaml: {}", DEBUG_NAME, e);
            } else {
                info!("[{}] Default config.yaml created at {}", DEBUG_NAME, yaml_path.display());
            }
        } else {
            info!("[{}] Found existing config.yaml for Window Manager", DEBUG_NAME);
        }   
    } else {
        error!("[{}] Failed to get addons directory", DEBUG_NAME);
    }
}

fn check_assets() {
    // Check if assets directory '~/.Sentinel/Assets/windowmanager' exists
    if let Some(assets_dir) = _sentinel_assets_dir() {
        let assets_dir = assets_dir.join(ADDON_NAME);
        if !assets_dir.exists() {
            info!("[{}] No assets directory found for Window Manager, creating default", DEBUG_NAME);
            // Create assets directory
            if let Err(e) = std::fs::create_dir_all(&assets_dir) {
                error!("[{}] Failed to create assets directory: {}", DEBUG_NAME, e);
            } else {
                info!("[{}] Assets directory created at {}", DEBUG_NAME, assets_dir.display());
            }
        } else {
            info!("[{}] Found existing assets directory for Window Manager", DEBUG_NAME);
        }   
    } else {
        error!("[{}] Failed to get assets directory", DEBUG_NAME);
    }
}

pub fn initial_startup() {
    info!("[{}] Performing initial startup tasks", DEBUG_NAME);

    // Check and create config.yaml if missing
    check_config();
    // Check and create assets directory if missing
    check_assets();
}

/* =========================
   MAIN
   ========================= */

fn main() -> windows::core::Result<()> {
    initial_startup();
    let mut debug_enabled = false;
    let mut log_level = "warn".to_string();
    if let Some(addons_dir) = _sentinel_addons_dir() {
        let yaml_path = addons_dir.join(ADDON_NAME).join("config.yaml");
        if let Some(value) = load_yaml(&yaml_path) {
            debug_enabled = value
                .get("debug")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            log_level = value
                .get("log_level")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_else(|| if debug_enabled { "info".to_string() } else { "warn".to_string() });
        }
    }

    logging::init(debug_enabled, &log_level);
    std::panic::set_hook(Box::new(|info| {
        error!("[{}] Panic: {}", DEBUG_NAME, info);
    }));
    info!("[{}] Window Manager addon starting", DEBUG_NAME);
    

    unsafe {
        // get monitors from IPC
        let ipc_displays = ipc_get_displays().unwrap_or_default();
        // If DisplayInfo and MonitorInfo are the same, you can use DisplayInfo directly.
        // Otherwise, convert DisplayInfo to MonitorInfo as needed.
        let monitors: Vec<DisplayInfo> = ipc_displays.into_iter().collect();
        if monitors.is_empty() {
            error!("[{}] No monitors received from IPC", DEBUG_NAME);
            return Ok(());
        }

        info!("[{}] Starting Window Manager for {} monitor(s)", DEBUG_NAME, monitors.len());

        // Collect configs for event-driven window management
        let mut configs = Vec::new();
        for (idx, monitor) in monitors.iter().cloned().enumerate() {
            info!("[{}] Initializing Window Manager for monitor {} ({}x{})", 
                  DEBUG_NAME, idx, monitor.width, monitor.height);
            
            // Create default config for this monitor
            let config = WindowManagerConfig::default();
            let manager_type = config.manager_type.unwrap_or_default();
            info!("[{}] Monitor {} using {} layout manager", 
                  DEBUG_NAME, idx, manager_type);
            configs.push(config);
        }

        // Set up event-driven management with debounce and drag detection
        let event_manager = EventManager::new(monitors.clone(), configs);
        set_event_manager(event_manager);
        let hooks = setup_event_hooks();
        info!("[{}] Event hooks installed - window manager is now active", DEBUG_NAME);

        if let Some(addons_dir) = _sentinel_addons_dir() {
            let yaml_dir = addons_dir.join("windowmanager").join("config.yaml");
            yaml_watcher(&yaml_dir, || true);
        } else {
            error!("[{}] Failed to get addons directory", DEBUG_NAME);
        }

        let mut msg: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut msg, None, 0, 0);
            if result.0 == 0 {
                warn!("[{}] Message loop exited (WM_QUIT received)", DEBUG_NAME);
                break;
            }
            if result.0 == -1 {
                let err = GetLastError();
                error!("[{}] Message loop error: {:?}", DEBUG_NAME, err);
                break;
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        cleanup_event_hooks(hooks);
    }

    Ok(())
}