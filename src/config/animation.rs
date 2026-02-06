// ~/src/config/animation.rs
// Animation configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnimationConfig {
    pub enabled: Option<bool>,
    pub duration: Option<u64>,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        AnimationConfig {
            enabled: Some(true),
            duration: Some(150),
        }
    }
}
