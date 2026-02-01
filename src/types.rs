use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MonitorInfo {
    pub id: String,
    pub primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

#[derive(Debug, Deserialize)]
pub struct IpcMonitorsResponse {
    pub ok: bool,
    pub data: Option<Vec<MonitorInfo>>,
    pub error: Option<String>,
}
