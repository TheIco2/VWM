/// Window positioning and animation operations
/// Handles direct window placement, DWM frame adjustments, and smooth animations

use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::*,
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
        System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT},
    },
};

use std::time::Duration;
use super::ops_utils::{ANIMATION_FRAMES};

/// Set window position synchronously - use for resize operations to ensure precision
/// This ensures the window is placed exactly where intended without async delays
#[inline]
pub fn force_set_pos_sync(hwnd: HWND, rect: RECT) {
    unsafe {
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;

        let _ = SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            w,
            h,
            SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_NOSENDCHANGING, // No ASYNCWINDOWPOS for immediate, precise positioning
        );
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

/// Get the actual client-area aware rect accounting for DWM decorations
/// Modern Windows apps have invisible shadows/borders that need to be accounted for
pub fn get_adjusted_rect_for_positioning(hwnd: HWND, target: RECT) -> RECT {
    unsafe {
        let mut win: RECT = std::mem::zeroed();
        let mut dwm: RECT = std::mem::zeroed();

        if GetWindowRect(hwnd, &mut win).is_err() {
            return target;
        }

        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut dwm as *mut _ as _,
            std::mem::size_of::<RECT>() as u32,
        ).is_err() {
            return target;
        }

        // Correct resize border math
        let border_left   = dwm.left   - win.left;
        let border_top    = dwm.top    - win.top;
        let border_right  = win.right  - dwm.right;
        let border_bottom = win.bottom - dwm.bottom;

        RECT {
            left:   target.left   - border_left,
            top:    target.top    - border_top,
            right:  target.right  + border_right,
            bottom: target.bottom + border_bottom,
        }
    }
}

/// Animate multiple windows synchronously with smooth easing
/// This keeps all windows in sync without race conditions or app crashes
pub fn animate_windows_batched(
    window_animations: &[(HWND, RECT, RECT)],
    duration_ms: u64,
) {
    if window_animations.is_empty() {
        return;
    }

    let frames = ANIMATION_FRAMES;
    let frame_delay = Duration::from_millis(duration_ms / frames);

    // Phase 1 — move only with easing
    for frame in 0..frames {
        let t = frame as f32 / frames as f32;
        let eased = 1.0 - (1.0 - t).powf(3.0);

        for (hwnd, start, target) in window_animations {
            let x = (start.left as f32
                + (target.left - start.left) as f32 * eased) as i32;
            let y = (start.top as f32
                + (target.top - start.top) as f32 * eased) as i32;

            unsafe {
                let _ = SetWindowPos(
                    *hwnd,
                    None,
                    x,
                    y,
                    start.right - start.left,
                    start.bottom - start.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }

        std::thread::sleep(frame_delay);
    }

    // Phase 2 — REAL resize (critical)
    for (hwnd, _, target) in window_animations {
        force_set_pos_sync(*hwnd, *target); // Use sync positioning for final frame precision
    }
}
