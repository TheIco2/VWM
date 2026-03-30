// ~/OpenDesktop/od-addons/windowmanager/src/bootstrap.rs

use std::fs;
use std::path::PathBuf;
use crate::ADDON_NAME;
use crate::{info, warn};

const EXE_NAME: &str = "od-windowmanager.exe";

/// Check if OpenDesktop.exe (the backend) is running; if not, start it.
fn ensure_backend_running() {
    info!("[{}] Checking if OpenDesktop.exe is running...", ADDON_NAME);
    let backend_running = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq OpenDesktop.exe", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("OpenDesktop.exe"))
        .unwrap_or(false);

    if backend_running {
        info!("[{}] OpenDesktop.exe is already running", ADDON_NAME);
        return;
    }

    warn!("[{}] OpenDesktop.exe is NOT running, attempting to start it", ADDON_NAME);
    let Some(home) = std::env::var("USERPROFILE").ok() else {
        warn!("[{}] Cannot resolve USERPROFILE to find OpenDesktop.exe", ADDON_NAME);
        return;
    };
    let backend_exe = PathBuf::from(&home).join("ProjectOpen").join("OpenDesktop").join("OpenDesktop.exe");
    if !backend_exe.exists() {
        warn!("[{}] Backend not found at {}", ADDON_NAME, backend_exe.display());
        return;
    }
    match std::process::Command::new(&backend_exe).spawn() {
        Ok(_) => {
            info!("[{}] Started OpenDesktop.exe from {}", ADDON_NAME, backend_exe.display());
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        Err(e) => warn!("[{}] Failed to start OpenDesktop.exe: {e}", ADDON_NAME),
    }
}

pub fn bootstrap_addon() {
    info!("[{}] === Bootstrap starting ===", ADDON_NAME);
    info!("[{}] Current exe: {:?}", ADDON_NAME, std::env::current_exe());

    ensure_backend_running();

    let config = crate::installer::InstallerConfig::addon("OpenDesktop", ADDON_NAME)
        .exe_name(EXE_NAME)
        .addon_subdirs(&["options"]);

    // Scaffold default files (so the installed copy has them too)
    if let Some(addon_dir) = crate::installer::install_dir(&config) {
        let options_dir = addon_dir.join("options");
        scaffold_addon_json(&addon_dir);
        scaffold_config_yaml(&addon_dir);
        scaffold_schema_yaml(&addon_dir);
        scaffold_options_html(&options_dir);
        scaffold_options_assets(&options_dir);
        info!("[{}] Scaffolding complete", ADDON_NAME);
    }

    // Self-install: copy exe to install dir and relaunch if needed
    fn log_fn(level: crate::installer::LogLevel, msg: &str) {
        match level {
            crate::installer::LogLevel::Info  => crate::info!("{}", msg),
            crate::installer::LogLevel::Warn  => crate::warn!("{}", msg),
            crate::installer::LogLevel::Error => crate::error!("{}", msg),
        }
    }
    crate::installer::bootstrap(&config, log_fn);
}

fn scaffold_addon_json(addon_dir: &PathBuf) {
    let path = addon_dir.join("addon.json");
    if path.exists() { return; }

    let content = r#"{
    "id": "od.addon.windowmanager",
    "name": "Window Manager",
    "package": "windowmanager",
    "exe_path": "bin/od-windowmanager.exe",
    "version": "1.0.0",
    "repo": "https://github.com/The-Ico2/od-windowmanager",
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

    let content = r#"settings:
  development:
    update_check: true
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
    - title: "Development"
      path: "settings.development"
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
      path: "settings.universal"
      fields:
        - path: "exclude_processes"
          label: "Global Excluded Processes"
          control: "text_list"

    - title: "Window Manager"
      path: "settings.window_manager"
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
  let pages: [(&str, &str, &str); 0] = [];
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

    const OPTIONS_CSS: &str = include_str!("../options/options.css");
    const OPTIONS_JS: &str = include_str!("../options/options.js");

    fn scaffold_options_assets(options_dir: &PathBuf) {
      let files: [(&str, &str); 2] = [
        ("options.css", OPTIONS_CSS),
        ("options.js", OPTIONS_JS),
      ];

      for (name, content) in files {
        let path = options_dir.join(name);
        if path.exists() { continue; }
        match fs::write(&path, content) {
          Ok(_) => info!("[{}] Created options/{name}", ADDON_NAME),
          Err(e) => warn!("[{}] Failed to create options/{name}: {e}", ADDON_NAME),
        }
      }
    }
