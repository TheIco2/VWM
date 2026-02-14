// window_events/event_manager.rs

use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::{HWND},
    },
};

use crate::{
    types::{DisplayInfo, WindowManagerConfig},
    layout::get_layout_strategy,
};

use crate::{
    window_events::{
        event_utils::{WindowState, LOG_SPAM_WINDOW, RETILE_SCHEDULED, EventKind},
    },
    window_ops::{
        retile_windows,
    },
};
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
            .and_then(|config| config.events.as_ref())
            .and_then(|e| e.debounce_ms)
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

    pub fn schedule_retile(&self) {
        let flag = RETILE_SCHEDULED
            .get_or_init(|| Arc::new(Mutex::new(false)))
            .clone();

        let Ok(mut scheduled) = flag.lock() else {
            return;
        };

        // Already scheduled? Do nothing.
        if *scheduled {
            return;
        }

        *scheduled = true;

        let monitors = self.monitors.clone();
        let configs = self.configs.clone();
        let delay = self.debounce_delay;
        let flag_clone = flag.clone();

        std::thread::spawn(move || {
            std::thread::sleep(delay);

            for (monitor, config) in monitors.iter().zip(configs.iter()) {
                if config.enabled {
                    let manager_type = config.manager_type.unwrap_or_default();
                    let layout = get_layout_strategy(manager_type);
                    retile_windows(monitor, &monitors, config, layout.as_ref());
                }
            }

            if let Ok(mut s) = flag_clone.lock() {
                *s = false;
            }
        });
    }

    /// Immediate retile without debounce - used for resize operations
    pub fn schedule_immediate_retile(&self) {
        let monitors = self.monitors.clone();
        let configs = self.configs.clone();

        // Small delay to let Windows finish its internal resize handling
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));

            for (monitor, config) in monitors.iter().zip(configs.iter()) {
                if config.enabled {
                    let manager_type = config.manager_type.unwrap_or_default();
                    let layout = get_layout_strategy(manager_type);
                    retile_windows(monitor, &monitors, config, layout.as_ref());
                }
            }
        });
    }

}

static EVENT_MANAGER: OnceLock<Arc<Mutex<EventManager>>> = OnceLock::new();

pub fn set_event_manager(manager: EventManager) {
    let _ = EVENT_MANAGER.set(Arc::new(Mutex::new(manager)));
}

pub fn get_event_manager() -> Option<Arc<Mutex<EventManager>>> {
    EVENT_MANAGER.get().cloned()
}