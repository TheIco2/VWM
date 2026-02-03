// Animation Module
// Handles smooth window animations with easing and frame-based interpolation

use crate::{info, DEBUG_NAME};
use windows::{
    Win32::{
        Foundation::{HWND, RECT, WPARAM, LPARAM},
        UI::WindowsAndMessaging::*,
        Graphics::Gdi::{InvalidateRect, RedrawWindow, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW, RDW_ALLCHILDREN},
    },
};
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashSet;

const DEBUG_SUBTAG: &str = "ANIMATIONS";
const ANIMATION_FRAMES: u64 = 32;        // 32 frames @ ~60fps = ~533ms animation (smooth curve)
const MIN_FRAME_DELAY_MS: u64 = 32;      // ~60fps timing, uses DeferWindowPos for atomicity

// Track windows currently being animated to prevent event-triggered retiles from interfering
static ANIMATING_WINDOWS: OnceLock<Arc<Mutex<HashSet<isize>>>> = OnceLock::new();

fn get_animating_windows() -> Arc<Mutex<HashSet<isize>>> {
    Arc::clone(ANIMATING_WINDOWS.get_or_init(|| Arc::new(Mutex::new(HashSet::new()))))
}

/// Check if a window is currently animating
pub fn is_window_animating(hwnd: HWND) -> bool {
    match get_animating_windows().lock() {
        Ok(set) => set.contains(&(hwnd.0 as isize)),
        Err(_) => false,
    }
}

/// Mark a window as animating or not animating
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

/// Animate multiple windows synchronously using DeferWindowPos for atomic updates
/// This keeps all windows in sync without race conditions or app crashes
pub fn animate_windows_batched(
    window_animations: &[(HWND, RECT, RECT)],  // (hwnd, start_rect, target_rect)
    duration_ms: u64,
) {
    if window_animations.is_empty() {
        return;
    }

    // Mark all windows as animating
    for (hwnd, _, _) in window_animations {
        mark_window_animating(*hwnd, true);
    }

    // Clone data for thread
    let animations: Vec<(isize, RECT, RECT)> = window_animations
        .iter()
        .map(|(hwnd, start, target)| (hwnd.0 as isize, *start, *target))
        .collect();

    thread::spawn(move || {
        unsafe {
            // Disable redraw for all windows during animation to prevent flicker
            for (hwnd_raw, _, _) in &animations {
                let hwnd = HWND(*hwnd_raw as *mut std::ffi::c_void);
                SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(0)), Some(LPARAM(0)));
            }

            let num_frames = ANIMATION_FRAMES as usize;
            let frame_delay = Duration::from_millis(duration_ms / ANIMATION_FRAMES);
            let frame_delay = frame_delay.max(Duration::from_millis(MIN_FRAME_DELAY_MS));

            // Animate through all frames
            for frame in 0..=num_frames {
                let progress = frame as f32 / num_frames as f32;

                // Begin deferred window positioning - atomic batch update
                if let Ok(hdwp) = BeginDeferWindowPos(animations.len() as i32) {
                    let mut hdwp_result = hdwp;

                    // Queue all window position changes atomically
                    for (hwnd_raw, start_rect, target_rect) in &animations {
                        let hwnd = HWND(*hwnd_raw as *mut std::ffi::c_void);
                        let animated_rect = interpolate_rect(*start_rect, *target_rect, progress);
                        let width = animated_rect.right - animated_rect.left;
                        let height = animated_rect.bottom - animated_rect.top;

                        // DeferWindowPos queues the change without applying it yet
                        // Use SWP_NOCOPYBITS to prevent Windows from trying to copy old pixels
                        if let Ok(new_hdwp) = DeferWindowPos(
                            hdwp_result,
                            hwnd,
                            None,
                            animated_rect.left,
                            animated_rect.top,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOCOPYBITS,
                        ) {
                            hdwp_result = new_hdwp;
                        } else {
                            info!("[{}][{}] Failed to defer window position", DEBUG_NAME, DEBUG_SUBTAG);
                            break;
                        }
                    }

                    // Apply all queued changes atomically - all windows move together
                    let _ = EndDeferWindowPos(hdwp_result);
                }

                // Frame timing
                if frame < num_frames {
                    thread::sleep(frame_delay);
                }
            }

            // Final atomic update to ensure exact positioning
            if let Ok(hdwp) = BeginDeferWindowPos(animations.len() as i32) {
                let mut hdwp_result = hdwp;

                for (hwnd_raw, _, target_rect) in &animations {
                    let hwnd = HWND(*hwnd_raw as *mut std::ffi::c_void);
                    let width = target_rect.right - target_rect.left;
                    let height = target_rect.bottom - target_rect.top;

                    if let Ok(new_hdwp) = DeferWindowPos(
                        hdwp_result,
                        hwnd,
                        None,
                        target_rect.left,
                        target_rect.top,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOCOPYBITS,
                    ) {
                        hdwp_result = new_hdwp;
                    }
                }

                let _ = EndDeferWindowPos(hdwp_result);
            }

            // Re-enable redraw for all windows and force full repaint
            for (hwnd_raw, _, _) in &animations {
                let hwnd = HWND(*hwnd_raw as *mut std::ffi::c_void);
                // Re-enable drawing
                SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(1)), Some(LPARAM(0)));
                // Force a complete repaint
                InvalidateRect(Some(hwnd), None, true);
                RedrawWindow(
                    Some(hwnd),
                    None,
                    None,
                    RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
                );
            }

            // Allow windows to render
            thread::sleep(Duration::from_millis(10));

            // Mark all windows as no longer animating
            for (hwnd_raw, _, _) in &animations {
                let hwnd = HWND(*hwnd_raw as *mut std::ffi::c_void);
                mark_window_animating(hwnd, false);
            }
        }
    });
}
