// ~/src/data_loaders/json.rs
// TODO: Make this file a universal .json data loader.

use std::{
    fs,
    sync::{RwLock, LazyLock},
    time::{Duration, Instant},
    path::Path,
    collections::HashMap,
};
use serde_json::Value;

static JSON_CACHE: LazyLock<RwLock<HashMap<String, (Value, Instant)>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
const CACHE_TTL: Duration = Duration::from_secs(1);

pub fn load_json(path: &Path) -> Option<Value> {
    let txt = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&txt).ok()?;
    Some(v)
}