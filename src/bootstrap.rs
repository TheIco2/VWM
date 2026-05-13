// ~/VEIL/veil-addons/windowmanager/src/bootstrap.rs

use std::fs;
use std::path::PathBuf;
use crate::ADDON_NAME;
use crate::{info, warn};

const EXE_NAME: &str = "veil-windowmanager.exe";
const STANDALONE_EXE_NAME: &str = "vwm-s.exe";
const STANDALONE_APP_FOLDER: &str = "WindowManager";
const START_MENU_LINK_NAME: &str = "Window Manager.lnk";

/// Check if VEIL.exe (the backend) is running; if not, start it.
fn ensure_backend_running() {
    info!("[{}] Checking if VEIL.exe is running...", ADDON_NAME);
    let backend_running = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq VEIL.exe", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("VEIL.exe"))
        .unwrap_or(false);

    if backend_running {
        info!("[{}] VEIL.exe is already running", ADDON_NAME);
        return;
    }

    warn!("[{}] VEIL.exe is NOT running, attempting to start it", ADDON_NAME);
    let Some(home) = std::env::var("USERPROFILE").ok() else {
        warn!("[{}] Cannot resolve USERPROFILE to find VEIL.exe", ADDON_NAME);
        return;
    };
    let backend_exe = PathBuf::from(&home).join("VEIL").join("Core").join("VEIL.exe");
    if !backend_exe.exists() {
        warn!("[{}] Backend not found at {}", ADDON_NAME, backend_exe.display());
        return;
    }
    match std::process::Command::new(&backend_exe).spawn() {
        Ok(_) => {
            info!("[{}] Started VEIL.exe from {}", ADDON_NAME, backend_exe.display());
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        Err(e) => warn!("[{}] Failed to start VEIL.exe: {e}", ADDON_NAME),
    }
}

pub fn bootstrap_addon() {
    info!("[{}] === Bootstrap starting ===", ADDON_NAME);
    info!("[{}] Current exe: {:?}", ADDON_NAME, std::env::current_exe());

    ensure_backend_running();

    let config = crate::installer::InstallerConfig::addon("VEIL", ADDON_NAME)
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

  pub fn bootstrap_standalone() {
    info!("[{}] === Standalone bootstrap starting ===", ADDON_NAME);
    info!("[{}] Current exe: {:?}", ADDON_NAME, std::env::current_exe());

    let config = crate::installer::InstallerConfig::standalone("VEIL", STANDALONE_APP_FOLDER)
      .exe_name(STANDALONE_EXE_NAME)
      .no_exit_after_relaunch();

    if let Some(standalone_dir) = crate::installer::install_dir(&config) {
      scaffold_info_json(&standalone_dir);
      scaffold_standalone_config_yaml(&standalone_dir);
      info!("[{}] Standalone scaffolding complete", ADDON_NAME);
    }

    fn log_fn(level: crate::installer::LogLevel, msg: &str) {
      match level {
        crate::installer::LogLevel::Info  => crate::info!("{}", msg),
        crate::installer::LogLevel::Warn  => crate::warn!("{}", msg),
        crate::installer::LogLevel::Error => crate::error!("{}", msg),
      }
    }

    let result = crate::installer::bootstrap(&config, log_fn);

    if let Some(standalone_dir) = crate::installer::install_dir(&config) {
      let target_exe = standalone_dir.join(STANDALONE_EXE_NAME);
      ensure_start_menu_shortcut(&target_exe, &standalone_dir);

      let run_on_startup = read_run_on_startup(&standalone_dir.join("config.yaml"));
      ensure_startup_shortcut(&target_exe, &standalone_dir, run_on_startup);
    }

    if matches!(result, crate::installer::BootstrapResult::Relaunched) {
      std::process::exit(0);
    }
  }

  fn read_run_on_startup(config_path: &PathBuf) -> bool {
    let Ok(text) = fs::read_to_string(config_path) else {
      return true;
    };

    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
      return true;
    };

    value
      .get("settings")
      .and_then(|s| s.get("run_on_startup"))
      .and_then(|v| v.as_bool())
      .or_else(|| value.get("run_on_startup").and_then(|v| v.as_bool()))
      .unwrap_or(true)
  }

  fn ensure_start_menu_shortcut(target_exe: &PathBuf, working_dir: &PathBuf) {
    let appdata = match std::env::var("APPDATA") {
      Ok(v) => PathBuf::from(v),
      Err(_) => return,
    };

    let start_menu_dir = appdata.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("VEIL");
    if let Err(e) = std::fs::create_dir_all(&start_menu_dir) {
      warn!("[{}] Failed to create Start Menu directory '{}': {}", ADDON_NAME, start_menu_dir.display(), e);
      return;
    }

    let shortcut_path = start_menu_dir.join(START_MENU_LINK_NAME);
    if create_windows_shortcut(&shortcut_path, target_exe, working_dir, "VEIL Window Manager") {
      info!("[{}] Ensured Start Menu shortcut at {}", ADDON_NAME, shortcut_path.display());
    }
  }

  fn ensure_startup_shortcut(target_exe: &PathBuf, working_dir: &PathBuf, enabled: bool) {
    let appdata = match std::env::var("APPDATA") {
      Ok(v) => PathBuf::from(v),
      Err(_) => return,
    };

    let startup_dir = appdata
      .join("Microsoft")
      .join("Windows")
      .join("Start Menu")
      .join("Programs")
      .join("Startup");

    if let Err(e) = std::fs::create_dir_all(&startup_dir) {
      warn!("[{}] Failed to create Startup directory '{}': {}", ADDON_NAME, startup_dir.display(), e);
      return;
    }

    let shortcut_path = startup_dir.join(START_MENU_LINK_NAME);

    if enabled {
      if create_windows_shortcut(&shortcut_path, target_exe, working_dir, "VEIL Window Manager") {
        info!("[{}] run_on_startup=true; ensured Startup shortcut at {}", ADDON_NAME, shortcut_path.display());
      }
    } else if shortcut_path.exists() {
      match std::fs::remove_file(&shortcut_path) {
        Ok(_) => info!("[{}] run_on_startup=false; removed Startup shortcut at {}", ADDON_NAME, shortcut_path.display()),
        Err(e) => warn!("[{}] Failed removing Startup shortcut '{}': {}", ADDON_NAME, shortcut_path.display(), e),
      }
    }
  }

  fn ps_quote(value: &str) -> String {
    value.replace('\'', "''")
  }

  fn create_windows_shortcut(shortcut_path: &PathBuf, target_exe: &PathBuf, working_dir: &PathBuf, description: &str) -> bool {
    let shortcut = ps_quote(&shortcut_path.display().to_string());
    let target = ps_quote(&target_exe.display().to_string());
    let working = ps_quote(&working_dir.display().to_string());
    let desc = ps_quote(description);

    let command = format!(
      "$WshShell = New-Object -ComObject WScript.Shell; \
       $Shortcut = $WshShell.CreateShortcut('{}'); \
       $Shortcut.TargetPath = '{}'; \
       $Shortcut.WorkingDirectory = '{}'; \
       $Shortcut.Description = '{}'; \
       $Shortcut.Save();",
      shortcut, target, working, desc
    );

    match std::process::Command::new("powershell")
      .arg("-NoProfile")
      .arg("-NonInteractive")
      .arg("-ExecutionPolicy")
      .arg("Bypass")
      .arg("-Command")
      .arg(command)
      .status()
    {
      Ok(status) if status.success() => true,
      Ok(status) => {
        warn!("[{}] Shortcut creation PowerShell exited with status {}", ADDON_NAME, status);
        false
      }
      Err(e) => {
        warn!("[{}] Failed to run PowerShell for shortcut creation: {}", ADDON_NAME, e);
        false
      }
    }
  }

fn scaffold_info_json(standalone_dir: &PathBuf) {
    let path = standalone_dir.join("info.json");
    if path.exists() { return; }

    let content = r#"{
    "id": "veil.app.windowmanager",
    "name": "Window Manager",
    "package": "windowmanager",
    "exe_path": "bin/vwm-s.exe",
    "version": "1.0.0",
    "repo": "https://github.com/The-Ico2/VWM",
    "authors": {
        "Ico2": "https://github.com/The-Ico2"
    }
}
"#;
    match fs::write(&path, content) {
        Ok(_) => info!("[{}] Created info.json", ADDON_NAME),
        Err(e) => warn!("[{}] Failed to create info.json: {e}", ADDON_NAME),
    }
}

fn scaffold_standalone_config_yaml(standalone_dir: &PathBuf) {
    let path = standalone_dir.join("config.yaml");
    if path.exists() { return; }

    let content = r#"settings:
  update_check: true
    debug: false
    log_level: warn
    run_on_startup: true

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
        space: 10
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
        Ok(_) => info!("[{}] Created standalone config.yaml", ADDON_NAME),
        Err(e) => warn!("[{}] Failed to create standalone config.yaml: {e}", ADDON_NAME),
    }
}

fn scaffold_addon_json(addon_dir: &PathBuf) {
    let path = addon_dir.join("addon.json");
    if path.exists() { return; }

    let content = r#"{
    "id": "veil.addon.windowmanager",
    "name": "Window Manager",
    "package": "windowmanager",
    "exe_path": "bin/veil-windowmanager.exe",
    "version": "1.0.0",
    "repo": "https://github.com/The-Ico2/veil-windowmanager",
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
