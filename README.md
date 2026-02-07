# Tiling Window Manager

A dynamic, multi-monitor window management addon for Sentinel that provides automatic window tiling, positioning, and organization with extensive customization options.

## Features

### Layout Managers

The window manager supports three distinct layout modes:

#### 1. **Tiling Layout** (Default)

- **BSP (Binary Space Partitioning)** algorithm for dynamic window arrangement
- Automatically splits screen space between windows
- Intelligent splitting direction based on available space (horizontal vs vertical)
- Supports both shared and per-window gap behaviors
- **Position-based window ordering** - windows maintain stable positions based on their screen coordinates
- **Drag-to-reorder** - physically drag windows to reposition them in the layout order
  - Swap functionality: dragging a window onto another window's position swaps their order
  - No focus-based reordering - windows stay where you place them
- **Per-monitor BSP state** - each monitor maintains its own independent window order
- **Smart monitor boundary detection** - prevents windows from jumping between monitors during drag operations
  - 50% overlap threshold on adjacent monitor edges prevents cross-monitor swapping
  - Any intersection allowed on isolated edges for flexible positioning

#### 2. **Floating Layout**

- Windows retain their natural positions
- No automatic repositioning
- Useful for non-tiled workflows

#### 3. **Stacking Layout**

- All windows stacked in the same position
- Maximizes screen real estate for focused work
- Quick switching between full-screen windows

### Window Animations

Smooth, configurable animations when windows are repositioned:

- **Toggle animations** on/off per configuration
- **Configurable duration** (default: 150ms)
- Native Windows animation support via `SetWindowPos` with `SWP_ASYNCWINDOWPOS`

### Gap Management

Flexible gap system with two distinct behaviors:

#### Per-Window Gap (Default)

- Each window has a full gap on all sides
- Interior gaps are naturally doubled where windows meet
- Clean, uniform spacing around every window

#### Shared Gap

- Interior gaps are shared between adjacent windows (halved)
- Edge gaps remain full-width for monitor boundaries
- More space-efficient for crowded layouts

### Window Filtering

Comprehensive filtering system to control which windows are managed:

#### Dimension Filters

- **`min_width`** - Minimum window width in pixels
- **`min_height`** - Minimum window height in pixels

#### Process Filters

- **`include_processes`** - Whitelist specific processes (when set, only these processes are managed)
- **`exclude_processes`** - Blacklist specific processes
  - Default excludes: `explorer.exe`, `taskmgr.exe`, `systemsettings.exe`, `steamwebhelper.exe`, `msiexec.exe`

#### Class Filters

- **`exclude_classes`** - Exclude windows by window class name
  - Default excludes: `Shell_TrayWnd`, `Progman`, `WorkerW`

#### Title Filters

- **`exclude_titles`** - Exclude windows by window title
  - Default excludes: "Program Manager", "Task Manager", "Settings", "Windows Input Experience", "PowerToys Quick Access"

### Automatic Filters

The window manager automatically excludes:

- Invisible windows
- Minimized windows
- Cloaked windows (e.g., virtual desktop windows on other desktops)
- Windows without `WS_VISIBLE` style
- Windows with `WS_EX_TOOLWINDOW` extended style
- Fullscreen windows (automatically detected and ignored)
- Popup windows
- Tool windows
- Windows without captions

### Multi-Monitor Support

Full multi-monitor capability:

- **Per-monitor configuration** - Each monitor can have its own layout type, gaps, filters, and settings
- **Monitor index selection** - Configure specific monitors or use `"*"` for all monitors
- **Independent BSP state** - Each monitor maintains its own window order
- **Intelligent monitor assignment** - Windows are assigned to monitors based on intersection area
- **Smart boundary detection** - Detects which monitor edges have adjacent monitors
  - Prevents accidental cross-monitor window jumping
  - Allows windows to extend beyond isolated edges without losing management
- **Dynamic monitor topology** - Automatically adapts to display configuration changes

### Window Events

Real-time window management via Windows event hooks:

- **Window creation/destruction** - Automatically retiles when windows appear or disappear
- **Window show/hide** - Responds to visibility changes
- **Drag detection** - Tracks when windows are being manually moved
- **Move/resize end events** - Updates layout after user repositions windows
- **Debounced retiling** - Configurable delay (default: 500ms) prevents excessive retiling
- **Rate-limited logging** - Prevents log spam from noisy windows (250ms window per event type)

### Configuration Hot-Reload

- **YAML file watching** - Automatically detects changes to `config.yaml`
- **Live updates** - Configuration changes apply immediately without restart
- **Cross-platform watching** - Uses `notify` crate for file system events

### IPC Integration

Seamless integration with Sentinel's IPC system:

- **Monitor information** - Retrieves display data via `get_displays` IPC call
- **Dynamic updates** - Responds to monitor configuration changes from Sentinel core

## Configuration

Configuration is managed via YAML file at `~/.Sentinel/addons/windowmanager/config.yaml`:

```yaml
update_check: true
debug: false
log_level: warn

universal:
  exclude_processes:
    - "ShellExperienceHost.exe"
    - "taskmgr.exe"

window_manager:
  enabled: true
  manager_type: tiling  # Options: tiling, floating, stacking
  monitor_index:
    - "*"  # All monitors, or specify indices: [0, 1, 2]
  
  animation:
    enabled: true
    duration: 150  # milliseconds
  
  styling:
    gap:
      space: 10  # pixels
      behavior: per_window  # Options: per_window, shared
  
  events:
    debounce_ms: 500  # milliseconds
  
  filters:
    min_width: 200  # optional
    min_height: 150  # optional
    exclude_processes:
      - "explorer.exe"
      - "steamwebhelper.exe"
    exclude_classes:
      - "Shell_TrayWnd"
    exclude_titles:
      - "Task Manager"
```

## Architecture

### Core Components

- **`main.rs`** - Entry point, IPC communication, event loop management
- **`window_ops.rs`** - Window enumeration, filtering, BSP state management, drag-to-reorder logic
- **`layout.rs`** - Layout strategy implementations (Tiling/BSP, Floating, Stacking)
- **`window_events.rs`** - Windows event hook system, debouncing, event management
- **`types.rs`** - Core data structures (DisplayInfo, WindowManagerConfig, ManagedWindow)
- **`config/`** - Configuration modules (animation, events, filters, styling)
- **`data_loaders/`** - YAML/JSON parsing and loading
- **`watchers.rs`** - File system watching for hot-reload
- **`ipc_connector.rs`** - Sentinel IPC communication

### Window Order Management

The window manager uses a sophisticated position-based ordering system:

1. **Enumeration** - Windows are enumerated with their screen positions
2. **Stable Sorting** - Multi-level sort: (y, x) position → existing order → hwnd
3. **Per-Monitor State** - Each monitor's window order stored in `HashMap<DisplayInfo.id, Vec<hwnd>>`
4. **Drag Detection** - `EVENT_SYSTEM_MOVESIZEEND` triggers order update
5. **Swap Logic** - Finds window under drop position and swaps order indices
6. **Retiling** - Layout recalculated with new order

### Monitor Boundary Detection

Smart boundary system prevents unwanted cross-monitor behavior:

1. **Adjacency Detection** - Checks if another monitor's edge aligns perfectly with current monitor's edge
2. **Directional Thresholds** - Applies 50% overlap requirement only on adjacent sides
3. **Isolated Edges** - Allows any intersection on sides without adjacent monitors
4. **Independent Checks** - Horizontal and vertical constraints evaluated separately

## Technical Details

- **Language**: Rust
- **Platform**: Windows (Win32 API)
- **Window Positioning**: `SetWindowPos` with `HWND_TOP` and `SWP_ASYNCWINDOWPOS`
- **Event System**: Windows Accessibility Event Hooks (`SetWinEventHook`)
- **Configuration**: YAML with `serde` deserialization
- **File Watching**: `notify` crate for cross-platform filesystem events
- **Threading**: Debounced retiling via spawned threads with mutex-protected state

## Building

```bash
cargo build --release
```

The compiled binary will be at `target/release/sentinel-windowmanager.exe`
