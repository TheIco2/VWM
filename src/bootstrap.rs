// ~/Sentinel/sentinel-addons/windowmanager/src/bootstrap.rs

use std::fs;
use std::path::PathBuf;
use crate::ADDON_NAME;
use crate::utility::sentinel_addons_dir;
use crate::{info, warn};

const EXE_NAME: &str = "sentinel-windowmanager.exe";


/// Returns the canonical addon install directory
fn addon_install_dir() -> Option<PathBuf> {
    sentinel_addons_dir().map(|d| d.join(ADDON_NAME))
}

/// Returns true if the currently running exe is inside the addon's bin/ folder.
fn is_running_from_install_dir() -> bool {
    let install_bin = match addon_install_dir() {
        Some(d) => d.join("bin"),
        None => return false,
    };
    match std::env::current_exe() {
        Ok(exe) => exe.starts_with(&install_bin),
        Err(_) => false,
    }
}

/// Check if sentinelc.exe (the backend) is running; if not, start it.
fn ensure_backend_running() {
    info!("[{}] Checking if sentinelc.exe is running...", ADDON_NAME);
    let backend_running = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq sentinelc.exe", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("sentinelc.exe"))
        .unwrap_or(false);

    if backend_running {
        info!("[{}] sentinelc.exe is already running", ADDON_NAME);
        return;
    }

    warn!("[{}] sentinelc.exe is NOT running, attempting to start it", ADDON_NAME);
    let Some(home) = std::env::var("USERPROFILE").ok() else {
        warn!("[{}] Cannot resolve USERPROFILE to find sentinelc.exe", ADDON_NAME);
        return;
    };
    let backend_exe = PathBuf::from(&home).join(".Sentinel").join("sentinelc.exe");
    if !backend_exe.exists() {
        warn!("[{}] Backend not found at {}", ADDON_NAME, backend_exe.display());
        return;
    }
    match std::process::Command::new(&backend_exe).spawn() {
        Ok(_) => {
            info!("[{}] Started sentinelc.exe from {}", ADDON_NAME, backend_exe.display());
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        Err(e) => warn!("[{}] Failed to start sentinelc.exe: {e}", ADDON_NAME),
    }
}

pub fn bootstrap_addon() {
    info!("[{}] === Bootstrap starting ===", ADDON_NAME);
    info!("[{}] Current exe: {:?}", ADDON_NAME, std::env::current_exe());

    ensure_backend_running();

    let addon_dir = match addon_install_dir() {
        Some(d) => d,
        None => {
            warn!("[{}] Cannot resolve addon install directory", ADDON_NAME);
            return;
        }
    };
    info!("[{}] Addon directory: {}", ADDON_NAME, addon_dir.display());

    let bin_dir = addon_dir.join("bin");
    let options_dir = addon_dir.join("options");
    let _ = fs::create_dir_all(&bin_dir);
    let _ = fs::create_dir_all(&options_dir);
    info!("[{}] Created directory structure at {}", ADDON_NAME, addon_dir.display());

    scaffold_addon_json(&addon_dir);
    scaffold_config_yaml(&addon_dir);
    scaffold_schema_yaml(&addon_dir);
    scaffold_options_html(&options_dir);
    info!("[{}] Scaffolding complete", ADDON_NAME);

    if is_running_from_install_dir() {
        info!("[{}] Already running from install directory - continuing startup", ADDON_NAME);
        return;
    }

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => { warn!("[{}] Cannot determine current exe path: {e}", ADDON_NAME); return; }
    };

    let dst = bin_dir.join(EXE_NAME);
    info!("[{}] Source: {}", ADDON_NAME, current_exe.display());
    info!("[{}] Target: {}", ADDON_NAME, dst.display());

    let should_copy = match (fs::metadata(&current_exe), fs::metadata(&dst)) {
        (Ok(src_meta), Ok(dst_meta)) => {
            let src_size = src_meta.len();
            let dst_size = dst_meta.len();
            let src_newer = src_meta.modified().ok().zip(dst_meta.modified().ok())
                .map(|(s, d)| s > d).unwrap_or(false);
            info!("[{}] Source size={src_size}, Target size={dst_size}, source_newer={src_newer}", ADDON_NAME);
            src_newer || src_size != dst_size
        }
        (Ok(src_meta), Err(_)) => {
            info!("[{}] Target does not exist, source size={}", ADDON_NAME, src_meta.len());
            true
        }
        _ => {
            warn!("[{}] Cannot read source exe metadata", ADDON_NAME);
            false
        }
    };

    if should_copy {
        info!("[{}] Copying exe to install directory...", ADDON_NAME);
        match fs::copy(&current_exe, &dst) {
            Ok(bytes) => info!("[{}] Copied {bytes} bytes -> {}", ADDON_NAME, dst.display()),
            Err(e) => {
                warn!("[{}] Failed to copy exe: {e}", ADDON_NAME);
                return;
            }
        }
    } else {
        info!("[{}] Exe already up to date, skipping copy", ADDON_NAME);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    info!("[{}] Relaunching from {} with args: {:?}", ADDON_NAME, dst.display(), args);
    match std::process::Command::new(&dst).args(&args).spawn() {
        Ok(_) => {
            info!("[{}] Relaunch successful, exiting current process", ADDON_NAME);
            std::process::exit(0);
        }
        Err(e) => {
            warn!("[{}] Failed to relaunch: {e}", ADDON_NAME);
        }
    }
}

fn scaffold_addon_json(addon_dir: &PathBuf) {
    let path = addon_dir.join("addon.json");
    if path.exists() { return; }

    let content = r#"{
    "id": "sentinel.addon.windowmanager",
    "name": "Window Manager",
    "package": "windowmanager",
    "exe_path": "bin/sentinel-windowmanager.exe",
    "version": "1.0.0",
    "repo": "https://github.com/The-Ico2/sentinel-windowmanager",
    "author": {
        "Ico2": "https://github.com/The-Ico2"
    }
}
"#;
    match fs::write(&path, content) {
        Ok(_) => info!("[{}] Created addon.json", ADDON_NAME),
        Err(e) => warn!("[{}] Failed to create addon.json: {e}", ADDON_NAME),
    }
}

fn scaffold_config_yaml(addon_dir: &PathBuf) {
    let path = addon_dir.join("config.yaml");
    if path.exists() { return; }

    let content = r#"update_check: true
debug: false
log_level: warn

universal:
  exclude_processes:
    - "ShellExperienceHost.exe"
    - "taskmgr.exe"
    - "explorer.exe"
    - "systemsettings.exe"
    - "steamwebhelper.exe"
    - "msiexec.exe"

window_manager:
  enabled: true
  manager_type: tiling
  monitor_index:
    - "*"
  animation:
    enabled: true
    duration: 150
  styling:
    gap:
      space: 5
      behavior: "Shared"
  events:
    debounce_ms: 500

  filters:
    min_width: 1
    min_height: 1

    include_processes: []

    exclude_processes:
      - "ShellExperienceHost.exe"
      - "taskmgr.exe"
      - "explorer.exe"
      - "systemsettings.exe"
      - "steamwebhelper.exe"
      - "msiexec.exe"

    exclude_classes:
      - "Shell_TrayWnd"
      - "Progman"
      - "WorkerW"
      - "Windows.UI.Core"
      - "ApplicationFrameWindow"

    exclude_titles:
      - "Program Manager"
      - "NVIDIA GeForce Overlay"
      - "Windows Input Experience"
      - "Task Manager"
      - "Settings"
      - "PowerToys Quick Access"
"#;
    match fs::write(&path, content) {
        Ok(_) => info!("[{}] Created config.yaml", ADDON_NAME),
        Err(e) => warn!("[{}] Failed to create config.yaml: {e}", ADDON_NAME),
    }
}

fn scaffold_schema_yaml(addon_dir: &PathBuf) {
    let path = addon_dir.join("schema.yaml");
    if path.exists() { return; }

    let content = r#"version: "1.0"
ui:
  sections:
    - title: "General"
      path: "."
      fields:
        - path: "update_check"
          label: "Check for updates"
          control: "toggle"
        - path: "debug"
          label: "Debug mode"
          control: "toggle"
        - path: "log_level"
          label: "Log Level"
          control: "dropdown"
          options: ["trace", "debug", "info", "warn", "error"]

    - title: "Universal"
      path: "universal"
      fields:
        - path: "exclude_processes"
          label: "Global Excluded Processes"
          control: "text_list"

    - title: "Window Manager"
      path: "window_manager"
      fields:
        - path: "enabled"
          label: "Enabled"
          control: "toggle"
        - path: "manager_type"
          label: "Manager Type"
          control: "dropdown"
          options: ["tiling", "floating"]
        - path: "monitor_index"
          label: "Target Monitors"
          control: "text_list"

      sections:
        - title: "Animation"
          path: "animation"
          fields:
            - path: "enabled"
              label: "Enabled"
              control: "toggle"
            - path: "duration"
              label: "Duration (ms)"
              control: "number_range"
              min: 0
              max: 1000
              step: 1

        - title: "Styling"
          path: "styling.gap"
          fields:
            - path: "space"
              label: "Gap Size"
              control: "number_range"
              min: 0
              max: 80
              step: 1
            - path: "behavior"
              label: "Gap Behavior"
              control: "dropdown"
              options: ["Shared", "PerWindow"]

        - title: "Events"
          path: "events"
          fields:
            - path: "debounce_ms"
              label: "Debounce (ms)"
              control: "number_range"
              min: 0
              max: 3000
              step: 10

        - title: "Filters"
          path: "filters"
          fields:
            - path: "min_width"
              label: "Minimum Width"
              control: "number_range"
              min: 0
              max: 4000
              step: 1
            - path: "min_height"
              label: "Minimum Height"
              control: "number_range"
              min: 0
              max: 4000
              step: 1
            - path: "include_processes"
              label: "Included Processes"
              control: "text_list"
            - path: "exclude_processes"
              label: "Excluded Processes"
              control: "text_list"
            - path: "exclude_classes"
              label: "Excluded Classes"
              control: "text_list"
            - path: "exclude_titles"
              label: "Excluded Titles"
              control: "text_list"
"#;
    match fs::write(&path, content) {
        Ok(_) => info!("[{}] Created schema.yaml", ADDON_NAME),
        Err(e) => warn!("[{}] Failed to create schema.yaml: {e}", ADDON_NAME),
    }
}

fn scaffold_options_html(options_dir: &PathBuf) {
    let pages = [
        ("settings.html", "Window Manager - Settings", "settings-root"),
        ("discover.html", "Window Manager - Discover", "discover-root"),
        ("editor.html",   "Window Manager - Editor",   "editor-root"),
        ("library.html",  "Window Manager - Library",  "library-root"),
    ];
    for (file, title, root_id) in &pages {
        let path = options_dir.join(file);
        if path.exists() { continue; }
        let content = format!(
r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>{title}</title>
<link rel="stylesheet" href="./options.css" />
<script src="./options.js"></script>
</head>
<body>
<main class="page">
<div id="{root_id}"></div>
</main>
</body>
</html>
"#);
        match fs::write(&path, content) {
            Ok(_) => info!("[{}] Created options/{file}", ADDON_NAME),
            Err(e) => warn!("[{}] Failed to create options/{file}: {e}", ADDON_NAME),
        }
    }
}
