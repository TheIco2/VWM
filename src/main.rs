// ~/VEIL/veil-addons/windowmanager/src/main.rs

#![windows_subsystem = "windows"] 
mod bootstrap;
mod display_provider;
mod ipc_connector;

pub mod installer;
mod logging;
mod layout;
mod types;
mod window_ops;
mod window_events;
mod config;
mod utility;
mod watchers;
mod data_loaders;

use crate::{
    display_provider::get_displays,
    data_loaders::yaml::load_yaml,
    config::UniversalConfig,
    watchers::yaml_watcher,
    utility::{veil_addons_dir, veil_assets_dir},
    types::WindowManagerConfig,
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

fn standalone_mode() -> bool {
    if cfg!(feature = "standalone-mode") {
        return true;
    }

    let arg_enabled = std::env::args().any(|arg| arg.eq_ignore_ascii_case("--standalone"));
    let env_enabled = std::env::var("VEIL_STANDALONE")
        .map(|v| {
            let normalized = v.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes"
        })
        .unwrap_or(false);

    arg_enabled || env_enabled
}

/* =========================
   IPC MONITORS
   ========================= */
/* =========================
   Initial Startup
   ========================= */
fn check_assets() {
    // Check if assets directory '~/ProjectOpen/VEIL/Assets/windowmanager' exists
    if let Some(assets_dir) = veil_assets_dir() {
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
    logging::init("VEIL", "WindowManager", true);
    let standalone = standalone_mode();

    if standalone {
        info!("[{}] Running in standalone mode", DEBUG_NAME);
    } else {
        bootstrap::bootstrap_addon();
    }

    initial_startup();
    let mut debug_enabled = false;
    let mut _log_level = "warn".to_string();
    let mut window_manager_config = WindowManagerConfig::default();
    let mut universal_config = UniversalConfig::default();
    
    if let Some(addons_dir) = veil_addons_dir() {
        let yaml_path = addons_dir.join(ADDON_NAME).join("config.yaml");
        if let Some(value) = load_yaml(&yaml_path) {
            let settings = value.get("settings");
            let development = settings.and_then(|s| s.get("development"));

            debug_enabled = value
                .get("debug")
                .or_else(|| development.and_then(|d| d.get("debug")))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            _log_level = value
                .get("log_level")
                .or_else(|| development.and_then(|d| d.get("log_level")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_else(|| if debug_enabled { "info".to_string() } else { "warn".to_string() });
            
            // Load universal configuration from YAML
            if let Some(universal_value) = settings
                .and_then(|s| s.get("universal"))
                .or_else(|| value.get("universal"))
            {
                if let Ok(config) = serde_yaml::from_value::<UniversalConfig>(universal_value.clone()) {
                    universal_config = config;
                    info!("[{}] Loaded universal config from YAML", DEBUG_NAME);
                }
            }
            
            // Load window manager configuration from YAML
            if let Some(wm_value) = settings
                .and_then(|s| s.get("window_manager"))
                .or_else(|| value.get("window_manager"))
            {
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
            let monitors = get_displays(standalone);
            if monitors.is_empty() {
                error!("[{}] No monitors available (IPC/local lookup failed)", DEBUG_NAME);
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

        if let Some(addons_dir) = veil_addons_dir() {
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