use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::mem;

use crate::types::{WindowManagerConfig, DisplayInfo, ManagedWindow, ManagerType};
use crate::config::styling::GapBehavior;
use crate::layout::LayoutStrategy;
use crate::config::FiltersConfig;

use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::*,
    },
};

pub static BSP_STATE: OnceLock<Arc<Mutex<HashMap<String, Vec<isize>>>>> = OnceLock::new();
pub static RESIZE_STATE: OnceLock<Arc<Mutex<HashMap<String, HashMap<isize, ResizeIntent>>>>> = OnceLock::new();


pub const DEBUG_SUBTAG: &str = "WINDOW_OPS";
pub const DEFAULT_ANIMATION_DURATION_MS: u64 = 300;  // 300ms smooth animation
pub const ANIMATION_FRAMES: u64 = 32;        // 16 frames @ ~60fps = ~267ms animation (smooth curve)
pub const EDGE_TOLERANCE: i32 = 2;

#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeIntent {
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub top: Option<i32>,
    pub bottom: Option<i32>,
}

/// Get internal gap between windows based on configuration
pub fn get_internal_gap(config: &WindowManagerConfig) -> i32 {
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
    super::resize_ops::apply_resize_deltas(display, config, &mut finals);

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
        super::positioning::animate_windows_batched(&animations, animation_duration);
    } else {
        // Instant layout — use synchronous positioning for precision
        for (hwnd, rect) in finals {
            super::positioning::force_set_pos_sync(hwnd, rect);
        }
    }
}

pub fn compute_layout_targets(
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

        let adjusted_target = super::positioning::get_adjusted_rect_for_positioning(window.hwnd, target);
        finals.push((window.hwnd, adjusted_target));
    }

    finals
}

pub fn get_layout_bounds(display: &DisplayInfo, config: &WindowManagerConfig) -> RECT {
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
        let title = super::positioning::get_window_title(hwnd);
        let class_name = super::positioning::get_window_class_name(hwnd);
        let process_name = super::positioning::get_process_name(hwnd);

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

// Window Enumeration
pub fn enumerate_windows_raw(display: &DisplayInfo, all_displays: &[DisplayInfo], filters: &FiltersConfig) -> Vec<ManagedWindow> {
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
                    title: super::positioning::get_window_title(hwnd),
                    class_name: super::positioning::get_window_class_name(hwnd),
                    process_name: super::positioning::get_process_name(hwnd),
                };

                data.windows.push(managed_window);
            }

            true.into()
        }

        let _ = EnumWindows(Some(enum_callback), LPARAM(&mut data as *mut _ as isize));
        data.windows
    }
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
    super::window_enumeration::sort_windows_by_position(&mut position_sorted, Some(order));

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
/// Retile all windows on a monitor with the current layout strategy
pub fn retile_windows(
    display: &DisplayInfo,
    all_displays: &[DisplayInfo],
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
) {
    use crate::{info, DEBUG_NAME, config::FiltersConfig};
    let default_filters = FiltersConfig::default();
    let filters = config.filters.as_ref().unwrap_or(&default_filters);
    let windows = enumerate_windows_with_all(display, all_displays, filters);

    if windows.is_empty() {
        info!("[{}][{}] No windows to tile", DEBUG_NAME, DEBUG_SUBTAG);
        return;
    }

    apply_layout(&windows, display, config, layout_strategy);
}