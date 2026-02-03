// Window Operations Module
// Handles window enumeration, filtering, and positioning

use crate::{info, DEBUG_NAME};
use crate::types::{DisplayInfo, ManagedWindow, WindowFilters, WindowManagerConfig};
use crate::layout::LayoutStrategy;
use crate::animations::animate_windows_batched;
use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::*,
        System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT},
    },
};
use std::mem;

const DEBUG_SUBTAG: &str = "WINDOW_OPS";
const DEFAULT_ANIMATION_DURATION_MS: u64 = 300;  // 300ms smooth animation

/// Get window title
pub fn get_window_title(hwnd: HWND) -> String {
    unsafe {
        let mut buffer: [u16; 512] = [0; 512];
        let len = GetWindowTextW(hwnd, &mut buffer);
        if len > 0 {
            String::from_utf16_lossy(&buffer[..len as usize])
        } else {
            String::new()
        }
    }
}

/// Get window class name
pub fn get_window_class_name(hwnd: HWND) -> String {
    unsafe {
        let mut buffer: [u16; 256] = [0; 256];
        let len = GetClassNameW(hwnd, &mut buffer);
        if len > 0 {
            String::from_utf16_lossy(&buffer[..len as usize])
        } else {
            String::new()
        }
    }
}

/// Get process name for a window
pub fn get_process_name(hwnd: HWND) -> String {
    unsafe {
        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        
        if process_id == 0 {
            return String::new();
        }

        let process_handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
            Ok(handle) => handle,
            Err(_) => return String::new(),
        };

        let mut buffer: [u16; 512] = [0; 512];
        let mut size = buffer.len() as u32;
        
        match QueryFullProcessImageNameW(process_handle, PROCESS_NAME_FORMAT(0), PWSTR(buffer.as_mut_ptr()), &mut size) {
            Ok(_) => {
                let path = String::from_utf16_lossy(&buffer[..size as usize]);
                // Extract just the filename from the full path
                path.split('\\').last().unwrap_or("").to_string()
            }
            Err(_) => String::new(),
        }
    }
}

/// Check if window should be managed based on filters
pub fn should_manage_window(hwnd: HWND, filters: &WindowFilters) -> bool {
    unsafe {
        // Check if window is visible
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        // Ignore minimized or maximized windows
        if IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool() {
            return false;
        }

        // Check if it's a normal window (not a child window)
        let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
        if !style.contains(WS_VISIBLE) || style.contains(WS_CHILD) {
            return false;
        }

        // Ignore tool windows and owned windows (sub-windows/dialogs)
        let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
        if ex_style.contains(WS_EX_TOOLWINDOW) {
            return false;
        }
        if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
            if owner.0 != std::ptr::null_mut() {
                return false;
            }
        }

        // Get window rect to check size and position
        let mut rect: RECT = mem::zeroed();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }

        // Note: We don't filter by minimum size here because the layout algorithm
        // will assign sizes. Filtering here prevents managing 6+ windows on a single monitor.
        // Windows that are too small will be handled during layout application.

        // Get window info
        let title = get_window_title(hwnd);
        let class_name = get_window_class_name(hwnd);
        let process_name = get_process_name(hwnd);

        // Ignore common installer dialogs
        if class_name == "#32770" {
            let title_lower = title.to_lowercase();
            if title_lower.contains("installer") || title_lower.contains("setup") {
                return false;
            }
        }

        // Check exclude classes
        if let Some(ref exclude_classes) = filters.exclude_classes {
            for excluded in exclude_classes {
                if class_name.to_lowercase().contains(&excluded.to_lowercase()) {
                    return false;
                }
            }
        }

        // Check exclude titles
        if let Some(ref exclude_titles) = filters.exclude_titles {
            for excluded in exclude_titles {
                if title.to_lowercase().contains(&excluded.to_lowercase()) {
                    return false;
                }
            }
        }

        // Check exclude processes
        if let Some(ref exclude_processes) = filters.exclude_processes {
            for excluded in exclude_processes {
                if process_name.to_lowercase() == excluded.to_lowercase() {
                    return false;
                }
            }
        }

        // Check include processes (if specified)
        if let Some(ref include_processes) = filters.include_processes {
            if !include_processes.is_empty() {
                let mut found = false;
                for included in include_processes {
                    if process_name.to_lowercase() == included.to_lowercase() {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
        }

        true
    }
}

/// Enumerate all manageable windows on a specific monitor
pub fn enumerate_windows(display: &DisplayInfo, filters: &WindowFilters) -> Vec<ManagedWindow> {
    unsafe {
        let display_rect = RECT {
            left: display.x,
            top: display.y,
            right: display.x + display.width,
            bottom: display.y + display.height,
        };

        // Callback data structure
        struct EnumData {
            windows: Vec<ManagedWindow>,
            display_rect: RECT,
            filters: WindowFilters,
        }

        let mut data = EnumData {
            windows: Vec::new(),
            display_rect,
            filters: filters.clone(),
        };

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let data = &mut *(lparam.0 as *mut EnumData);
            
            // Check if window should be managed
            if !should_manage_window(hwnd, &data.filters) {
                return true.into();
            }

            // Get window rect
            let mut rect: RECT = mem::zeroed();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return true.into();
            }

            // Ignore fullscreen windows on this display
            let is_fullscreen = rect.left <= data.display_rect.left + 1
                && rect.top <= data.display_rect.top + 1
                && rect.right >= data.display_rect.right - 1
                && rect.bottom >= data.display_rect.bottom - 1;
            if is_fullscreen {
                return true.into();
            }

            // Check if window is on this monitor (at least partially)
            let window_center_x = (rect.left + rect.right) / 2;
            let window_center_y = (rect.top + rect.bottom) / 2;
            
            if window_center_x >= data.display_rect.left 
                && window_center_x < data.display_rect.right
                && window_center_y >= data.display_rect.top
                && window_center_y < data.display_rect.bottom {
                
                let managed_window = ManagedWindow {
                    hwnd,
                    title: get_window_title(hwnd),
                    class_name: get_window_class_name(hwnd),
                    process_name: get_process_name(hwnd),
                };

                data.windows.push(managed_window);
            }

            true.into()
        }

        let _ = EnumWindows(Some(enum_callback), LPARAM(&mut data as *mut _ as isize));
        
        // Sort windows by HWND (stable, persistent ID) to maintain consistent ordering
        // This prevents focused windows from changing positions due to Z-order changes
        // HWND is a stable handle that doesn't change when windows are focused
        data.windows.sort_by_key(|w| w.hwnd.0 as isize);
        
        data.windows
    }
    .into_iter()
    .collect::<Vec<ManagedWindow>>()
}

/// Apply layout to windows with smooth synchronized animations
pub fn apply_layout(
    windows: &[ManagedWindow],
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) {
    info!("[{}][{}] Applying layout to {} windows", DEBUG_NAME, DEBUG_SUBTAG, windows.len());

    let animation_enabled = config.animation_enabled.unwrap_or(false);
    let animation_duration = config.animation_duration_ms.unwrap_or(DEFAULT_ANIMATION_DURATION_MS);

    // Calculate target layouts for all windows
    let mut target_layouts = Vec::new();
    for (idx, window) in windows.iter().enumerate() {
        let rect = layout_strategy.calculate_layout(display, windows.len(), idx, config);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        
        info!("[{}][{}] Layout window '{}' to ({}, {}) with size {}x{}", 
              DEBUG_NAME, DEBUG_SUBTAG, window.title, rect.left, rect.top, width, height);
        
        target_layouts.push((window.hwnd, rect));
    }

    if animation_enabled {
        // Collect current positions for all windows
        let mut animation_data = Vec::new();
        unsafe {
            for (hwnd, target_rect) in &target_layouts {
                let mut current_rect: RECT = mem::zeroed();
                if GetWindowRect(*hwnd, &mut current_rect).is_ok() {
                    // Only animate if position will actually change
                    if current_rect != *target_rect {
                        animation_data.push((*hwnd, current_rect, *target_rect));
                    } else {
                        // Position already correct, just apply it
                        let width = target_rect.right - target_rect.left;
                        let height = target_rect.bottom - target_rect.top;
                        let _ = SetWindowPos(
                            *hwnd,
                            None,
                            target_rect.left,
                            target_rect.top,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_DRAWFRAME,
                        );
                    }
                }
            }
        }

        // Animate all windows together synchronously
        if !animation_data.is_empty() {
            animate_windows_batched(&animation_data, animation_duration);
        }
    } else {
        // Instant positioning - apply all at once atomically
        unsafe {
            if !target_layouts.is_empty() {
                if let Ok(hdwp) = BeginDeferWindowPos(target_layouts.len() as i32) {
                    let mut hdwp_result = hdwp;

                    for (hwnd, rect) in &target_layouts {
                        let width = rect.right - rect.left;
                        let height = rect.bottom - rect.top;

                        if let Ok(new_hdwp) = DeferWindowPos(
                            hdwp_result,
                            *hwnd,
                            None,
                            rect.left,
                            rect.top,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_DRAWFRAME,
                        ) {
                            hdwp_result = new_hdwp;
                        }
                    }

                    let _ = EndDeferWindowPos(hdwp_result);
                }
            }
        }
    }
}

/// Retile all windows on a monitor
pub fn retile_windows(
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) {
    let default_filters = WindowFilters::default();
    let filters = config.filters.as_ref().unwrap_or(&default_filters);
    let windows = enumerate_windows(display, filters);
    
    if windows.is_empty() {
        info!("[{}][{}] No windows to tile", DEBUG_NAME, DEBUG_SUBTAG);
        return;
    }

    apply_layout(&windows, display, config, layout_strategy);
}
