// Window Operations Module
// Handles window enumeration, filtering, and positioning

use crate::{info, DEBUG_NAME};
use crate::types::{DisplayInfo, ManagedWindow, WindowFilters, WindowManagerConfig};
use crate::layout::LayoutStrategy;
use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::*,
        System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT},
        Graphics::Gdi::{RedrawWindow, InvalidateRect, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW, RDW_ALLCHILDREN, RDW_ERASE},
    },
};
use std::mem;
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashSet;

const DEBUG_SUBTAG: &str = "WINDOW_OPS";
const DEFAULT_ANIMATION_DURATION_MS: u64 = 300;  // 300ms to allow content to redraw
const ANIMATION_STEPS: u64 = 8;          // Reasonable default - prevents app crashes
const MAX_ANIMATION_STEPS: u64 = 30;     // Cap to prevent overwhelming application message queues
const MIN_FRAME_DELAY_MS: u64 = 10;      // Minimum 10ms between frames - apps need time to process messages

// Track windows currently being animated to prevent event-triggered retiles from interfering
static ANIMATING_WINDOWS: OnceLock<Arc<Mutex<HashSet<isize>>>> = OnceLock::new();

fn get_animating_windows() -> Arc<Mutex<HashSet<isize>>> {
    Arc::clone(ANIMATING_WINDOWS.get_or_init(|| Arc::new(Mutex::new(HashSet::new()))))
}

pub fn is_window_animating(hwnd: HWND) -> bool {
    match get_animating_windows().lock() {
        Ok(set) => set.contains(&(hwnd.0 as isize)),
        Err(_) => false,
    }
}

fn mark_window_animating(hwnd: HWND, animating: bool) {
    if let Ok(mut set) = get_animating_windows().lock() {
        let hwnd_val = hwnd.0 as isize;
        if animating {
            set.insert(hwnd_val);
        } else {
            set.remove(&hwnd_val);
        }
    }
}

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

/// Easing function for smooth animation (ease-out cubic)
/// Returns a value from 0.0 to 1.0 based on progress (0.0 to 1.0)
fn easing_ease_out_cubic(progress: f32) -> f32 {
    let p = progress;
    1.0 - (1.0 - p).powf(3.0)
}

/// Interpolate between two rectangles with easing
fn interpolate_rect(from: RECT, to: RECT, progress: f32) -> RECT {
    let eased = easing_ease_out_cubic(progress);
    RECT {
        left: (from.left as f32 + (to.left as f32 - from.left as f32) * eased) as i32,
        top: (from.top as f32 + (to.top as f32 - from.top as f32) * eased) as i32,
        right: (from.right as f32 + (to.right as f32 - from.right as f32) * eased) as i32,
        bottom: (from.bottom as f32 + (to.bottom as f32 - from.bottom as f32) * eased) as i32,
    }
}

/// Animate a window from its current position to target position smoothly
fn animate_window(hwnd: HWND, target_rect: RECT, duration_ms: u64) {
    // Extract raw handle to make it Send
    let hwnd_raw = hwnd.0 as isize;
    
    // Mark this window as animating to prevent event-driven retiles from interfering
    mark_window_animating(hwnd, true);
    
    thread::spawn(move || {
        unsafe {
            // Reconstruct HWND from raw handle
            let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
            
            // Get current window position
            let mut current_rect: RECT = mem::zeroed();
            if GetWindowRect(hwnd, &mut current_rect).is_err() {
                mark_window_animating(hwnd, false);
                return;
            }

            // Skip animation if already at target
            if current_rect == target_rect {
                mark_window_animating(hwnd, false);
                return;
            }

            // SAFETY: Cap animation steps to prevent overwhelming application message queues
            // Each step = one SetWindowPos = one WM_SIZE/WM_MOVE message to the app
            let safe_steps = (ANIMATION_STEPS).min(MAX_ANIMATION_STEPS);
            let frame_delay = Duration::from_millis(duration_ms / safe_steps);
            
            // SAFETY: Enforce minimum frame delay to give apps time to process messages
            let frame_delay = frame_delay.max(Duration::from_millis(MIN_FRAME_DELAY_MS));
            
            if safe_steps < ANIMATION_STEPS {
                info!("[{}][{}] Animation steps capped from {} to {} for safety (min frame delay: {}ms)",
                      DEBUG_NAME, DEBUG_SUBTAG, ANIMATION_STEPS, safe_steps, MIN_FRAME_DELAY_MS);
            }

            // Animate through frames - force complete redraws to prevent gray boxes
            for frame in 0..=safe_steps {
                let progress = frame as f32 / safe_steps as f32;
                let animated_rect = interpolate_rect(current_rect, target_rect, progress);

                let width = animated_rect.right - animated_rect.left;
                let height = animated_rect.bottom - animated_rect.top;

                // Move window with flags that force complete window redraw to prevent gray boxes
                // SWP_DRAWFRAME + SWP_FRAMECHANGED ensures window chrome and content are redrawn
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    animated_rect.left,
                    animated_rect.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_DRAWFRAME,
                );

                // Force immediate window content update to prevent gray boxes
                let _ = RedrawWindow(
                    Some(hwnd),
                    None,
                    None,
                    RDW_UPDATENOW,
                );

                if frame < ANIMATION_STEPS {
                    thread::sleep(frame_delay);
                    // Extra delay to allow window content to catch up and redraw
                    thread::sleep(Duration::from_millis(10));
                }
            }

            // Final position - ensure it's exactly at target with full redraw
            let width = target_rect.right - target_rect.left;
            let height = target_rect.bottom - target_rect.top;
            let _ = SetWindowPos(
                hwnd,
                None,
                target_rect.left,
                target_rect.top,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_DRAWFRAME,
            );
            
            // Force complete window redraw to eliminate any gray box artifacts
            let _ = RedrawWindow(
                Some(hwnd),
                None,
                None,
                RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN | RDW_ERASE,
            );
            
            // Critical delay: Give window content time to fully render at final position
            thread::sleep(Duration::from_millis(50));
            
            // Animation complete - mark window as no longer animating so retiles can resume
            mark_window_animating(hwnd, false);
        }
    });
}

/// Apply layout to windows with smooth animations
pub fn apply_layout(
    windows: &[ManagedWindow],
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) {
    info!("[{}][{}] Applying layout to {} windows", DEBUG_NAME, DEBUG_SUBTAG, windows.len());

    let animation_enabled = config.animation_enabled.unwrap_or(false);  // DEFAULT: false to prevent gray boxes
    let animation_duration = config.animation_duration_ms.unwrap_or(DEFAULT_ANIMATION_DURATION_MS);

    for (idx, window) in windows.iter().enumerate() {
        let rect = layout_strategy.calculate_layout(display, windows.len(), idx, config);
        
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        
        info!("[{}][{}] Layout window '{}' to ({}, {}) with size {}x{}", 
              DEBUG_NAME, DEBUG_SUBTAG, window.title, rect.left, rect.top, width, height);
        

        if animation_enabled {
            // Use animation for smooth transition (but this can cause gray boxes with concurrent threads)
            animate_window(window.hwnd, rect, animation_duration);
        } else {
            // Instant positioning without animation - MOST RELIABLE, no gray boxes
            unsafe {
                let _ = SetWindowPos(
                    window.hwnd,
                    None,
                    rect.left,
                    rect.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_DRAWFRAME,
                );
                
                // Invalidate entire window region to force repaint
                let _ = InvalidateRect(Some(window.hwnd), None, true);
                
                // Force comprehensive redraw to eliminate gray boxes
                let _ = RedrawWindow(
                    Some(window.hwnd),
                    None,
                    None,
                    RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN | RDW_ERASE,
                );
                
                // Give content time to render at new size/position
                thread::sleep(Duration::from_millis(20));
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
