// ~/Sentinel/sentinel-addons/windowmanager/src/main.rs

#![windows_subsystem = "windows"] 
mod bootstrap;
mod ipc_connector;

mod logging;
mod layout;
mod types;
mod window_ops;
mod window_events;
mod config;
mod utility;
mod watchers;
mod data_loaders;
mod paths;

use crate::{
    ipc_connector::request,
    data_loaders::yaml::load_yaml,
    config::UniversalConfig,
    watchers::yaml_watcher,
    utility::{sentinel_addons_dir, sentinel_assets_dir},
    types::{DisplayInfo, WindowManagerConfig},
    window_events::{
        event_manager::{
            EventManager,
            set_event_manager,
        }, 
        setup_hooks::setup_event_hooks, 
        cleanup_hooks::cleanup_event_hooks,
    }
};

use windows::{
    Win32::{
        Foundation::GetLastError,
        UI::{
            WindowsAndMessaging::*,
            Accessibility::HWINEVENTHOOK,
        },
    },
};

pub const ADDON_NAME: &str = "windowmanager";
pub const DEBUG_NAME: &str = "WINDOWMANAGER";

/* =========================
   IPC MONITORS
   ========================= */
fn ipc_get_displays() -> Option<Vec<DisplayInfo>> {
    info!("[{}][IPC] Requesting monitors via pipe", DEBUG_NAME);

    if let Some(resp) = request("sysdata", "get_displays", None) {
        info!("[{}][IPC] Received IPC response, parsing JSON", DEBUG_NAME);
        // Try to parse the JSON response into Vec<DisplayInfo>
        match serde_json::from_str::<Vec<DisplayInfo>>(&resp) {
            Ok(displays) => {
                for (idx, display) in displays.iter().enumerate() {
                    info!("[{}][IPC] Display {}: pos=({},{}), size={}x{}, primary={}", 
                          DEBUG_NAME, idx, display.x, display.y, display.width, display.height, display.primary);
                }
                Some(displays)
            },
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
fn check_assets() {
    // Check if assets directory '~/.Sentinel/Assets/windowmanager' exists
    if let Some(assets_dir) = sentinel_assets_dir() {
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
    info!("!---------- [{}] Starting Window Manager Addon ----------!", DEBUG_NAME);
    info!("[{}] Performing initial startup tasks", DEBUG_NAME);
    // Check and create assets directory if missing
    check_assets();
}

/* =========================
   MAIN
   ========================= */

fn main() -> windows::core::Result<()> {
    logging::init(true, "info");
    bootstrap::bootstrap_addon();
    initial_startup();
    let mut debug_enabled = false;
    let mut log_level = "warn".to_string();
    let mut window_manager_config = WindowManagerConfig::default();
    let mut universal_config = UniversalConfig::default();
    
    if let Some(addons_dir) = sentinel_addons_dir() {
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
            
            // Load universal configuration from YAML
            if let Some(universal_value) = value.get("universal") {
                if let Ok(config) = serde_yaml::from_value::<UniversalConfig>(universal_value.clone()) {
                    universal_config = config;
                    info!("[{}] Loaded universal config from YAML", DEBUG_NAME);
                }
            }
            
            // Load window manager configuration from YAML
            if let Some(wm_value) = value.get("window_manager") {
                if let Ok(config) = serde_yaml::from_value::<WindowManagerConfig>(wm_value.clone()) {
                    window_manager_config = config;
                    info!("[{}] Loaded window manager config from YAML", DEBUG_NAME);
                } else {
                    warn!("[{}] Failed to parse window_manager config, using defaults", DEBUG_NAME);
                }
            }
        }
    }

    logging::set_debug(debug_enabled);
    std::panic::set_hook(Box::new(|info| {
        error!("[{}] Panic: {}", DEBUG_NAME, info);
    }));
    info!("[{}] Window Manager addon starting", DEBUG_NAME);
    info!("[{}] Window Manager enabled: {}", DEBUG_NAME, window_manager_config.enabled);
    

    unsafe {
        let mut hooks: Option<Vec<HWINEVENTHOOK>> = None;

        if window_manager_config.enabled {
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

            // Merge universal exclusions with window manager filters
            if let Some(universal_excludes) = &universal_config.exclude_processes {
                if let Some(filters) = &mut window_manager_config.filters {
                    if let Some(wm_excludes) = &mut filters.exclude_processes {
                        // Combine: start with universal, then add window-manager specific ones
                        let mut combined = universal_excludes.clone();
                        combined.extend(wm_excludes.iter().cloned());
                        // Remove duplicates while preserving order
                        combined.sort();
                        combined.dedup();
                        *wm_excludes = combined;
                    } else {
                        filters.exclude_processes = Some(universal_excludes.clone());
                    }
                } else {
                    // Create filters with universal exclusions
                    window_manager_config.filters = Some(crate::config::FiltersConfig {
                        exclude_processes: Some(universal_excludes.clone()),
                        ..Default::default()
                    });
                }
            }

            // Use the loaded config for all monitors
            let mut configs = Vec::new();
            for (idx, monitor) in monitors.iter().cloned().enumerate() {
                info!("[{}] Initializing Window Manager for monitor {} ({}x{})", 
                      DEBUG_NAME, idx, monitor.width, monitor.height);
                
                let manager_type = window_manager_config.manager_type.unwrap_or_default();
                info!("[{}] Monitor {} using {} layout manager (debounce: {}ms, animation: {})", 
                      DEBUG_NAME, idx, manager_type, 
                      window_manager_config.events.as_ref().and_then(|e| e.debounce_ms).unwrap_or(500),
                      window_manager_config.animation.as_ref().and_then(|a| a.enabled).unwrap_or(true));
                configs.push(window_manager_config.clone());
            }

            // Set up event-driven management with debounce and drag detection
            let event_manager = EventManager::new(monitors.clone(), configs);
            set_event_manager(event_manager);
            hooks = Some(setup_event_hooks());
            info!("[{}] Event hooks installed - window manager is now active", DEBUG_NAME);
        } else {
            info!("[{}] Window Manager disabled in config - staying idle", DEBUG_NAME);
        }

        if let Some(addons_dir) = sentinel_addons_dir() {
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

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if let Some(hooks) = hooks {
            cleanup_event_hooks(hooks);
        }
    }

    Ok(())
}