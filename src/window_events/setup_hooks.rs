// window_events/setup_hooks.rs

use windows::{
    Win32::{
        UI::{
            Accessibility::*,
            WindowsAndMessaging::*,
        },
    },
};
use crate::{
    info, DEBUG_NAME,
    window_events::{
        event_utils::win_event_proc,
    },
};

pub const DEBUG_SUBTAG: &str = "SETUP";

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
        if hook.0 != std::ptr::null_mut() {
            hooks.push(hook);
            info!("[{}][{}] Set up LOCATIONCHANGE event hook", DEBUG_NAME, DEBUG_SUBTAG);
        }
    }

    info!("[{}][{}] Set up {} event hooks", DEBUG_NAME, DEBUG_SUBTAG, hooks.len());
    hooks
}

