// ~/src/config.rs
// Configuration module - exports all config-related types

pub mod animation;
pub mod events;
pub mod filters;
pub mod styling;
pub mod universal;

pub use animation::AnimationConfig;
pub use events::EventsConfig;
pub use filters::FiltersConfig;
pub use styling::StylingConfig;
pub use universal::UniversalConfig;
