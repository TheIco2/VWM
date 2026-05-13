use std::{
    os::windows::ffi::{OsStringExt, OsStrExt},
    ffi::{OsString, OsStr},
    env,
    path::PathBuf
};

// VEIL Wide String Utilities
// --------------------------------

// Utility function to convert &str to wide string (Vec<u16>)
pub fn to_wstring(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// Utility function to convert wide string (slice of u16) to String
pub fn _from_wstring(ws: &[u16]) -> String {
    let len = ws.iter().position(|&c| c == 0).unwrap_or(ws.len());
    OsString::from_wide(&ws[..len]).to_string_lossy().into_owned()
}
// --------------------------------



// VEIL Directory Utilities
// --------------------------------

// Get user home directory
pub fn user_home_dir() -> Option<PathBuf> {
    env::var("USERPROFILE").map(PathBuf::from).ok()
}

// Get VEIL Root Directory
pub fn veil_root_dir() -> Option<PathBuf> {
    user_home_dir().map(|p| p.join("VEIL").join("Core"))
}

// Get VEIL Base Directory (contains Core and standalone addon folders)
pub fn veil_base_dir() -> Option<PathBuf> {
    user_home_dir().map(|p| p.join("VEIL"))
}

// Get VEIL Addons Directory
pub fn veil_addons_dir() -> Option<PathBuf> {
    veil_root_dir().map(|p| p.join("Addons"))
}

// Get VEIL Assets Directory
pub fn veil_assets_dir() -> Option<PathBuf> {
    veil_root_dir().map(|p| p.join("Assets"))
}

// Get standalone app directory (e.g. ~/VEIL/WindowManager)
pub fn standalone_app_dir(app_name: &str) -> Option<PathBuf> {
    veil_base_dir().map(|p| p.join(app_name))
}
// --------------------------------