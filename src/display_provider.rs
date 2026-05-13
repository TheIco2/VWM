use crate::{
    ipc_connector::request,
    types::DisplayInfo,
    DEBUG_NAME,
    error,
    info,
    warn,
};
use windows::core::BOOL;
use windows::Win32::{
    Foundation::{LPARAM, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors,
        GetMonitorInfoW,
        HDC,
        HMONITOR,
        MONITORINFO,
    },
};

pub fn get_displays(force_local: bool) -> Vec<DisplayInfo> {
    if !force_local {
        if let Some(ipc) = get_displays_via_ipc() {
            if !ipc.is_empty() {
                return ipc;
            }
        }
    }

    get_displays_local()
}

fn get_displays_via_ipc() -> Option<Vec<DisplayInfo>> {
    info!("[{}][IPC] Requesting monitors via pipe", DEBUG_NAME);

    if let Some(resp) = request("sysdata", "get_displays", None) {
        match serde_json::from_str::<Vec<DisplayInfo>>(&resp) {
            Ok(displays) => {
                info!("[{}][IPC] Parsed {} monitor(s)", DEBUG_NAME, displays.len());
                Some(displays)
            }
            Err(e) => {
                error!("[{}][IPC] Failed to parse monitor payload: {}", DEBUG_NAME, e);
                None
            }
        }
    } else {
        warn!("[{}][IPC] No IPC monitor response; will use local monitor query", DEBUG_NAME);
        None
    }
}

fn get_displays_local() -> Vec<DisplayInfo> {
    unsafe extern "system" fn enum_proc(
        monitor: HMONITOR,
        _hdc: HDC,
        _rc: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(data.0 as *mut Vec<DisplayInfo>);

        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if GetMonitorInfoW(monitor, &mut info as *mut MONITORINFO as *mut _).as_bool() {
            let rc = info.rcMonitor;
            let idx = monitors.len();
            monitors.push(DisplayInfo {
                id: idx.to_string(),
                primary: (info.dwFlags & 0x1) != 0,
                x: rc.left,
                y: rc.top,
                width: rc.right - rc.left,
                height: rc.bottom - rc.top,
                scale: 1.0,
            });
        }

        BOOL(1)
    }

    let mut monitors: Vec<DisplayInfo> = Vec::new();

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    if monitors.is_empty() {
        warn!("[{}][LOCAL] No monitors discovered from Win32", DEBUG_NAME);
    } else {
        info!("[{}][LOCAL] Discovered {} monitor(s)", DEBUG_NAME, monitors.len());
    }

    monitors
}
