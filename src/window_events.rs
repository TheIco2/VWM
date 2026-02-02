// Window Event Handling Module
// Handles Windows event hooks for window creation, destruction, and movement

use crate::{info, DEBUG_NAME};
use crate::types::{DisplayInfo, WindowManagerConfig};
use crate::layout::get_layout_strategy;
use crate::window_ops::retile_windows;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::HWND,
        UI::{
            Accessibility::*,
            WindowsAndMessaging::*,
        },
    },
};

const DEBUG_SUBTAG: &str = "EVENTS";
const LOG_SPAM_WINDOW: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    CreateShow,
    DestroyHide,
    MoveSizeStart,
    MoveSizeEnd,
}

#[derive(Debug, Clone)]
pub struct WindowState {
    pub is_being_dragged: bool,
    pub last_event_time: Instant,
    pub last_log_time: Instant,
    pub last_log_kind: Option<EventKind>,
}

pub struct EventManager {
    pub monitors: Vec<DisplayInfo>,
    pub configs: Vec<WindowManagerConfig>,
    pub window_states: Arc<Mutex<HashMap<isize, WindowState>>>,
    pub debounce_delay: Duration,
}

impl EventManager {
    pub fn new(monitors: Vec<DisplayInfo>, configs: Vec<WindowManagerConfig>) -> Self {
        let debounce_ms = configs
            .first()
            .and_then(|config| config.debounce_ms)
            .unwrap_or(500);
        EventManager {
            monitors,
            configs,
            window_states: Arc::new(Mutex::new(HashMap::new())),
            debounce_delay: Duration::from_millis(debounce_ms),
        }
    }

    /// Mark a window as being dragged
    pub fn mark_window_dragging(&self, hwnd: HWND, dragging: bool) {
        let Ok(mut states) = self.window_states.lock() else {
            return;
        };
        let hwnd_val = hwnd.0 as isize;
        
        if let Some(state) = states.get_mut(&hwnd_val) {
            state.is_being_dragged = dragging;
            state.last_event_time = Instant::now();
        } else {
            states.insert(hwnd_val, WindowState {
                is_being_dragged: dragging,
                last_event_time: Instant::now(),
                last_log_time: Instant::now() - LOG_SPAM_WINDOW,
                last_log_kind: None,
            });
        }
    }

    /// Mark a window as newly created
    pub fn mark_window_created(&self, hwnd: HWND) {
        let Ok(mut states) = self.window_states.lock() else {
            return;
        };
        let hwnd_val = hwnd.0 as isize;
        
        if let Some(state) = states.get_mut(&hwnd_val) {
            state.last_event_time = Instant::now();
        } else {
            states.insert(hwnd_val, WindowState {
                is_being_dragged: false,
                last_event_time: Instant::now(),
                last_log_time: Instant::now() - LOG_SPAM_WINDOW,
                last_log_kind: None,
            });
        }
    }

    /// Rate-limit noisy event logs per window
    pub fn should_log_event(&self, hwnd: HWND, kind: EventKind) -> bool {
        let Ok(mut states) = self.window_states.lock() else {
            return false;
        };
        let hwnd_val = hwnd.0 as isize;
        let now = Instant::now();

        let state = states.entry(hwnd_val).or_insert(WindowState {
            is_being_dragged: false,
            last_event_time: now,
            last_log_time: now - LOG_SPAM_WINDOW,
            last_log_kind: None,
        });

        if state.last_log_kind == Some(kind) && now.duration_since(state.last_log_time) < LOG_SPAM_WINDOW {
            return false;
        }

        state.last_log_kind = Some(kind);
        state.last_log_time = now;
        true
    }

    /// Remove window from tracking
    pub fn remove_window(&self, hwnd: HWND) {
        if let Ok(mut states) = self.window_states.lock() {
            states.remove(&(hwnd.0 as isize));
        }
    }

    /// Check if any window is currently being dragged
    pub fn is_any_window_dragging(&self) -> bool {
        let Ok(states) = self.window_states.lock() else {
            return false;
        };
        states.values().any(|s| s.is_being_dragged)
    }

    /// Schedule a retile after debounce delay (coalesces multiple events)
    pub fn schedule_retile(&self) {
        let monitors = self.monitors.clone();
        let configs = self.configs.clone();
        let debounce_delay = self.debounce_delay;
        let window_states = Arc::clone(&self.window_states);

        std::thread::spawn(move || {
            std::thread::sleep(debounce_delay);

            // Check if any window is still being dragged
            {
                if let Ok(states) = window_states.lock() {
                    if states.values().any(|s| s.is_being_dragged) {
                        info!("[{}][{}] Retile cancelled - window still being dragged", DEBUG_NAME, DEBUG_SUBTAG);
                        return;
                    }
                } else {
                    return;
                }
            }

            // Retile all monitors
            for (monitor, config) in monitors.iter().zip(configs.iter()) {
                if config.enabled {
                    let manager_type = config.manager_type.unwrap_or_default();
                    let layout = get_layout_strategy(manager_type);
                    retile_windows(monitor, config, layout.as_ref());
                }
            }
            
            // Clean up stale window states (windows not seen in 5 seconds)
            if let Ok(mut states) = window_states.lock() {
                let now = Instant::now();
                states.retain(|_, state| now.duration_since(state.last_event_time) < Duration::from_secs(5));
            }
        });
    }
}

/// Global event manager (will be set during initialization)
static EVENT_MANAGER: OnceLock<Arc<Mutex<EventManager>>> = OnceLock::new();

pub fn set_event_manager(manager: EventManager) {
    let _ = EVENT_MANAGER.set(Arc::new(Mutex::new(manager)));
}

fn get_event_manager() -> Option<Arc<Mutex<EventManager>>> {
    EVENT_MANAGER.get().cloned()
}

/// Windows event hook callback
unsafe extern "system" fn win_event_proc(
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
        EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW => {
            if manager.should_log_event(hwnd, EventKind::CreateShow) {
                info!("[{}][{}] Window created/shown: {:?}", DEBUG_NAME, DEBUG_SUBTAG, hwnd);
            }
            manager.mark_window_created(hwnd);
            // Don't retile immediately - wait to see if it's being dragged
            manager.schedule_retile();
        }
        EVENT_OBJECT_DESTROY | EVENT_OBJECT_HIDE => {
            if manager.should_log_event(hwnd, EventKind::DestroyHide) {
                info!("[{}][{}] Window destroyed/hidden: {:?}", DEBUG_NAME, DEBUG_SUBTAG, hwnd);
            }
            manager.remove_window(hwnd);
            manager.schedule_retile();
        }
        EVENT_SYSTEM_MOVESIZESTART => {
            if manager.should_log_event(hwnd, EventKind::MoveSizeStart) {
                info!("[{}][{}] Window move/resize started: {:?}", DEBUG_NAME, DEBUG_SUBTAG, hwnd);
            }
            manager.mark_window_dragging(hwnd, true);
        }
        EVENT_SYSTEM_MOVESIZEEND => {
            if manager.should_log_event(hwnd, EventKind::MoveSizeEnd) {
                info!("[{}][{}] Window move/resize ended: {:?}", DEBUG_NAME, DEBUG_SUBTAG, hwnd);
            }
            manager.mark_window_dragging(hwnd, false);
            // Window was released - now we can retile
            manager.schedule_retile();
        }
        EVENT_OBJECT_LOCATIONCHANGE => {
            // Skip retile if window is currently being animated - prevents feedback loops
            if !crate::window_ops::is_window_animating(hwnd) {
                // Ignore location changes during animation
                // Windows should only be retiled on create/destroy/show/hide events
            }
        }
        _ => {}
    }
}

/// Set up Windows event hooks
pub fn setup_event_hooks() -> Vec<HWINEVENTHOOK> {
    let mut hooks = Vec::new();

    unsafe {
        // Hook for window creation and destruction
        let hook = SetWinEventHook(
            EVENT_OBJECT_CREATE,
            EVENT_OBJECT_DESTROY,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.0 != std::ptr::null_mut() {
            hooks.push(hook);
            info!("[{}][{}] Set up CREATE/DESTROY event hook", DEBUG_NAME, DEBUG_SUBTAG);
        }

        // Hook for window show/hide
        let hook = SetWinEventHook(
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_HIDE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.0 != std::ptr::null_mut() {
            hooks.push(hook);
            info!("[{}][{}] Set up SHOW/HIDE event hook", DEBUG_NAME, DEBUG_SUBTAG);
        }

        // Hook for window move/resize
        let hook = SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZEEND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.0 != std::ptr::null_mut() {
            hooks.push(hook);
            info!("[{}][{}] Set up MOVESIZE event hook", DEBUG_NAME, DEBUG_SUBTAG);
        }

        // Hook for location changes
        let hook = SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.0 != std::ptr::null_mut() {
            hooks.push(hook);
            info!("[{}][{}] Set up LOCATIONCHANGE event hook", DEBUG_NAME, DEBUG_SUBTAG);
        }
    }

    info!("[{}][{}] Set up {} event hooks", DEBUG_NAME, DEBUG_SUBTAG, hooks.len());
    hooks
}

/// Cleanup event hooks
pub fn cleanup_event_hooks(hooks: Vec<HWINEVENTHOOK>) {
    for hook in hooks {
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
    info!("[{}][{}] Cleaned up event hooks", DEBUG_NAME, DEBUG_SUBTAG);
}
