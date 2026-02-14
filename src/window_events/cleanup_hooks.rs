// window_events/cleanup_hooks.rs

use crate::{
    info, DEBUG_NAME,
};

use windows::{
    Win32::{
        UI::{
            Accessibility::*,
        },
    },
};

const DEBUG_SUBTAG: &str = "CLEANUP";

pub fn cleanup_event_hooks(hooks: Vec<HWINEVENTHOOK>) {
    for hook in hooks {
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
    info!("[{}][{}] Cleaned up event hooks", DEBUG_NAME, DEBUG_SUBTAG);
}