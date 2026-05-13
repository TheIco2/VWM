use crate::{
    error, info, warn, DEBUG_NAME,
    ipc_connector::request,

};

use std::{
    sync::{Arc, Mutex},
    time::Duration,
    thread,
    path::{Path, PathBuf},
};

const DEBUG_SUBTAG: &str = "WATCHER";
const ADDON_NAME: &str = "Window Manager";

fn is_update_check_enabled_from_yaml(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };

    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return false;
    };

    value
        .get("settings")
        .and_then(|d| d.get("update_check"))
        .and_then(|v| v.as_bool())
        .or_else(|| value.get("update_check").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn relaunch_current_process() {
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            error!("[{}][{}] Failed to resolve current executable for relaunch: {}", DEBUG_NAME, DEBUG_SUBTAG, e);
            return;
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    match std::process::Command::new(&exe).args(&args).spawn() {
        Ok(_) => {
            info!("[{}][{}] Relaunched process after config change: {}", DEBUG_NAME, DEBUG_SUBTAG, exe.display());
            std::process::exit(0);
        }
        Err(e) => {
            error!("[{}][{}] Failed to relaunch process after config change: {}", DEBUG_NAME, DEBUG_SUBTAG, e);
        }
    }
}

/// Universal YAML watcher: watches for config changes and either notifies backend (addon mode)
/// or relaunches the standalone process (standalone mode) when update_check is enabled.
pub fn yaml_watcher(directory: &Path, standalone: bool)
{
    let directory = directory.to_path_buf();
    thread::spawn(move || {
        let yaml_path: PathBuf = directory;

        let debounce_timer = Arc::new(Mutex::new(None::<std::time::Instant>));
        let pending_reload = Arc::new(Mutex::new(false));
        let mut last_modified = std::fs::metadata(&yaml_path)
            .and_then(|m| m.modified())
            .ok();

        info!("[{}][{}] Watching for changes to: {}", DEBUG_NAME, DEBUG_SUBTAG, yaml_path.display());

        loop {
            thread::sleep(Duration::from_secs(1));

            // Check if file has been modified
            if let Ok(metadata) = std::fs::metadata(&yaml_path) {
                if let Ok(modified) = metadata.modified() {
                    if Some(modified) != last_modified {
                        last_modified = Some(modified);
                        info!("[{}][{}] Detected change to {}", DEBUG_NAME, DEBUG_SUBTAG, yaml_path.display());

                        // Set debounce timer
                        let now = std::time::Instant::now();
                        let mut timer = debounce_timer.lock().unwrap();
                        *timer = Some(now);
                        drop(timer);

                        // Only spawn ONE debounce thread if one isn't already running
                        let mut pending = pending_reload.lock().unwrap();
                        if !*pending {
                            *pending = true;
                            drop(pending);

                            let debounce = Arc::clone(&debounce_timer);
                            let pending_clone = Arc::clone(&pending_reload);
                            let yaml_path_for_thread = yaml_path.clone();
                            thread::spawn(move || {
                                thread::sleep(Duration::from_secs(1));

                                // Check if timer hasn't been reset
                                if let Ok(timer) = debounce.lock() {
                                    if let Some(last_time) = *timer {
                                        if last_time.elapsed() >= Duration::from_secs(1) {
                                            // Check if update_check is enabled before reloading
                                            if is_update_check_enabled_from_yaml(&yaml_path_for_thread) {
                                                if standalone {
                                                    info!("[{}][{}] update_check enabled; relaunching standalone process", DEBUG_NAME, DEBUG_SUBTAG);
                                                    relaunch_current_process();
                                                } else if let Some(_) = request("addon", "reload", Some(serde_json::json!({"addon_name": ADDON_NAME}))) {
                                                    info!("[{}][{}] Reload request sent to backend", DEBUG_NAME, DEBUG_SUBTAG);
                                                } else {
                                                    warn!("[{}][{}] Failed to send reload request to backend", DEBUG_NAME, DEBUG_SUBTAG);
                                                }
                                            } else {
                                                info!("[{}][{}] Config changed but update_check is disabled", DEBUG_NAME, DEBUG_SUBTAG);
                                            }
                                        }
                                    }
                                }
                                // Mark pending reload as complete
                                if let Ok(mut pending) = pending_clone.lock() {
                                    *pending = false;
                                }
                            });
                        }
                    }
                }
            }
        }
    });
}