use std::{
    os::windows::ffi::{OsStringExt, OsStrExt},
    ffi::{OsString, OsStr},
    env,
    path::PathBuf
};

// OpenDesktop Wide String Utilities
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



// OpenDesktop Directory Utilities
// --------------------------------

// Get user home directory
pub fn user_home_dir() -> Option<PathBuf> {
    env::var("USERPROFILE").map(PathBuf::from).ok()
}

// Get OpenDesktop Root Directory
pub fn od_root_dir() -> Option<PathBuf> {
    user_home_dir().map(|p| p.join("ProjectOpen").join("OpenDesktop"))
}

// Get OpenDesktop Addons Directory
pub fn od_addons_dir() -> Option<PathBuf> {
    od_root_dir().map(|p| p.join("Addons"))
}

// Get OpenDesktop Assets Directory
pub fn od_assets_dir() -> Option<PathBuf> {
    od_root_dir().map(|p| p.join("Assets"))
}
// --------------------------------