// ~/Sentinel/sentinel-addons/statusbar/src/ipc/ipc_connector.rs

use serde_json::Value;
use windows::{
    core::{
        PCWSTR,
    },
    Win32::{
        System::Pipes::WaitNamedPipeW,
        Foundation::{HANDLE, INVALID_HANDLE_VALUE, CloseHandle},
        Storage::FileSystem::{
            CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            ReadFile, WriteFile,
        },
    },
};

use crate::{warn, error};


fn to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Sends a JSON IPC request to the Sentinel IPC server and returns the JSON response.
pub fn send_ipc_request(req: &Value) -> Option<Value> {
    unsafe {
        let name = to_wide(r"\\.\pipe\sentinel");
        let pipe_name = PCWSTR(name.as_ptr());

        // Wait for server
        if !WaitNamedPipeW(pipe_name, 5000).as_bool() {
            warn!("[IPC] WaitNamedPipe failed or timed out");
            return None;
        }

        // Open pipe
        let handle: HANDLE = match CreateFileW(
            pipe_name,
            (FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0) as u32,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                error!("[IPC] Failed to open pipe: {:?}", e);
                return None;
            }
        };

        if handle == INVALID_HANDLE_VALUE {
            error!("[IPC] Invalid handle returned from CreateFileW");
            return None;
        }

        // Serialize request
        let req_bytes = match serde_json::to_vec(req) {
            Ok(b) => b,
            Err(e) => {
                error!("[IPC] Failed to serialize request JSON: {:?}", e);
                if let Err(e2) = CloseHandle(handle) { warn!("[IPC] CloseHandle failed: {:?}", e2); }
                return None;
            }
        };
        // Write request
        let mut written: u32 = 0;
        if WriteFile(handle, Some(&req_bytes), Some(&mut written), None).is_err() {
            error!("[IPC] Failed to write to pipe");
            if let Err(e2) = CloseHandle(handle) { warn!("[IPC] CloseHandle failed: {:?}", e2); }
            return None;
        }
        let mut buffer: Vec<u8> = vec![0u8; 16 * 1024];
        let mut read: u32 = 0;
        if ReadFile(handle, Some(&mut buffer), Some(&mut read), None).is_err() {
            error!("[IPC] Failed to read from pipe");
            if let Err(e2) = CloseHandle(handle) { warn!("[IPC] CloseHandle failed: {:?}", e2); }
            return None;
        }

        // Close handle
        if let Err(e2) = CloseHandle(handle) { warn!("[IPC] CloseHandle failed: {:?}", e2); }

        // Parse response
        match serde_json::from_slice::<Value>(&buffer[..read as usize]) {
            Ok(v) => Some(v),
            Err(e) => {
                error!("[IPC] Failed to parse IPC response JSON: {:?}", e);
                None
            }
        }
    }
}