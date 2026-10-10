//! Pulsar core: pure data collection, models and parsers. No UI, no framework.

pub mod cgroup;
pub mod desktop;
pub mod error;
pub mod group;
pub mod meminfo;
pub mod power;
pub mod pressure;
pub mod process;
pub mod pss;
pub mod rapl;

pub use error::{Error, Result};
