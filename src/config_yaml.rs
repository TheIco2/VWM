// ~/Sentinel/sentinel-addons/statusbar/src/config_yaml.rs

use serde::Deserialize;
use std::{fs};
use std::sync::{RwLock};
use std::time::{Instant, Duration};
use serde_yaml::Value;
use crate::{info, error};
use crate::types::MonitorInfo;

#[derive(Debug, Deserialize, Clone)]
pub struct BarStyling {
    pub alignment: Alignment,
    pub dimensions: Dimensions,
    pub padding: Padding,
    pub margin: Margin,
    pub border: Border,
    pub background: Background,
    pub blur: Blur,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Alignment { pub _vertical: String, pub _horizontal: String }
#[derive(Debug, Deserialize, Clone)]
pub struct Dimensions {
    #[serde(default = "default_width")]
    pub _width: String,
    pub _height: i32,
}

fn default_width() -> String {
    "100%".to_string()
}
#[derive(Debug, Deserialize, Clone)]
pub struct Padding { pub _top: i32, pub _left: i32, pub _bottom: i32, pub _right: i32 }
#[derive(Debug, Deserialize, Clone)]
pub struct Margin { pub _top: i32, pub _left: i32, pub _bottom: i32, pub _right: i32 }
#[derive(Debug, Deserialize, Clone)]
pub struct Border { pub _radius: i32, pub _style: String, pub _width: i32, pub _color: String, pub _opacity: f32 }
#[derive(Debug, Deserialize, Clone)]
pub struct Background { pub _type: String, pub _color: String, pub _opacity: f32 }
#[derive(Debug, Deserialize, Clone)]
pub struct Blur { pub _enabled: bool }

#[derive(Debug, Deserialize, Clone)]
pub struct BarWidgets { pub left: Vec<String>, pub center: Vec<String>, pub right: Vec<String> }

/* =========================
   CONFIG CACHE
========================= */

static CONFIG_CACHE: RwLock<Option<(Value, Instant)>> = RwLock::new(None);
const CACHE_TTL: Duration = Duration::from_secs(1);

fn load_config() -> Option<Value> {
    let now = Instant::now();
    {
        let cache = CONFIG_CACHE.read().unwrap();
        if let Some((v, t)) = &*cache {
            if now.duration_since(*t) < CACHE_TTL {
                return Some(v.clone());
            }
        }
    }

    // Try multiple config paths
    let config_paths = vec![
        // First, try user's Sentinel config directory
        std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default())
            .join(".Sentinel")
            .join("Addons")
            .join("statusbar")
            .join("config.yaml"),
        // Fall back to current directory
        std::path::PathBuf::from("./config.yaml"),
    ];

    let mut txt = None;
    for path in config_paths {
        if let Ok(content) = fs::read_to_string(&path) {
            txt = Some(content);
            break;
        }
    }

    let txt = txt?;
    let v: Value = serde_yaml::from_str(&txt).ok()?;
    let mut cache = CONFIG_CACHE.write().unwrap();
    *cache = Some((v.clone(), now));
    Some(v)
}

/* =========================
   PUBLIC HELPER FUNCTIONS
========================= */

pub fn load_bar_styling(monitor: MonitorInfo) -> Option<BarStyling> {
    info!("[CONFIG] Loading BarStyling for monitor {:?}", monitor.id);

    let v = load_config()?;
    let bars = v.get("bars")?.as_mapping().unwrap_or_else(|| {
        error!("[CONFIG] 'bars' section is not a mapping");
        panic!()
    });

    let bar_entry = bars.values().find(|bar| {
        // skip entries explicitly disabled
        if let Some(en) = bar.get(&Value::from("enabled")) {
            if let Some(b) = en.as_bool() {
                if !b { return false; }
            }
        }
        let matches = matches_monitor(bar, &monitor);
        info!("[CONFIG] Checking bar entry against monitor {}: match={}", monitor.id, matches);
        matches
    })?;

    let styling_val = bar_entry.get(&Value::from("styling"))?;
    match serde_yaml::from_value(styling_val.clone()) {
        Ok(styling) => Some(styling),
        Err(e) => {
            error!("[CONFIG] Failed to deserialize BarStyling for monitor {}: {}", monitor.id, e);
            None
        }
    }
}

pub fn get_widgets_for_bar(monitor: MonitorInfo) -> Option<BarWidgets> {
    info!("[CONFIG] Loading BarWidgets for monitor {:?}", monitor.id);

    let v = load_config()?;
    let bars = v.get("bars")?.as_mapping().unwrap_or_else(|| {
        error!("[CONFIG] 'bars' section is not a mapping");
        panic!()
    });

    let bar_entry = bars.values().find(|bar| {
        // skip entries explicitly disabled
        if let Some(en) = bar.get(&Value::from("enabled")) {
            if let Some(b) = en.as_bool() {
                if !b { return false; }
            }
        }
        matches_monitor(bar, &monitor)
    })?;

    let widgets_val = bar_entry.get(&Value::from("widgets"))?;
    serde_yaml::from_value(widgets_val.clone()).ok()
}

/* =========================
   HELPER FUNCTIONS
========================= */

fn matches_monitor(bar: &Value, monitor: &MonitorInfo) -> bool {
    if let Some(screen_index) = bar.get("screen_index") {
        if let Some(arr) = screen_index.as_sequence() {
            for s in arr {
                if let Some(sstr) = s.as_str() {
                    // wildcard
                    if sstr == "*" {
                        return true;
                    }
                    // primary
                    if sstr == "p" && monitor.primary {
                        return true;
                    }
                    // id string match
                    if sstr == monitor.id {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Return a default BarStyling when no config entry matches the monitor.
pub fn load_bar_styling_or_default(monitor: MonitorInfo) -> BarStyling {
    match load_bar_styling(monitor.clone()) {
        Some(s) => s,
        None => {
            info!("[CONFIG] No BarStyling found for monitor {:?}, using defaults", monitor.id);
            BarStyling {
                alignment: Alignment { _vertical: "center".into(), _horizontal: "center".into() },
                dimensions: Dimensions { _width: "100%".into(), _height: 42 },
                padding: Padding { _top: 0, _left: 0, _bottom: 0, _right: 0 },
                margin: Margin { _top: 0, _left: 0, _bottom: 0, _right: 0 },
                border: Border { _radius: 0, _style: "none".into(), _width: 0, _color: "#000000".into(), _opacity: 0.0 },
                background: Background { _type: "solid".into(), _color: "#00000000".into(), _opacity: 0.0 },
                blur: Blur { _enabled: false },
            }
        }
    }
}

/// Return configured widgets for the bar or an empty default set.
pub fn get_widgets_for_bar_or_default(monitor: MonitorInfo) -> BarWidgets {
    match get_widgets_for_bar(monitor.clone()) {
        Some(w) => w,
        None => {
            info!("[CONFIG] No BarWidgets found for monitor {:?}, using empty widget lists", monitor.id);
            BarWidgets { left: Vec::new(), center: Vec::new(), right: Vec::new() }
        }
    }
}

/// Check if automatic config updates are enabled
pub fn is_update_check_enabled() -> bool {
    if let Some(v) = load_config() {
        if let Some(enabled) = v.get("update_check").and_then(|v| v.as_bool()) {
            return enabled;
        }
    }
    false
}