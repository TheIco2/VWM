use crate::{info, warn};
use crate::DEBUG_NAME;
use crate::ADDON_NAME;
use std::path::{Path, PathBuf};

const DEBUG_SUBTAG: &str = "WATCHER";

/// Universal YAML watcher: watches for changes in YAML config files in the given directory for the given addon.
pub fn yaml_watcher(directory: &Path, is_update_check_enabled: fn() -> bool)
{
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use crate::ipc_connector::request;
    let directory = directory.to_path_buf();
    thread::spawn(move || {
        let yaml_path: PathBuf = directory;

        let debounce_timer = Arc::new(Mutex::new(None::<std::time::Instant>));
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

                        // Schedule reload after 1 second
                        let debounce = Arc::clone(&debounce_timer);
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
                        });
                    }
                }
            }
        }
    });
}