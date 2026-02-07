// ~/src/window_ops.rs
// Window Operations Module
// Handles window enumeration, filtering, and positioning

use crate::{info, DEBUG_NAME};
use crate::types::{DisplayInfo, ManagedWindow, WindowManagerConfig, ManagerType};
use crate::config::FiltersConfig;
use crate::config::styling::GapBehavior;
use crate::layout::LayoutStrategy;

use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::*,
        System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
    },
};
use std::mem;
use std::time::Duration;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub static BSP_STATE: OnceLock<Arc<Mutex<HashMap<String, Vec<isize>>>>> = OnceLock::new();
pub static RESIZE_STATE: OnceLock<Arc<Mutex<HashMap<String, HashMap<isize, ResizeIntent>>>>> = OnceLock::new();


const DEBUG_SUBTAG: &str = "WINDOW_OPS";
const DEFAULT_ANIMATION_DURATION_MS: u64 = 300;  // 300ms smooth animation
const ANIMATION_FRAMES: u64 = 32;        // 16 frames @ ~60fps = ~267ms animation (smooth curve)
const EDGE_TOLERANCE: i32 = 2;

#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeIntent {
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub top: Option<i32>,
    pub bottom: Option<i32>,
}

#[inline]
fn force_set_pos(hwnd: HWND, rect: RECT) {
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
                | SWP_ASYNCWINDOWPOS
                | SWP_NOSENDCHANGING,
        );
    }
}

/// Get the actual client-area aware rect accounting for DWM decorations
/// Modern Windows apps have invisible shadows/borders that need to be accounted for
fn get_adjusted_rect_for_positioning(hwnd: HWND, target: RECT) -> RECT {
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
pub fn should_manage_window(hwnd: HWND, filters: &FiltersConfig) -> bool {
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

        // Ignore popup windows without captions (tooltips/hover popups)
        if style.contains(WS_POPUP) && !style.contains(WS_CAPTION) {
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

        let class_lower = class_name.to_lowercase();
        if class_lower == "tooltips_class32" || class_lower == "sysshadow" {
            return false;
        }

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

fn enumerate_windows_raw(display: &DisplayInfo, all_displays: &[DisplayInfo], filters: &FiltersConfig) -> Vec<ManagedWindow> {
    unsafe {
        let display_rect = RECT {
            left: display.x,
            top: display.y,
            right: display.x + display.width,
            bottom: display.y + display.height,
        };

        // Check for adjacent monitors on each side
        let has_left = all_displays.iter().any(|d| d.x + d.width == display.x);
        let has_right = all_displays.iter().any(|d| d.x == display.x + display.width);
        let has_top = all_displays.iter().any(|d| d.y + d.height == display.y);
        let has_bottom = all_displays.iter().any(|d| d.y == display.y + display.height);

        // Callback data structure
        struct EnumData {
            windows: Vec<ManagedWindow>,
            display_rect: RECT,
            filters: FiltersConfig,
            has_left: bool,
            has_right: bool,
            has_top: bool,
            has_bottom: bool,
        }

        let mut data = EnumData {
            windows: Vec::new(),
            display_rect,
            filters: filters.clone(),
            has_left,
            has_right,
            has_top,
            has_bottom,
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

            // Apply 50% threshold only on sides with adjacent monitors
            let inter_left = rect.left.max(data.display_rect.left);
            let inter_top = rect.top.max(data.display_rect.top);
            let inter_right = rect.right.min(data.display_rect.right);
            let inter_bottom = rect.bottom.min(data.display_rect.bottom);

            if inter_right <= inter_left || inter_bottom <= inter_top {
                return true.into();
            }

            let window_width = (rect.right - rect.left) as i64;
            let window_height = (rect.bottom - rect.top) as i64;
            let inter_width = (inter_right - inter_left) as i64;
            let inter_height = (inter_bottom - inter_top) as i64;

            // Check horizontal constraint
            let h_ok = if data.has_left && rect.left < data.display_rect.left {
                inter_width * 100 >= window_width * 50
            } else if data.has_right && rect.right > data.display_rect.right {
                inter_width * 100 >= window_width * 50
            } else {
                true
            };

            // Check vertical constraint
            let v_ok = if data.has_top && rect.top < data.display_rect.top {
                inter_height * 100 >= window_height * 50
            } else if data.has_bottom && rect.bottom > data.display_rect.bottom {
                inter_height * 100 >= window_height * 50
            } else {
                true
            };

            if h_ok && v_ok {
                
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
        data.windows
    }
}

fn sort_windows_by_position(windows: &mut Vec<ManagedWindow>, order: Option<&Vec<isize>>) {
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

pub fn get_window_order_by_position(display: &DisplayInfo, all_displays: &[DisplayInfo], filters: &FiltersConfig) -> Vec<isize> {
    let mut windows = enumerate_windows_raw(display, all_displays, filters);
    sort_windows_by_position(&mut windows, None);
    windows.into_iter().map(|w| w.hwnd.0 as isize).collect()
}

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

    let windows = enumerate_windows_raw(display, all_displays, filters);
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

pub fn enumerate_windows_with_all(display: &DisplayInfo, all_displays: &[DisplayInfo], filters: &FiltersConfig) -> Vec<ManagedWindow> {
    let managed_windows_list = enumerate_windows_raw(display, all_displays, filters);

    // ---------------- BSP ORDER PERSISTENCE ----------------
    let state = BSP_STATE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone();

    let mut state_map = state.lock().unwrap();
    let order = state_map
        .entry(display.id.clone())
        .or_insert_with(Vec::new);
    let mut result = managed_windows_list;
    let mut position_sorted = result.clone();
    sort_windows_by_position(&mut position_sorted, Some(order));

    if order.is_empty() {
        order.extend(position_sorted.iter().map(|w| w.hwnd.0 as isize));
    }

    // Add new windows to persistent order
    for w in &position_sorted {
        let hwnd_val = w.hwnd.0 as isize;
        if !order.contains(&hwnd_val) {
            order.push(hwnd_val);
        }
    }

    // Remove windows that no longer exist
    order.retain(|h| result.iter().any(|w| w.hwnd.0 as isize == *h));

    // Sort windows by persistent BSP order
    result.sort_by_key(|w| {
        order
            .iter()
            .position(|h| *h == w.hwnd.0 as isize)
            .unwrap_or(usize::MAX)
    });
    // -------------------------------------------------------

    result
}

fn get_layout_bounds(display: &DisplayInfo, config: &WindowManagerConfig) -> RECT {
    let mut left = display.x;
    let mut top = display.y;
    let mut right = display.x + display.width;
    let mut bottom = display.y + display.height;

    if config.manager_type.unwrap_or_default() == ManagerType::Tiling {
        let gap_cfg = config.styling.as_ref().and_then(|s| s.gap.as_ref());
        let gap = gap_cfg.map(|g| g.space as i32).unwrap_or(0);
        let behavior = gap_cfg
            .map(|g| &g.behavior)
            .unwrap_or(&GapBehavior::PerWindow);

        let edge_gap = match behavior {
            GapBehavior::PerWindow => gap,
            GapBehavior::Shared => gap / 2,
        };

        left += edge_gap;
        top += edge_gap;
        right -= edge_gap;
        bottom -= edge_gap;
    }

    RECT {
        left,
        top,
        right,
        bottom,
    }
}

fn get_internal_gap(config: &WindowManagerConfig) -> i32 {
    let gap_cfg = config.styling.as_ref().and_then(|s| s.gap.as_ref());
    let gap = gap_cfg.map(|g| g.space as i32).unwrap_or(0);
    let behavior = gap_cfg
        .map(|g| &g.behavior)
        .unwrap_or(&GapBehavior::PerWindow);

    match behavior {
        GapBehavior::PerWindow => gap * 2,
        GapBehavior::Shared => gap,
    }
}

fn find_neighbor_indices(
    rects: &[(HWND, RECT)],
    idx: usize,
    side: &str,
    max_gap: i32,
) -> Vec<usize> {
    let Some(rect) = rects.get(idx).map(|r| r.1) else {
        return Vec::new();
    };
    let mut neighbors = Vec::new();

    let boundary = match side {
        "right" => rect.right,
        "left" => rect.left,
        "top" => rect.top,
        "bottom" => rect.bottom,
        _ => return neighbors,
    };

    for (j, (_, other)) in rects.iter().enumerate() {
        if j == idx {
            continue;
        }

        let aligned = match side {
            "right" => (other.left - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            "left" => (other.right - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            "top" => (other.bottom - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            "bottom" => (other.top - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            _ => false,
        };

        if aligned {
            neighbors.push(j);
        }
    }

    neighbors
}

fn find_aligned_indices(
    rects: &[(HWND, RECT)],
    idx: usize,
    side: &str,
) -> Vec<usize> {
    let Some(rect) = rects.get(idx).map(|r| r.1) else {
        return Vec::new();
    };
    let mut aligned = Vec::new();

    let boundary = match side {
        "right" => rect.right,
        "left" => rect.left,
        "top" => rect.top,
        "bottom" => rect.bottom,
        _ => return aligned,
    };

    let tol = EDGE_TOLERANCE * 4;
    for (j, (_, other)) in rects.iter().enumerate() {
        let matches = match side {
            "right" => (other.right - boundary).abs() <= tol,
            "left" => (other.left - boundary).abs() <= tol,
            "top" => (other.top - boundary).abs() <= tol,
            "bottom" => (other.bottom - boundary).abs() <= tol,
            _ => false,
        };

        if matches {
            aligned.push(j);
        }
    }

    aligned
}

fn compute_layout_targets(
    windows: &[ManagedWindow],
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) -> Vec<(HWND, RECT)> {
    let mut finals = Vec::new();

    for (idx, window) in windows.iter().enumerate() {
        let target = layout_strategy.calculate_layout(
            display,
            windows.len(),
            idx,
            config,
        );

        let adjusted_target = get_adjusted_rect_for_positioning(window.hwnd, target);
        finals.push((window.hwnd, adjusted_target));
    }

    finals
}

pub fn update_resize_state_for_window(
    display: &DisplayInfo,
    all_displays: &[DisplayInfo],
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
    hwnd: HWND,
) -> bool {
    if config.manager_type.unwrap_or_default() != ManagerType::Tiling {
        return false;
    }

    let default_filters = FiltersConfig::default();
    let filters = config.filters.as_ref().unwrap_or(&default_filters);
    let windows = enumerate_windows_with_all(display, all_displays, filters);
    if windows.is_empty() {
        return false;
    }

    let mut finals = compute_layout_targets(&windows, display, config, layout_strategy);
    apply_resize_deltas(display, config, &mut finals);

    let idx = finals.iter().position(|(h, _)| *h == hwnd);
    let Some(idx) = idx else { return false; };
    let expected = finals[idx].1;

    let actual = unsafe {
        let mut rect: RECT = mem::zeroed();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        rect
    };

    let mut intent = ResizeIntent::default();

    let dl = actual.left - expected.left;
    let dr = actual.right - expected.right;
    if dl.abs() >= 1 || dr.abs() >= 1 {
        if (dl - dr).abs() > 1 {
            if dl.abs() >= dr.abs() {
                intent.left = Some(actual.left);
            } else {
                intent.right = Some(actual.right);
            }
        }
    }

    let dt = actual.top - expected.top;
    let db = actual.bottom - expected.bottom;
    if dt.abs() >= 1 || db.abs() >= 1 {
        if (dt - db).abs() > 1 {
            if dt.abs() >= db.abs() {
                intent.top = Some(actual.top);
            } else {
                intent.bottom = Some(actual.bottom);
            }
        }
    }

    if intent.left.is_none()
        && intent.right.is_none()
        && intent.top.is_none()
        && intent.bottom.is_none()
    {
        return false;
    }

    let bounds = get_layout_bounds(display, config);
    let internal_gap = get_internal_gap(config);

    if expected.left <= bounds.left + EDGE_TOLERANCE {
        intent.left = None;
    }
    if expected.right >= bounds.right - EDGE_TOLERANCE {
        intent.right = None;
    }
    if expected.top <= bounds.top + EDGE_TOLERANCE {
        intent.top = None;
    }
    if expected.bottom >= bounds.bottom - EDGE_TOLERANCE {
        intent.bottom = None;
    }

    if intent.right.is_some() && find_neighbor_indices(&finals, idx, "right", internal_gap).is_empty() {
        intent.right = None;
    }
    if intent.left.is_some() && find_neighbor_indices(&finals, idx, "left", internal_gap).is_empty() {
        intent.left = None;
    }
    if intent.top.is_some() && find_neighbor_indices(&finals, idx, "top", internal_gap).is_empty() {
        intent.top = None;
    }
    if intent.bottom.is_some() && find_neighbor_indices(&finals, idx, "bottom", internal_gap).is_empty() {
        intent.bottom = None;
    }

    if intent.left.is_none()
        && intent.right.is_none()
        && intent.top.is_none()
        && intent.bottom.is_none()
    {
        return false;
    }

    let state = RESIZE_STATE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone();
    let mut state_map = state.lock().unwrap();
    let monitor_state = state_map.entry(display.id.clone()).or_insert_with(HashMap::new);
    monitor_state.insert(hwnd.0 as isize, intent);

    true
}

fn apply_resize_deltas(
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    rects: &mut Vec<(HWND, RECT)>,
) {
    if config.manager_type.unwrap_or_default() != ManagerType::Tiling {
        return;
    }

    let internal_gap = get_internal_gap(config);

    let state = RESIZE_STATE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone();
    let mut state_map = state.lock().unwrap();
    let monitor_state = match state_map.get_mut(&display.id) {
        Some(state) => state,
        None => return,
    };

    monitor_state.retain(|hwnd, _| rects.iter().any(|(h, _)| h.0 as isize == *hwnd));

    let mut deltas: HashMap<usize, (i32, i32, i32, i32)> = HashMap::new();
    let mut ops: Vec<(Vec<usize>, Vec<usize>, i32, &'static str)> = Vec::new();

    for idx in 0..rects.len() {
        let hwnd_val = (rects[idx].0).0 as isize;
        let Some(intent) = monitor_state.get(&hwnd_val).copied() else {
            continue;
        };

        if let Some(target_right) = intent.right {
            let delta = target_right - rects[idx].1.right;
            if delta != 0 {
                let same = find_aligned_indices(rects, idx, "right");
                let neighbors = find_neighbor_indices(rects, idx, "right", internal_gap);
                ops.push((same, neighbors, delta, "right"));
            }
        }

        if let Some(target_left) = intent.left {
            let delta = target_left - rects[idx].1.left;
            if delta != 0 {
                let same = find_aligned_indices(rects, idx, "left");
                let neighbors = find_neighbor_indices(rects, idx, "left", internal_gap);
                ops.push((same, neighbors, delta, "left"));
            }
        }

        if let Some(target_bottom) = intent.bottom {
            let delta = target_bottom - rects[idx].1.bottom;
            if delta != 0 {
                let same = find_aligned_indices(rects, idx, "bottom");
                let neighbors = find_neighbor_indices(rects, idx, "bottom", internal_gap);
                ops.push((same, neighbors, delta, "bottom"));
            }
        }

        if let Some(target_top) = intent.top {
            let delta = target_top - rects[idx].1.top;
            if delta != 0 {
                let same = find_aligned_indices(rects, idx, "top");
                let neighbors = find_neighbor_indices(rects, idx, "top", internal_gap);
                ops.push((same, neighbors, delta, "top"));
            }
        }
    }

    for (same, neighbors, delta, side) in ops {
        match side {
            "right" => {
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.1 += delta;
                }
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.0 += delta;
                }
            }
            "left" => {
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.0 += delta;
                }
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.1 += delta;
                }
            }
            "bottom" => {
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.3 += delta;
                }
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.2 += delta;
                }
            }
            "top" => {
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.2 += delta;
                }
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.3 += delta;
                }
            }
            _ => {}
        }
    }

    for (idx, (left_delta, right_delta, top_delta, bottom_delta)) in deltas {
        if left_delta != 0 {
            rects[idx].1.left += left_delta;
        }
        if right_delta != 0 {
            rects[idx].1.right += right_delta;
        }
        if top_delta != 0 {
            rects[idx].1.top += top_delta;
        }
        if bottom_delta != 0 {
            rects[idx].1.bottom += bottom_delta;
        }
    }
}

/// Animate multiple windows synchronously using DeferWindowPos for atomic updates
/// This keeps all windows in sync without race conditions or app crashes
fn animate_windows_batched(
    window_animations: &[(HWND, RECT, RECT)],
    duration_ms: u64,
) {
    if window_animations.is_empty() {
        return;
    }

    let frames = ANIMATION_FRAMES;
    let frame_delay = Duration::from_millis(duration_ms / frames);

    // Phase 1 — move only
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
        force_set_pos(*hwnd, *target);
    }
}

/// Apply layout to windows with smooth synchronized animations
pub fn apply_layout(
    windows: &[ManagedWindow],
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) {
    let animation_enabled = config.animation.as_ref()
        .and_then(|a| a.enabled)
        .unwrap_or(false);
    let animation_duration = config.animation.as_ref()
        .and_then(|a| a.duration)
        .unwrap_or(DEFAULT_ANIMATION_DURATION_MS);

    let mut animations = Vec::new();
    let mut finals = compute_layout_targets(windows, display, config, layout_strategy);
    apply_resize_deltas(display, config, &mut finals);

    unsafe {
        for (hwnd, target) in &finals {
            let mut current: RECT = mem::zeroed();
            if GetWindowRect(*hwnd, &mut current).is_err() {
                continue;
            }

            if current != *target {
                animations.push((*hwnd, current, *target));
            }
        }
    }

    if animation_enabled && !animations.is_empty() {
        animate_windows_batched(&animations, animation_duration);
    } else {
        // Instant layout — FORCE, no defer, no negotiation
        for (hwnd, rect) in finals {
            force_set_pos(hwnd, rect);
        }
    }
}

/// Retile all windows on a monitor
pub fn retile_windows(
    display: &DisplayInfo,
    all_displays: &[DisplayInfo],
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) {
    use crate::config::FiltersConfig;
    let default_filters = FiltersConfig::default();
    let filters = config.filters.as_ref().unwrap_or(&default_filters);
    let windows = enumerate_windows_with_all(display, all_displays, filters);

    if windows.is_empty() {
        info!("[{}][{}] No windows to tile", DEBUG_NAME, DEBUG_SUBTAG);
        return;
    }

    apply_layout(&windows, display, config, layout_strategy);
}