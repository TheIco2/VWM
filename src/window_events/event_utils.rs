// ~/src/window_events/event_utils.rs

use crate::{
    info, DEBUG_NAME,
    types::{DisplayInfo, WindowManagerConfig},
    config::FiltersConfig,
    layout::get_layout_strategy,
    window_events::{
        event_manager::get_event_manager,
    },
};

use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        UI::{
            Accessibility::*,
            WindowsAndMessaging::*,
        },
    },
};

pub const DEBUG_SUBTAG: &str = "EVENT_UTILS";
pub const LOG_SPAM_WINDOW: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    CreateShow,
}

#[derive(Debug, Clone)]
pub struct WindowState {
    pub is_being_dragged: bool,
    pub last_event_time: Instant,
    pub last_log_time: Instant,
    pub last_log_kind: Option<EventKind>,
}

pub static RETILE_SCHEDULED: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();

/// Windows event hook callback
pub unsafe extern "system" fn win_event_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if hwnd.0.is_null() {
        return;
    }

    let Some(manager_arc) = get_event_manager() else {
        return;
    };

    let Ok(manager) = manager_arc.lock() else {
        return;
    };

    match event {
        EVENT_OBJECT_CREATE
        | EVENT_OBJECT_DESTROY
        | EVENT_OBJECT_SHOW
        | EVENT_OBJECT_HIDE => {
            if manager.should_log_event(hwnd, EventKind::CreateShow) {
                info!(
                    "[{}][{}] Structural window event: {:?}",
                    DEBUG_NAME, DEBUG_SUBTAG, hwnd
                );
            }

            manager.mark_window_created(hwnd);
            manager.schedule_retile();
        }

        EVENT_SYSTEM_MOVESIZESTART => {
            manager.mark_window_dragging(hwnd, true);
        }

        EVENT_SYSTEM_MOVESIZEEND => {
            manager.mark_window_dragging(hwnd, false);
            unsafe {
                let mut rect: RECT = std::mem::zeroed();
                if GetWindowRect(hwnd, &mut rect).is_err() {
                    return;
                }

                let mut best: Option<(&DisplayInfo, &WindowManagerConfig, i64)> = None;

                for (monitor, config) in manager.monitors.iter().zip(manager.configs.iter()) {
                    let inter_left = rect.left.max(monitor.x);
                    let inter_top = rect.top.max(monitor.y);
                    let inter_right = rect.right.min(monitor.x + monitor.width);
                    let inter_bottom = rect.bottom.min(monitor.y + monitor.height);

                    let area = if inter_right > inter_left && inter_bottom > inter_top {
                        (inter_right - inter_left) as i64 * (inter_bottom - inter_top) as i64
                    } else {
                        0
                    };

                    if area > 0 {
                        if best.map_or(true, |(_, _, best_area)| area > best_area) {
                            best = Some((monitor, config, area));
                        }
                    }
                }

                if let Some((monitor, config, _)) = best {
                    if config.enabled {
                        let default_filters = FiltersConfig::default();
                        let filters = config.filters.as_ref().unwrap_or(&default_filters);

                        let manager_type = config.manager_type.unwrap_or_default();
                        let layout = get_layout_strategy(manager_type);

                        let resized = crate::window_ops::update_resize_state_for_window(
                            monitor,
                            &manager.monitors,
                            config,
                            layout.as_ref(),
                            hwnd,
                        );

                        if resized {
                            // Immediate retile for resize operations - no debounce
                            // This makes resize feel instant and responsive
                            manager.schedule_immediate_retile();
                            return;
                        }

                        let state = crate::window_ops::BSP_STATE
                            .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
                            .clone();
                        let mut state_map = state.lock().unwrap();
                        let order = state_map.entry(monitor.id.clone()).or_insert_with(Vec::new);

                        let swapped = crate::window_ops::swap_window_order_by_drop(
                            monitor,
                            &manager.monitors,
                            filters,
                            hwnd,
                            order,
                        );

                        if !swapped && order.is_empty() {
                            *order = crate::window_ops::get_window_order_by_position(monitor, &manager.monitors, filters);
                        }
                    }
                }
            }

            // Use regular debounced retile for drag operations
            manager.schedule_retile();
        }


        _ => {}
    }

}