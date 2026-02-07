# 🪟 Sentinel Window Manager

![Sentinel Banner](https://img.shields.io/badge/Sentinel-WindowManager-blue) ![Rust](https://img.shields.io/badge/lang-Rust-orange) ![Windows](https://img.shields.io/badge/platform-Windows-brightgreen) ![Status](https://img.shields.io/badge/status-Beta-yellow)

**Dynamic, multi-monitor window management** for Sentinel — fast tiling, smooth animations, clean gaps, and **edge-drag resizing** in BSP layouts.

> ⚡ Modern tiling workflow with predictable placement, clean spacing, and per-monitor customization.

---

## 🚀 Features

| Feature                   | Description                                                                 | Status |
| ------------------------- | --------------------------------------------------------------------------- | ------ |
| **Tiling Layout (BSP)**   | Automatic BSP window arrangement with drag-to-reorder and per-monitor state | ✅     |
| **Floating Layout**       | Natural window positions for freeform workflows                             | ✅     |
| **Stacking Layout**       | Focused full-screen stack of windows                                        | ✅     |
| **Edge-Drag Resizing**    | Resize windows while maintaining BSP integrity                              | ✅     |
| **Smooth Animations**     | Configurable duration, toggleable                                           | ✅     |
| **Gap Management**        | Per-window or shared gaps for clean spacing                                 | ✅     |
| **Multi-Monitor Support** | Per-monitor layouts, smart boundary detection                               | ✅     |
| **Window Filtering**      | Dimension, process, class, title filters, auto exclusions                   | ✅     |
| **Hot-Reload Config**     | YAML updates live without restarting                                        | ✅     |
| **IPC Integration**       | Sentinel IPC-aware for dynamic updates                                      | ✅     |

---

## 🖼️ Layout Diagrams

### 1️⃣ Tiling Layout (BSP)

```pwsh
+----------------+----------------+
|       W1       |       W2       |
|                +-------+--------+
|                |  W3   |  W4    |
+----------------+-------+--------+
```

* Windows split dynamically by screen space
* Drag-to-reorder & swap supported
* Edge-drag resizing affects neighbors

### 2️⃣ Floating Layout

```pwsh
+----------------+----------------+
|       W1       |   (floating)   |
|     W2         |                |
+----------------+----------------+
```

* Freeform, natural positions
* No automatic tiling

### 3️⃣ Stacking Layout

```pwsh
+----------------+
|       W1       |
|       W2       |
|       W3       |
+----------------+
```

* Stack all windows for focus
* Quick full-screen switching

---

## ⚙️ Configuration

**Path:** `~/.Sentinel/addons/windowmanager/config.yaml`

<details>
<summary>Click to expand YAML</summary>

```yaml
update_check: true        # Check for addon updates
debug: false              # Enable verbose debug logging
log_level: warn           # trace | debug | info | warn | error

# Universal filters apply to all addons
universal:
  exclude_processes:      # Always ignore these processes
    - "ShellExperienceHost.exe"
    - "taskmgr.exe"
    - "systemsettings.exe"
    - "steamwebhelper.exe"
    - "msiexec.exe"

window_manager:
  enabled: true            # Master toggle for the window manager

  # Layout type: tiling | floating | stacking
  manager_type: tiling

  # Target monitors by index ("*" = all monitors)
  # Example: ["0", "1"] or ["*"]
  monitor_index:
    - "*"

  # Window animations when applying layout
  animation:
    enabled: true
    duration: 150          # milliseconds

  # Visual styling
  styling:
    gap:
      space: 10            # pixels between windows
      behavior: "Shared"   # Shared | PerWindow

  # Event handling
  events:
    debounce_ms: 500       # Delay between retiles after window events

  # Window filters (applied after universal filters)
  filters:
    # Minimum dimensions to manage
    min_width: 1
    min_height: 1

    # Include only these processes (empty = allow all)
    include_processes: []

    # Exclude specific processes (empty = exclude none)
    exclude_processes: []

    # Exclude window classes by partial match
    exclude_classes:
      - "Shell_TrayWnd"
      - "Progman"
      - "WorkerW"
      - "Windows.UI.Core"
      - "ApplicationFrameWindow"

    # Exclude windows by title substring
    exclude_titles:
      - "Program Manager"
      - "NVIDIA GeForce Overlay"
      - "Windows Input Experience"
      - "Task Manager"
      - "Settings"
      - "PowerToys Quick Access"
```

</details>

---

## 🏗️ Architecture Overview

```pwsh
Sentinel Window Manager
┌───────────────┐
│   main.rs     │ ← IPC & event loop
├───────────────┤
│ window_ops.rs │ ← enumeration, filtering, BSP logic
├───────────────┤
│   layout.rs   │ ← Tiling / Floating / Stacking
├───────────────┤
│window_events.rs│ ← Windows event hooks
├───────────────┤
│   types.rs    │ ← Core data structures
├───────────────┤
│  watchers.rs  │ ← Config hot-reload
├───────────────┤
│ ipc_connector │ ← Sentinel IPC
└───────────────┘
```

---

## 🔧 Multi-Monitor & Window Management

* **Per-monitor BSP state**
* **Intelligent monitor assignment**
* **Boundary-aware edge detection**
* **Dynamic topology adaptation**

### Window Filtering Highlights

* Dimensions: `min_width`, `min_height`
* Processes: `include` / `exclude`
* Classes: `exclude_classes`
* Titles: `exclude_titles`
* Automatic: hidden, minimized, cloaked, tooltips, fullscreen, popup, captionless

---

## 🛠️ Technical Stack

* **Language:** Rust
* **Platform:** Windows (Win32 API)
* **Window Positioning:** `SetWindowPos` (`HWND_TOP`, `SWP_ASYNCWINDOWPOS`)
* **Event System:** Windows Accessibility Event Hooks (`SetWinEventHook`)
* **Config:** YAML via `serde`
* **File Watching:** `notify` crate
* **Threading:** Debounced retiling with mutex-protected threads

---

## ⚡ Building

```bash
cargo build --release
```

**Output:** `target/release/sentinel-windowmanager.exe`
