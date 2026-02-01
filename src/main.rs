// ~/Sentinel/sentinel-addons/windowmanager/src/main.rs

#![windows_subsystem = "windows"] 

mod logging;
mod ipc;
mod widget;
mod styling;
mod layout;
mod config_yaml;
mod types;

use styling::apply_all_styling;
use layout::calculate_layout;
use config_yaml::{load_bar_styling_or_default, get_widgets_for_bar_or_default, is_update_check_enabled, BarStyling, BarWidgets};

use crate::widget::widget_loader::{load_widgets, spawn_widgets};
use crate::ipc::ipc_connector;
use crate::types::{MonitorInfo, IpcMonitorsResponse};

// serde not required here
use std::{mem::size_of, ptr::null_mut, time::Duration, thread};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::*,
            WindowsAndMessaging::*,
        },
        Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT},
    },
};
use std::sync::{Arc, Mutex};

/* ========================= GLOBAL BAR CONFIG ========================= */
static mut BAR_HEIGHT: i32 = 42;
static mut EDGE_TOP: bool = true;
static CLASS_NAME: &str = "SentinelStatusBarWndClass";

/* ========================= APP STATE ========================= */
#[derive(Clone)]
struct AppState {
    callback_msg: u32,
    monitor: MonitorInfo,
    widgets: BarWidgets,
    style: BarStyling,
}

// MonitorInfo and IpcMonitorsResponse moved to `src/types.rs`

/* ========================= WINDOW PROCEDURE ========================= */

fn bar_height(styling: &BarStyling, scale: f64) -> i32 {
    let h = styling.dimensions._height; 
    let base = if h > 0 { h } else { unsafe { BAR_HEIGHT } }; (base as f64 * scale).round() as i32 }

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let state = get_state(hwnd);

        if msg == WM_NCCREATE {
        let createstruct = &*(lparam.0 as *const CREATESTRUCTW);
        let monitor = (*(createstruct.lpCreateParams as *const MonitorInfo)).clone();

        // Use configured styling/widgets when present, otherwise fall back to defaults
        let style = load_bar_styling_or_default(monitor.clone());
        let widgets = get_widgets_for_bar_or_default(monitor.clone());

        set_state(hwnd, Some(AppState {
            callback_msg: WM_USER + 1,
            monitor,
            widgets,
            style,
        }));

        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    if let Some(ref st) = state {
        if msg == st.callback_msg {
            if wparam.0 as u32 == ABN_POSCHANGED {
                set_appbar_pos(hwnd);
            }
            return LRESULT(0);
        }
    }

    match msg {
        WM_CREATE => {
            register_appbar(hwnd);
            set_appbar_pos(hwnd);
            let state = get_state(hwnd).unwrap();
            apply_all_styling(hwnd, &state.style, &state.monitor, None);
            // Start widget processes/watchers and embed HTML/widgets for this bar
            let layout = calculate_layout(
                state.monitor.width,
                bar_height(&state.style, state.monitor.scale),
                &state.style,
                &state.widgets,
            );
            let children = spawn_widgets(hwnd, &state.widgets, &layout);
            if !children.is_empty() {
                info!("[WIDGET] Spawned {} external widget processes for this bar", children.len());
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let _ = BeginPaint(hwnd, &mut ps);
            if let Some(state) = get_state(hwnd) {
                let layout = calculate_layout(
                    state.monitor.width,
                    bar_height(&state.style, state.monitor.scale),
                    &state.style,
                    &state.widgets,
                );
                // Use layout to drive placement in future; log now to mark as used
                info!("[LAYOUT] computed layout: {:?}", layout);

                load_widgets(&state.widgets);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            unregister_appbar(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/* =========================
   IPC MONITORS
   ========================= */
fn get_monitors_via_ipc() -> Option<Vec<MonitorInfo>> {
    info!("[IPC] Requesting monitors via pipe");

    // Use the generalized IPC request
    let req = serde_json::json!({
        "ns": "sysdata",
        "cmd": "get_displays",
        "args": null
    });

    if let Some(resp) = ipc_connector::send_ipc_request(&req) {
        info!("[IPC] Received IPC response, parsing JSON");

        // Deserialize the response into the expected struct
        match serde_json::from_value::<IpcMonitorsResponse>(resp) {
            Ok(parsed) if parsed.ok => {
                info!("[IPC] Monitors parsed successfully: {:?}", parsed.data);
                parsed.data
            }
            Ok(parsed) => {
                error!("[IPC] IPC error response: {:?}", parsed.error);
                None
            }
            Err(e) => {
                error!("[IPC] Failed to parse IPC JSON: {e}");
                None
            }
        }
    } else {
        warn!("[IPC] No IPC response received");
        None
    }
}


/* =========================
   UTILITIES
   ========================= */
fn to_wstring(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/* =========================
   WINDOWS / STATE HELPERS
   ========================= */

unsafe fn get_state(hwnd: HWND) -> Option<AppState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
    if ptr.is_null() { None } else { Some((*ptr).clone()) }
}

unsafe fn set_state(hwnd: HWND, state: Option<AppState>) {
    let old = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
    if !old.is_null() {
        drop(Box::from_raw(old));
    }
    let new_ptr = state.map(|s| Box::into_raw(Box::new(s))).unwrap_or(null_mut());
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, new_ptr as isize);
}

unsafe fn register_appbar(hwnd: HWND) {
    info!("[APPBAR] Registering appbar");
    let mut abd = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: WM_USER + 1,
        ..Default::default()
    };
    SHAppBarMessage(ABM_NEW, &mut abd);
}

unsafe fn unregister_appbar(hwnd: HWND) {
    info!("[APPBAR] Unregistering appbar");
    let mut abd = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        ..Default::default()
    };
    SHAppBarMessage(ABM_REMOVE, &mut abd);
}

unsafe fn set_appbar_pos(hwnd: HWND) {
    let state = get_state(hwnd).expect("missing AppState");

    let m = &state.monitor;
    let bar_h = bar_height(&state.style, state.monitor.scale);
    let rc = RECT {
        left: m.x + state.style.margin._left,
        right: m.x + m.width - state.style.margin._right,
        top: if EDGE_TOP { m.y + state.style.margin._top } else { m.y + m.height - bar_h - state.style.margin._bottom },
        bottom: if EDGE_TOP { m.y + state.style.margin._top + bar_h } else { m.y + m.height - state.style.margin._bottom },
    };

    let mut abd = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uEdge: if EDGE_TOP { ABE_TOP } else { ABE_BOTTOM },
        rc,
        ..Default::default()
    };

    SHAppBarMessage(ABM_QUERYPOS, &mut abd);
    SHAppBarMessage(ABM_SETPOS, &mut abd);

    let width = abd.rc.right - abd.rc.left;
    let height = abd.rc.bottom - abd.rc.top;

    let _ = SetWindowPos(
        hwnd,
        Some(HWND(null_mut())),
        abd.rc.left,
        abd.rc.top,
        width,
        height,
        SWP_NOZORDER,
    );

    let _ = ShowWindow(hwnd, SW_SHOW);
    info!("[APPBAR] Appbar positioned at ({}, {}), size {}x{} on monitor {}", abd.rc.left, abd.rc.top, width, height, m.id);
}

/* =========================
   CONFIG FILE WATCHER
   ========================= */

fn start_config_watcher() {
    thread::spawn(move || {
        let config_path = match std::env::var("USERPROFILE") {
            Ok(profile) => format!("{}/.Sentinel/Addons/statusbar/config.yaml", profile),
            Err(_) => "./config.yaml".to_string(),
        };

        let debounce_timer = Arc::new(Mutex::new(None::<std::time::Instant>));
        let mut last_modified = std::fs::metadata(&config_path)
            .and_then(|m| m.modified())
            .ok();

        info!("[CONFIG] Watching for changes to: {}", config_path);

        loop {
            thread::sleep(Duration::from_millis(500));

            // Check if file has been modified
            if let Ok(metadata) = std::fs::metadata(&config_path) {
                if let Ok(modified) = metadata.modified() {
                    if Some(modified) != last_modified {
                        last_modified = Some(modified);
                        info!("[CONFIG] Detected change to config.yaml");
                        
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
                                            // Send reload request to backend via IPC
                                            let req = serde_json::json!({
                                                "ns": "addon",
                                                "cmd": "reload",
                                                "args": {
                                                    "addon_name": "Statusbar"
                                                }
                                            });
                                            
                                            match ipc_connector::send_ipc_request(&req) {
                                                Some(_) => info!("[CONFIG] Reload request sent to backend"),
                                                None => warn!("[CONFIG] Failed to send reload request to backend"),
                                            }
                                        } else {
                                            info!("[CONFIG] Config changed but update_check is disabled");
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

/* =========================
   MAIN
   ========================= */

fn main() -> windows::core::Result<()> {
    logging::init(true);

    unsafe {
        // Ensure WebView2 runtime is installed (attempt bootstrap if missing)
        if !crate::widget::hosts::web_view_host::ensure_webview2_runtime() {
            error!("WebView2 runtime missing and installer failed — HTML embedding will not work");
        }
        let hinstance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = to_wstring(CLASS_NAME);

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class.as_ptr()),
            style: CS_HREDRAW | CS_VREDRAW,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        // get monitors from IPC
        let ipc_monitors = get_monitors_via_ipc().unwrap_or_default();

        // Build a real indexed list
        let monitors: Vec<MonitorInfo> = ipc_monitors.into_iter().collect();

        if monitors.is_empty() {
            error!("[MAIN] No monitors received from IPC");
            return Ok(());
        }

        info!("[MAIN] Creating windows for {} monitor(s)", monitors.len());

        // For each monitor, create a window only if a matching enabled bar exists
        for (idx, monitor) in monitors.iter().cloned().enumerate() {
            // if there's no enabled bar configured for this monitor, skip creating a window
            if config_yaml::load_bar_styling(monitor.clone()).is_none() {
                info!("[MAIN] No enabled bar configured for monitor {}, skipping", monitor.id);
                continue;
            }
            let boxed = Box::new(monitor.clone());

            // Use configured styling or defaults to compute initial height
            let styling = load_bar_styling_or_default(monitor.clone());

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0),
                PCWSTR(class.as_ptr()),
                PCWSTR(to_wstring("Sentinel Status Bar").as_ptr()),
                WINDOW_STYLE(WS_POPUP.0),
                0, 0, 1, bar_height(&styling, monitor.scale),
                Some(HWND(null_mut())),
                Some(HMENU(null_mut())),
                Some(hinstance),
                Some(Box::into_raw(boxed) as _),
            );

            if let Err(e) = hwnd {
                error!("[MAIN] Failed to create window (idx={}): {:?}", idx, e);
            } else {
                info!("[MAIN] Created window for monitor {} (idx={})", monitor.id, idx);
            }
        }

        // Start config file watcher (watches all windows)
        start_config_watcher();

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}