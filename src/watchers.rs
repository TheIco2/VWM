use crate::{
    info, warn, DEBUG_NAME,
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

/// Universal YAML watcher: watches for changes in YAML config files in the given directory for the given addon.
pub fn yaml_watcher(directory: &Path, is_update_check_enabled: fn() -> bool)
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
                            thread::spawn(move || {
                                thread::sleep(Duration::from_secs(1));

                                // Check if timer hasn't been reset
                                if let Ok(timer) = debounce.lock() {
                                    if let Some(last_time) = *timer {
                                        if last_time.elapsed() >= Duration::from_secs(1) {
                                            // Check if update_check is enabled before sending reload
                                            if is_update_check_enabled() {
                                                if let Some(_) = request("addon", "reload", Some(serde_json::json!({"addon_name": ADDON_NAME})))
                                                {
                                                    info!("[{}][{}] Reload request sent to backend", DEBUG_NAME, DEBUG_SUBTAG);
                                                } else
                                                {
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