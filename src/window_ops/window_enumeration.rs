/// Window enumeration and sorting operations
/// Handles enumerating windows by position and reordering via drag/drop

use crate::types::{DisplayInfo, ManagedWindow};
use crate::config::FiltersConfig;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

use std::mem;

/// Sort windows by their visual position (top-to-bottom, left-to-right)
/// Optionally respects a persistent ordering
pub fn sort_windows_by_position(windows: &mut Vec<ManagedWindow>, order: Option<&Vec<isize>>) {
    windows.sort_by(|a, b| unsafe {
        let mut rect_a: RECT = mem::zeroed();
        let mut rect_b: RECT = mem::zeroed();
        let ok_a = GetWindowRect(a.hwnd, &mut rect_a).is_ok();
        let ok_b = GetWindowRect(b.hwnd, &mut rect_b).is_ok();

        let (ay, ax) = if ok_a {
            ((rect_a.top + rect_a.bottom) / 2, (rect_a.left + rect_a.right) / 2)
        } else {
            (i32::MAX, i32::MAX)
        };
        let (by, bx) = if ok_b {
            ((rect_b.top + rect_b.bottom) / 2, (rect_b.left + rect_b.right) / 2)
        } else {
            (i32::MAX, i32::MAX)
        };

        let pos_cmp = ay.cmp(&by).then(ax.cmp(&bx));
        if pos_cmp != std::cmp::Ordering::Equal {
            return pos_cmp;
        }

        if let Some(order) = order {
            let a_idx = order.iter().position(|h| *h == a.hwnd.0 as isize).unwrap_or(usize::MAX);
            let b_idx = order.iter().position(|h| *h == b.hwnd.0 as isize).unwrap_or(usize::MAX);
            let order_cmp = a_idx.cmp(&b_idx);
            if order_cmp != std::cmp::Ordering::Equal {
                return order_cmp;
            }
        }

        (a.hwnd.0 as isize).cmp(&(b.hwnd.0 as isize))
    });
}

/// Get the window order sorted by current screen position
pub fn get_window_order_by_position(
    display: &DisplayInfo,
    all_displays: &[DisplayInfo],
    filters: &FiltersConfig,
) -> Vec<isize> {
    let mut windows = super::ops_utils::enumerate_windows_raw(display, all_displays, filters);
    sort_windows_by_position(&mut windows, None);
    windows.into_iter().map(|w| w.hwnd.0 as isize).collect()
}

/// Swap window order when one is dropped on another
/// Returns true if a swap occurred
pub fn swap_window_order_by_drop(
    display: &DisplayInfo,
    all_displays: &[DisplayInfo],
    filters: &FiltersConfig,
    moved_hwnd: HWND,
    order: &mut Vec<isize>,
) -> bool {
    let hwnd_val = moved_hwnd.0 as isize;
    if order.len() < 2 {
        return false;
    }

    let moved_rect = unsafe {
        let mut rect: RECT = mem::zeroed();
        if GetWindowRect(moved_hwnd, &mut rect).is_ok() {
            Some(rect)
        } else {
            None
        }
    };
    let Some(moved_rect) = moved_rect else { return false; };

    let center_x = (moved_rect.left + moved_rect.right) / 2;
    let center_y = (moved_rect.top + moved_rect.bottom) / 2;

    let windows = super::ops_utils::enumerate_windows_raw(display, all_displays, filters);
    let mut target_hwnd: Option<isize> = None;

    for w in windows {
        if w.hwnd == moved_hwnd {
            continue;
        }

        let mut rect: RECT = unsafe { mem::zeroed() };
        if unsafe { GetWindowRect(w.hwnd, &mut rect).is_err() } {
            continue;
        }

        if center_x >= rect.left && center_x < rect.right && center_y >= rect.top && center_y < rect.bottom {
            target_hwnd = Some(w.hwnd.0 as isize);
            break;
        }
    }

    let Some(target_hwnd) = target_hwnd else { return false; };
    if target_hwnd == hwnd_val {
        return false;
    }

    let pos_a = order.iter().position(|h| *h == hwnd_val);
    let pos_b = order.iter().position(|h| *h == target_hwnd);
    if let (Some(a), Some(b)) = (pos_a, pos_b) {
        order.swap(a, b);
        return true;
    }

    false
}
