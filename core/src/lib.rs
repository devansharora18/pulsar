//! Pulsar core: pure data collection, models and parsers. No UI, no framework.

pub mod actions;
pub mod cgroup;
pub mod config;
pub mod cpu;
pub mod desktop;
pub mod disk;
pub mod error;
pub mod filesystem;
pub mod group;
pub mod history;
pub mod hwmon;
pub mod meminfo;
pub mod net;
pub mod power;
pub mod pressure;
pub mod process;
pub mod pss;
pub mod rapl;
pub mod sampler;
pub mod snapshot;
pub mod sockets;
pub mod theme;

pub use error::{Error, Result};
